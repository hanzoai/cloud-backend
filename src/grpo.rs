use crate::error::{AppError, Result};
use crate::models::{ChatMessage, GrpoMetadata};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

/// GRPO Manager integrates with zoo-gym's Training-Free GRPO implementation
pub struct GrpoManager {
    zoo_gym_path: PathBuf,
    experience_lib_path: PathBuf,
    group_size: usize,
    max_operations: usize,
    python_initialized: Mutex<bool>,
}

impl GrpoManager {
    pub fn new(
        zoo_gym_path: String,
        experience_lib_path: String,
        group_size: usize,
        max_operations: usize,
    ) -> Result<Self> {
        Ok(Self {
            zoo_gym_path: PathBuf::from(zoo_gym_path),
            experience_lib_path: PathBuf::from(experience_lib_path),
            group_size,
            max_operations,
            python_initialized: Mutex::new(false),
        })
    }

    /// Initialize Python interpreter and import zoo-gym modules
    fn ensure_python_initialized(&self) -> Result<()> {
        let mut initialized = self.python_initialized.lock()
            .map_err(|e| AppError::Grpo(format!("Lock error: {}", e)))?;

        if *initialized {
            return Ok(());
        }

        Python::with_gil(|py| {
            // Add zoo-gym to Python path
            let sys = py.import("sys")
                .map_err(|e| AppError::Grpo(format!("Failed to import sys: {}", e)))?;
            let path: &PyList = sys.getattr("path")
                .map_err(|e| AppError::Grpo(format!("Failed to get sys.path: {}", e)))?
                .downcast()
                .map_err(|e| AppError::Grpo(format!("Failed to downcast sys.path: {}", e)))?;

            path.insert(0, self.zoo_gym_path.to_str().unwrap())
                .map_err(|e| AppError::Grpo(format!("Failed to add to sys.path: {}", e)))?;

            // Try to import zoo-gym modules
            py.import("src.gym.train.grpo.experience_manager")
                .map_err(|e| AppError::Grpo(format!("Failed to import experience_manager: {}", e)))?;
            py.import("src.gym.train.grpo.semantic_extractor")
                .map_err(|e| AppError::Grpo(format!("Failed to import semantic_extractor: {}", e)))?;

            info!("Python interpreter initialized successfully");
            *initialized = true;
            Ok(())
        })
    }

    /// Generate responses with GRPO enhancement
    pub async fn generate_with_grpo(
        &self,
        query: &str,
        messages: &[ChatMessage],
        model: &str,
        provider_generate: impl Fn(&str, &[ChatMessage]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>,
        groundtruth: Option<&str>,
    ) -> Result<(String, GrpoMetadata)> {
        self.ensure_python_initialized()?;

        Python::with_gil(|py| {
            // Load experience manager
            let exp_manager_mod = py.import("src.gym.train.grpo.experience_manager")
                .map_err(|e| AppError::Grpo(format!("Import error: {}", e)))?;
            let exp_manager_class = exp_manager_mod.getattr("ExperienceManager")
                .map_err(|e| AppError::Grpo(format!("Class not found: {}", e)))?;

            // Create experience manager instance
            let exp_path = self.experience_lib_path.join(format!("{}_experiences.json", model.replace("/", "_")));
            let kwargs = PyDict::new(py);
            kwargs.set_item("checkpoint_path", exp_path.to_str().unwrap())
                .map_err(|e| AppError::Grpo(format!("Failed to set checkpoint_path: {}", e)))?;

            let exp_manager = exp_manager_class.call((), Some(kwargs))
                .map_err(|e| AppError::Grpo(format!("Failed to create ExperienceManager: {}", e)))?;

            // Get formatted experiences
            let experiences_text: String = exp_manager.call_method0("format_for_prompt")
                .map_err(|e| AppError::Grpo(format!("Failed to format experiences: {}", e)))?
                .extract()
                .map_err(|e| AppError::Grpo(format!("Failed to extract experiences: {}", e)))?;

            debug!("Loaded {} experiences",
                exp_manager.call_method0("__len__")
                    .and_then(|v| v.extract::<usize>())
                    .unwrap_or(0)
            );

            // Inject experiences into messages
            let enhanced_messages = self.inject_experiences(messages, &experiences_text);

            // Generate multiple rollouts
            let rt = tokio::runtime::Handle::current();
            let mut outputs = Vec::new();
            let mut rewards = Vec::new();

            for i in 0..self.group_size {
                debug!("Generating rollout {}/{}", i + 1, self.group_size);

                // Generate response
                let response = rt.block_on(async {
                    provider_generate(query, &enhanced_messages).await
                })?;

                // Compute reward (simple for now - could be more sophisticated)
                let reward = if let Some(gt) = groundtruth {
                    self.compute_reward(&response, gt)
                } else {
                    0.5 // Neutral reward without groundtruth
                };

                outputs.push(response);
                rewards.push(reward);
            }

            // Find best response
            let best_idx = rewards.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            let best_response = outputs[best_idx].clone();
            let avg_reward: f64 = rewards.iter().sum::<f64>() / rewards.len() as f64;

            // If rewards vary, extract semantic advantages
            let rewards_vary = rewards.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap()
                != rewards.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

            if rewards_vary {
                debug!("Rewards vary, extracting semantic advantages");
                self.extract_and_update_experiences(
                    py,
                    query,
                    &outputs,
                    &rewards,
                    groundtruth,
                    &exp_manager,
                )?;
            }

            Ok((
                best_response,
                GrpoMetadata {
                    experiences_used: self.parse_experience_ids(&experiences_text),
                    group_size: self.group_size,
                    best_reward: rewards[best_idx],
                    avg_reward,
                },
            ))
        })
    }

    /// Inject experiences into messages
    fn inject_experiences(&self, messages: &[ChatMessage], experiences: &str) -> Vec<ChatMessage> {
        let mut enhanced = vec![ChatMessage {
            role: "system".to_string(),
            content: format!(
                "You are a helpful AI assistant. When solving problems, carefully read and apply these learned experiences:\n\n{}\n",
                experiences
            ),
        }];
        enhanced.extend_from_slice(messages);
        enhanced
    }

    /// Compute reward for a response
    fn compute_reward(&self, response: &str, groundtruth: &str) -> f64 {
        // Simple similarity-based reward (could be improved with more sophisticated metrics)
        let response_lower = response.to_lowercase();
        let gt_lower = groundtruth.to_lowercase();

        if response_lower.contains(&gt_lower) {
            1.0
        } else {
            // Compute word overlap
            let response_words: std::collections::HashSet<_> = response_lower.split_whitespace().collect();
            let gt_words: std::collections::HashSet<_> = gt_lower.split_whitespace().collect();
            let overlap = response_words.intersection(&gt_words).count();
            let total = gt_words.len();

            if total > 0 {
                overlap as f64 / total as f64
            } else {
                0.0
            }
        }
    }

    /// Extract semantic advantages and update experience library
    fn extract_and_update_experiences(
        &self,
        py: Python,
        query: &str,
        outputs: &[String],
        rewards: &[f64],
        groundtruth: Option<&str>,
        exp_manager: &PyAny,
    ) -> Result<()> {
        // Import semantic extractor
        let semantic_mod = py.import("src.gym.train.grpo.semantic_extractor")
            .map_err(|e| AppError::Grpo(format!("Import error: {}", e)))?;
        let trajectory_class = semantic_mod.getattr("Trajectory")
            .map_err(|e| AppError::Grpo(format!("Class not found: {}", e)))?;
        let extractor_class = semantic_mod.getattr("SemanticExtractor")
            .map_err(|e| AppError::Grpo(format!("Class not found: {}", e)))?;

        // Create dummy LLM client (TODO: integrate real API)
        let llm_client = py.None();
        let kwargs = PyDict::new(py);
        kwargs.set_item("max_operations", self.max_operations)?;

        let extractor = extractor_class.call((llm_client,), Some(kwargs))
            .map_err(|e| AppError::Grpo(format!("Failed to create SemanticExtractor: {}", e)))?;

        // Create trajectories
        let trajectories = PyList::empty(py);
        for (output, &reward) in outputs.iter().zip(rewards.iter()) {
            let traj_kwargs = PyDict::new(py);
            traj_kwargs.set_item("query", query)?;
            traj_kwargs.set_item("output", output)?;
            traj_kwargs.set_item("reward", reward)?;
            if let Some(gt) = groundtruth {
                traj_kwargs.set_item("groundtruth", gt)?;
            }

            let traj = trajectory_class.call((), Some(traj_kwargs))?;
            trajectories.append(traj)?;
        }

        // Get formatted experiences
        let experiences_text: String = exp_manager.call_method0("format_for_prompt")?.extract()?;

        // Extract group advantage
        let operations_list: Vec<std::collections::HashMap<String, serde_json::Value>> = extractor
            .call_method("extract_group_advantage", (trajectories, experiences_text, true), None)
            .and_then(|ops| ops.extract())
            .unwrap_or_default();

        if !operations_list.is_empty() {
            info!("Applying {} operations to experience library", operations_list.len());

            // Convert to Python list
            let ops_py = PyList::new(py, operations_list.iter().map(|op| {
                let dict = PyDict::new(py);
                for (k, v) in op {
                    dict.set_item(k, v.to_string()).ok();
                }
                dict
            }));

            exp_manager.call_method("apply_operations", (ops_py,), None)
                .map_err(|e| AppError::Grpo(format!("Failed to apply operations: {}", e)))?;

            // Save experiences
            let exp_path = self.experience_lib_path.join("experiences.json");
            exp_manager.call_method("save", (exp_path.to_str().unwrap(),), None)
                .map_err(|e| AppError::Grpo(format!("Failed to save experiences: {}", e)))?;
        }

        Ok(())
    }

    /// Parse experience IDs from formatted text
    fn parse_experience_ids(&self, experiences_text: &str) -> Vec<String> {
        experiences_text
            .lines()
            .filter_map(|line| {
                if line.starts_with("[G") {
                    line.split(']').next().and_then(|id| id.strip_prefix('['))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// List all experiences for a model
    pub fn list_experiences(&self, model: &str) -> Result<Vec<(String, String)>> {
        self.ensure_python_initialized()?;

        Python::with_gil(|py| {
            let exp_manager_mod = py.import("src.gym.train.grpo.experience_manager")?;
            let exp_manager_class = exp_manager_mod.getattr("ExperienceManager")?;

            let exp_path = self.experience_lib_path.join(format!("{}_experiences.json", model.replace("/", "_")));
            let kwargs = PyDict::new(py);
            kwargs.set_item("checkpoint_path", exp_path.to_str().unwrap())?;

            let exp_manager = exp_manager_class.call((), Some(kwargs))?;
            let experiences_dict: std::collections::HashMap<String, String> = exp_manager
                .getattr("experiences")?
                .extract()?;

            Ok(experiences_dict.into_iter().collect())
        }).map_err(|e: PyErr| AppError::Grpo(format!("Python error: {}", e)))
    }
}
