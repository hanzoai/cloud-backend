use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ============================================================================
// Authentication Models
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub name: Option<String>,
    pub credits: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Inference Models (OpenAI-compatible)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<i32>,
    #[serde(default)]
    pub stream: bool,

    // GRPO-specific fields
    #[serde(default)]
    pub grpo_enabled: bool,
    #[serde(default)]
    pub groundtruth: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Usage,

    // GRPO-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpo_metadata: Option<GrpoMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: i32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GrpoMetadata {
    pub experiences_used: Vec<String>,
    pub group_size: usize,
    pub best_reward: f64,
    pub avg_reward: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// ============================================================================
// GRPO Models
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TrainGrpoRequest {
    pub queries: Vec<String>,
    pub groundtruths: Option<Vec<String>>,
    pub model: String,
    #[serde(default = "default_group_size")]
    pub group_size: usize,
    #[serde(default = "default_use_groundtruth")]
    pub use_groundtruth: bool,
}

fn default_group_size() -> usize { 5 }
fn default_use_groundtruth() -> bool { true }

#[derive(Debug, Serialize, Deserialize)]
pub struct TrainGrpoResponse {
    pub experiences_updated: i32,
    pub total_operations: i32,
    pub avg_improvement: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Experience {
    pub id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub usage_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsolidateRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsolidateResponse {
    pub operations_applied: i32,
    pub experiences_before: i32,
    pub experiences_after: i32,
}

// ============================================================================
// Usage & Billing Models
// ============================================================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct UsageRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub model: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub cost_credits: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageResponse {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: i64,
    pub by_model: Vec<ModelUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreditsResponse {
    pub credits: i64,
    pub currency: String,
}
