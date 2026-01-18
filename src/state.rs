use crate::config::Config;
use crate::grpo::GrpoManager;
use redis::Client as RedisClient;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PgPool,
    pub redis: Option<RedisClient>,
    pub http_client: reqwest::Client,
    pub grpo_manager: Arc<GrpoManager>,
}

impl AppState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        // Database connection pool
        let db = PgPoolOptions::new()
            .max_connections(10)
            .connect(&config.database_url)
            .await?;

        // Run migrations (optional - may fail on managed DB permissions)
        match sqlx::migrate!("./migrations").run(&db).await {
            Ok(_) => tracing::info!("Database migrations completed successfully"),
            Err(e) => {
                tracing::warn!("Migration failed (may need manual setup): {}", e);
                // Continue anyway - tables might already exist or be created manually
            }
        }

        // Redis client (optional)
        let redis = match RedisClient::open(config.redis_url.as_str()) {
            Ok(client) => {
                tracing::info!("Redis client connected");
                Some(client)
            }
            Err(e) => {
                tracing::warn!("Redis connection failed (continuing without caching): {}", e);
                None
            }
        };

        // HTTP client for provider APIs
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;

        // GRPO manager
        let grpo_manager = Arc::new(GrpoManager::new(
            config.zoo_gym_path.clone(),
            config.experience_lib_path.clone(),
            config.grpo_group_size,
            config.grpo_max_operations,
        )?);

        Ok(Self {
            config,
            db,
            redis,
            http_client,
            grpo_manager,
        })
    }
}
