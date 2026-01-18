use crate::error::{AppError, Result};
use crate::models::{ChatMessage, ChatCompletionResponse, ChatChoice, Usage};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub enum Provider {
    DeepSeek { api_key: String },
    OpenAI { api_key: String },
    Anthropic { api_key: String },
    DigitalOcean { api_key: String },
    Local { base_url: String },
}

impl Provider {
    pub fn from_model(model: &str, config: &crate::config::Config) -> Result<Self> {
        if model.starts_with("deepseek-") {
            config.deepseek_api_key.as_ref()
                .map(|key| Provider::DeepSeek { api_key: key.clone() })
                .ok_or_else(|| AppError::Provider("DeepSeek API key not configured".to_string()))
        } else if model.starts_with("gpt-") {
            config.openai_api_key.as_ref()
                .map(|key| Provider::OpenAI { api_key: key.clone() })
                .ok_or_else(|| AppError::Provider("OpenAI API key not configured".to_string()))
        } else if model.starts_with("claude-") {
            config.anthropic_api_key.as_ref()
                .map(|key| Provider::Anthropic { api_key: key.clone() })
                .ok_or_else(|| AppError::Provider("Anthropic API key not configured".to_string()))
        } else if model.starts_with("qwen") || model.starts_with("zen-") {
            Ok(Provider::Local {
                base_url: std::env::var("LOCAL_NODE_URL")
                    .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            })
        } else {
            // Default to DeepSeek
            config.deepseek_api_key.as_ref()
                .map(|key| Provider::DeepSeek { api_key: key.clone() })
                .ok_or_else(|| AppError::Provider("No suitable provider found".to_string()))
        }
    }

    pub async fn generate(
        &self,
        client: &Client,
        model: &str,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        max_tokens: Option<i32>,
    ) -> Result<ChatCompletionResponse> {
        match self {
            Provider::DeepSeek { api_key } => {
                self.call_openai_compatible(
                    client,
                    "https://api.deepseek.com/v1",
                    api_key,
                    model,
                    messages,
                    temperature,
                    max_tokens,
                ).await
            }
            Provider::OpenAI { api_key } => {
                self.call_openai_compatible(
                    client,
                    "https://api.openai.com/v1",
                    api_key,
                    model,
                    messages,
                    temperature,
                    max_tokens,
                ).await
            }
            Provider::Anthropic { api_key } => {
                self.call_anthropic(
                    client,
                    api_key,
                    model,
                    messages,
                    temperature,
                    max_tokens,
                ).await
            }
            Provider::DigitalOcean { api_key } => {
                self.call_openai_compatible(
                    client,
                    "https://inference.do-ai.run/v1",
                    api_key,
                    model,
                    messages,
                    temperature,
                    max_tokens,
                ).await
            }
            Provider::Local { base_url } => {
                self.call_openai_compatible(
                    client,
                    base_url,
                    "",
                    model,
                    messages,
                    temperature,
                    max_tokens,
                ).await
            }
        }
    }

    async fn call_openai_compatible(
        &self,
        client: &Client,
        base_url: &str,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        max_tokens: Option<i32>,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", base_url);

        let mut body = json!({
            "model": model,
            "messages": messages,
        });

        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max) = max_tokens {
            body["max_tokens"] = json!(max);
        }

        debug!("Calling {} with model {}", base_url, model);

        let mut request = client.post(&url).json(&body);

        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request.send().await
            .map_err(|e| AppError::Provider(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Provider error ({}): {}", status, error_text);
            return Err(AppError::Provider(format!("Provider returned {}: {}", status, error_text)));
        }

        let response_json: ChatCompletionResponse = response.json().await
            .map_err(|e| AppError::Provider(format!("Failed to parse response: {}", e)))?;

        Ok(response_json)
    }

    async fn call_anthropic(
        &self,
        client: &Client,
        api_key: &str,
        model: &str,
        messages: &[ChatMessage],
        temperature: Option<f32>,
        max_tokens: Option<i32>,
    ) -> Result<ChatCompletionResponse> {
        let url = "https://api.anthropic.com/v1/messages";

        // Convert OpenAI-style messages to Anthropic format
        let system_msg = messages.iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        let anthropic_messages: Vec<Value> = messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({
                "role": if m.role == "assistant" { "assistant" } else { "user" },
                "content": m.content,
            }))
            .collect();

        let mut body = json!({
            "model": model,
            "messages": anthropic_messages,
            "max_tokens": max_tokens.unwrap_or(4096),
        });

        if let Some(system) = system_msg {
            body["system"] = json!(system);
        }
        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }

        debug!("Calling Anthropic API with model {}", model);

        let response = client.post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Anthropic error ({}): {}", status, error_text);
            return Err(AppError::Provider(format!("Anthropic returned {}: {}", status, error_text)));
        }

        let anthropic_response: Value = response.json().await
            .map_err(|e| AppError::Provider(format!("Failed to parse response: {}", e)))?;

        // Convert Anthropic response to OpenAI format
        let content = anthropic_response["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage_input = anthropic_response["usage"]["input_tokens"].as_i64().unwrap_or(0) as i32;
        let usage_output = anthropic_response["usage"]["output_tokens"].as_i64().unwrap_or(0) as i32;

        Ok(ChatCompletionResponse {
            id: anthropic_response["id"].as_str().unwrap_or("").to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: anthropic_response["stop_reason"].as_str().unwrap_or("stop").to_string(),
            }],
            usage: Usage {
                prompt_tokens: usage_input,
                completion_tokens: usage_output,
                total_tokens: usage_input + usage_output,
            },
            grpo_metadata: None,
        })
    }
}

pub fn get_available_models(config: &crate::config::Config) -> Vec<crate::models::ModelInfo> {
    let mut models = vec![];

    if config.deepseek_api_key.is_some() {
        models.extend(vec![
            crate::models::ModelInfo {
                id: "deepseek-chat".to_string(),
                object: "model".to_string(),
                created: 1704110400,
                owned_by: "deepseek".to_string(),
            },
            crate::models::ModelInfo {
                id: "deepseek-coder".to_string(),
                object: "model".to_string(),
                created: 1704110400,
                owned_by: "deepseek".to_string(),
            },
        ]);
    }

    if config.openai_api_key.is_some() {
        models.extend(vec![
            crate::models::ModelInfo {
                id: "gpt-4".to_string(),
                object: "model".to_string(),
                created: 1687882410,
                owned_by: "openai".to_string(),
            },
            crate::models::ModelInfo {
                id: "gpt-3.5-turbo".to_string(),
                object: "model".to_string(),
                created: 1677610602,
                owned_by: "openai".to_string(),
            },
        ]);
    }

    if config.anthropic_api_key.is_some() {
        models.extend(vec![
            crate::models::ModelInfo {
                id: "claude-3-opus-20240229".to_string(),
                object: "model".to_string(),
                created: 1709164800,
                owned_by: "anthropic".to_string(),
            },
            crate::models::ModelInfo {
                id: "claude-3-sonnet-20240229".to_string(),
                object: "model".to_string(),
                created: 1709164800,
                owned_by: "anthropic".to_string(),
            },
        ]);
    }

    // Always include local models
    models.extend(vec![
        crate::models::ModelInfo {
            id: "zen-nano".to_string(),
            object: "model".to_string(),
            created: 1704110400,
            owned_by: "hanzo".to_string(),
        },
        crate::models::ModelInfo {
            id: "zen-coder".to_string(),
            object: "model".to_string(),
            created: 1704110400,
            owned_by: "hanzo".to_string(),
        },
    ]);

    models
}
