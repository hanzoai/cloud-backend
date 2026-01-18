use axum::{
    extract::{Path, State},
    Json,
};
use axum_extra::{
    TypedHeader,
    headers::Authorization,
    headers::authorization::Bearer,
};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::models::{Experience, ConsolidateRequest, ConsolidateResponse, TrainGrpoRequest, TrainGrpoResponse};
use crate::routes::auth::extract_user_id;
use crate::state::AppState;

pub async fn train_with_grpo(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(req): Json<TrainGrpoRequest>,
) -> Result<Json<TrainGrpoResponse>> {
    let user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &state.config.jwt_secret).await?;

    info!("Training GRPO for user {} with {} queries", user_id, req.queries.len());

    // TODO: Implement batch training logic
    // This would involve:
    // 1. Generate multiple rollouts for each query
    // 2. Extract semantic advantages
    // 3. Consolidate and update experience library

    Ok(Json(TrainGrpoResponse {
        experiences_updated: 0,
        total_operations: 0,
        avg_improvement: 0.0,
    }))
}

pub async fn list_experiences(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<Vec<Experience>>> {
    let user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &state.config.jwt_secret).await?;

    info!("Listing experiences for user {}", user_id);

    // TODO: Get user's model preference and list experiences
    let model = "deepseek-chat"; // Default model
    let experiences = state.grpo_manager.list_experiences(model)?;

    let experience_list: Vec<Experience> = experiences
        .into_iter()
        .map(|(id, content)| Experience {
            id,
            content,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            usage_count: 0,
        })
        .collect();

    Ok(Json(experience_list))
}

pub async fn get_experience(
    State(_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(id): Path<String>,
) -> Result<Json<Experience>> {
    let _user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &_state.config.jwt_secret).await?;

    info!("Getting experience {} for user {}", id, _user_id);

    // TODO: Implement get single experience
    Ok(Json(Experience {
        id: id.clone(),
        content: "Experience content".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        usage_count: 0,
    }))
}

pub async fn consolidate_experiences(
    State(_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(_req): Json<ConsolidateRequest>,
) -> Result<Json<ConsolidateResponse>> {
    let _user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &_state.config.jwt_secret).await?;

    info!("Consolidating experiences for user {}", _user_id);

    // TODO: Implement consolidation logic
    Ok(Json(ConsolidateResponse {
        operations_applied: 0,
        experiences_before: 0,
        experiences_after: 0,
    }))
}
