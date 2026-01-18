use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,

    // Provider API keys
    pub deepseek_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub digitalocean_api_key: Option<String>,

    // Zoo-gym integration
    pub zoo_gym_path: String,
    pub experience_lib_path: String,

    // GRPO settings
    pub grpo_enabled: bool,
    pub grpo_group_size: usize,
    pub grpo_max_operations: usize,

    // Stripe
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,

    // Hanzo ID (OAuth/OIDC)
    pub hanzo_id_url: String,
    pub hanzo_id_client_id: Option<String>,
    pub hanzo_id_client_secret: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/hanzo_cloud".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "change-me-in-production".to_string()),

            // Providers
            deepseek_api_key: env::var("DEEPSEEK_API_KEY").ok(),
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            digitalocean_api_key: env::var("DIGITALOCEAN_API_KEY").ok(),

            // Zoo-gym
            zoo_gym_path: env::var("ZOO_GYM_PATH")
                .unwrap_or_else(|_| "/Users/z/work/zoo/gym".to_string()),
            experience_lib_path: env::var("EXPERIENCE_LIB_PATH")
                .unwrap_or_else(|_| "./experiences".to_string()),

            // GRPO
            grpo_enabled: env::var("GRPO_ENABLED")
                .map(|v| v == "true")
                .unwrap_or(true),
            grpo_group_size: env::var("GRPO_GROUP_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            grpo_max_operations: env::var("GRPO_MAX_OPERATIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),

            // Stripe
            stripe_secret_key: env::var("STRIPE_SECRET_KEY").ok(),
            stripe_publishable_key: env::var("STRIPE_PUBLISHABLE_KEY").ok(),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET").ok(),

            // Hanzo ID (OAuth/OIDC)
            hanzo_id_url: env::var("HANZO_ID_URL")
                .unwrap_or_else(|_| "https://hanzo.id".to_string()),
            hanzo_id_client_id: env::var("HANZO_ID_CLIENT_ID").ok(),
            hanzo_id_client_secret: env::var("HANZO_ID_CLIENT_SECRET").ok(),
        })
    }
}
