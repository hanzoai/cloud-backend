use axum::{
    extract::{State, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

mod config;
mod error;
mod grpo;
mod models;
mod providers;
mod routes;
mod state;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hanzo_cloud=debug,tower_http=debug".into())
        )
        .init();

    info!("Starting Hanzo Cloud Backend");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Initialize application state
    let state = Arc::new(AppState::new(config).await?);
    info!("Application state initialized");

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(routes::health::health_check))

        // Authentication
        .route("/v1/auth/register", post(routes::auth::register))
        .route("/v1/auth/login", post(routes::auth::login))
        .route("/v1/auth/refresh", post(routes::auth::refresh_token))

        // Inference endpoints
        .route("/v1/chat/completions", post(routes::inference::chat_completions))
        .route("/v1/models", get(routes::inference::list_models))

        // GRPO endpoints
        .route("/v1/grpo/train", post(routes::grpo::train_with_grpo))
        .route("/v1/grpo/experiences", get(routes::grpo::list_experiences))
        .route("/v1/grpo/experiences/:id", get(routes::grpo::get_experience))
        .route("/v1/grpo/consolidate", post(routes::grpo::consolidate_experiences))

        // Usage & Billing
        .route("/v1/usage", get(routes::usage::get_usage))
        .route("/v1/credits", get(routes::usage::get_credits))

        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let addr = format!("{}:{}",
        std::env::var("CLOUD_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
        std::env::var("CLOUD_PORT").unwrap_or_else(|_| "8001".to_string())
    );

    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
