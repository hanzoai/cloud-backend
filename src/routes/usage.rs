use axum::{
    extract::{State, TypedHeader},
    headers::Authorization,
    headers::authorization::Bearer,
    Json,
};
use std::sync::Arc;

use crate::error::Result;
use crate::models::{UsageResponse, CreditsResponse, ModelUsage};
use crate::routes::auth::extract_user_id;
use crate::state::AppState;

pub async fn get_usage(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<UsageResponse>> {
    let user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &state.config.jwt_secret).await?;

    // Get total usage
    let total: (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(total_tokens), 0), COALESCE(SUM(cost_credits), 0)
         FROM usage_records
         WHERE user_id = $1"
    )
    .bind(&user_id)
    .fetch_one(&state.db)
    .await?;

    // Get usage by model
    let by_model: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT model, COUNT(*), SUM(total_tokens), SUM(cost_credits)
         FROM usage_records
         WHERE user_id = $1
         GROUP BY model
         ORDER BY COUNT(*) DESC"
    )
    .bind(&user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(UsageResponse {
        total_requests: total.0,
        total_tokens: total.1,
        total_cost: total.2,
        by_model: by_model
            .into_iter()
            .map(|(model, requests, tokens, cost)| ModelUsage {
                model,
                requests,
                tokens,
                cost,
            })
            .collect(),
    }))
}

pub async fn get_credits(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<CreditsResponse>> {
    let user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &state.config.jwt_secret).await?;

    let credits: (i64,) = sqlx::query_as("SELECT credits FROM users WHERE id = $1")
        .bind(&user_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(CreditsResponse {
        credits: credits.0,
        currency: "USD".to_string(),
    }))
}
