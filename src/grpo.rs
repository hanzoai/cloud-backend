use crate::error::{AppError, Result};
use crate::models::{ChatMessage, GrpoMetadata};
use pyo3::prelude::*;
use pyo3::types::PyList;
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

        Python::with_gil(|py| -> PyResult<()> {
            // Add zoo-gym to Python path
            let sys = py.import("sys")?;
            let path: &PyList = sys.getattr("path")?.downcast()?;
            path.insert(0, self.zoo_gym_path.to_str().unwrap())?;

            // Try to import zoo-gym modules (optional - may not be available)
            match py.import("src.gym.train.grpo.experience_manager") {
                Ok(_) => {
                    info!("Zoo-gym experience_manager loaded");
                }
                Err(e) => {
                    warn!("Zoo-gym not available, GRPO will use basic mode: {}", e);
                }
            }

            *initialized = true;
            Ok(())
        })?;

        info!("Python interpreter initialized");
        Ok(())
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

        // Load experiences if available
        let experiences_text = self.load_experiences(model).unwrap_or_default();

        // Inject experiences into messages
        let enhanced_messages = self.inject_experiences(messages, &experiences_text);

        // Generate multiple rollouts
        let mut outputs = Vec::new();
        let mut rewards = Vec::new();

        for i in 0..self.group_size {
            debug!("Generating rollout {}/{}", i + 1, self.group_size);

            // Generate response
            let response = provider_generate(query, &enhanced_messages).await?;

            // Compute reward
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
            debug!("Rewards vary, could extract semantic advantages");
            // TODO: Implement semantic extraction when zoo-gym is available
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
    }

    /// Load experiences from file (simplified)
    fn load_experiences(&self, model: &str) -> Result<String> {
        let exp_path = self.experience_lib_path.join(format!("{}_experiences.json", model.replace('/', "_")));

        if exp_path.exists() {
            std::fs::read_to_string(&exp_path)
                .map_err(|e| AppError::Grpo(format!("Failed to load experiences: {}", e)))
        } else {
            Ok(String::new())
        }
    }

    /// Inject experiences into messages
    fn inject_experiences(&self, messages: &[ChatMessage], experiences: &str) -> Vec<ChatMessage> {
        if experiences.is_empty() {
            return messages.to_vec();
        }

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
        let exp_path = self.experience_lib_path.join(format!("{}_experiences.json", model.replace('/', "_")));

        if exp_path.exists() {
            let content = std::fs::read_to_string(&exp_path)
                .map_err(|e| AppError::Grpo(format!("Failed to read experiences: {}", e)))?;

            // Parse as simple JSON
            let experiences: std::collections::HashMap<String, String> = serde_json::from_str(&content)
                .unwrap_or_default();

            Ok(experiences.into_iter().collect())
        } else {
            Ok(Vec::new())
        }
    }
}
