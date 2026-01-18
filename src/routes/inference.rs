use axum::{
    extract::State,
    Json,
};
use axum_extra::{
    TypedHeader,
    headers::Authorization,
    headers::authorization::Bearer,
};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, ChatChoice, ChatMessage, ModelInfo, Usage};
use crate::providers::{Provider, get_available_models};
use crate::routes::auth::extract_user_id;
use crate::state::AppState;

pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>> {
    let models = get_available_models(&state.config);

    Ok(Json(serde_json::json!({
        "object": "list",
        "data": models,
    })))
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>> {
    // Extract user ID from token
    let user_id = extract_user_id(Some(&format!("Bearer {}", auth.token())), &state.config.jwt_secret).await?;

    debug!("Chat request from user {} for model {}", user_id, req.model);

    // Check user credits
    let user: Option<(i64,)> = sqlx::query_as("SELECT credits FROM users WHERE id = $1")
        .bind(&user_id)
        .fetch_optional(&state.db)
        .await?;

    let credits = user.map(|(c,)| c).unwrap_or(0);
    if credits <= 0 {
        return Err(AppError::InsufficientCredits);
    }

    // Get provider for model
    let model_name = req.model.clone();
    let provider = Provider::from_model(&model_name, &state.config)?;

    // Generate response (with or without GRPO)
    let response = if req.grpo_enabled && state.config.grpo_enabled {
        info!("Generating with GRPO for user {}", user_id);
        generate_with_grpo(state.clone(), provider, req).await?
    } else {
        debug!("Generating without GRPO");
        provider
            .generate(
                &state.http_client,
                &req.model,
                &req.messages,
                req.temperature,
                req.max_tokens,
            )
            .await?
    };

    // Calculate cost (simplified: 1 token = 0.01 credits)
    let cost = response.usage.total_tokens as i64 / 100;

    // Deduct credits and record usage
    sqlx::query("UPDATE users SET credits = credits - $1 WHERE id = $2")
        .bind(cost)
        .bind(&user_id)
        .execute(&state.db)
        .await?;

    sqlx::query(
        "INSERT INTO usage_records (id, user_id, model, prompt_tokens, completion_tokens, total_tokens, cost_credits)
         VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(Uuid::new_v4())
    .bind(&user_id)
    .bind(&model_name)
    .bind(response.usage.prompt_tokens)
    .bind(response.usage.completion_tokens)
    .bind(response.usage.total_tokens)
    .bind(cost)
    .execute(&state.db)
    .await?;

    debug!("Response generated, cost: {} credits, remaining: {}", cost, credits - cost);

    Ok(Json(response))
}

async fn generate_with_grpo(
    state: Arc<AppState>,
    provider: Provider,
    req: ChatCompletionRequest,
) -> Result<ChatCompletionResponse> {
    // Extract the last user message as the query
    let query = req.messages.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .ok_or_else(|| AppError::BadRequest("No user message found".to_string()))?;

    // Create a closure for provider generation
    let model = req.model.clone();
    let temperature = req.temperature;
    let max_tokens = req.max_tokens;
    let http_client = state.http_client.clone();

    let provider_generate = move |_query: &str, messages: &[ChatMessage]| {
        let provider = provider.clone();
        let model = model.clone();
        let http_client = http_client.clone();
        let messages = messages.to_vec();
        let temperature = temperature;
        let max_tokens = max_tokens;

        Box::pin(async move {
            let response = provider
                .generate(&http_client, &model, &messages, temperature, max_tokens)
                .await?;
            Ok(response.choices[0].message.content.clone())
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>
    };

    // Generate with GRPO
    let (best_response, grpo_metadata) = state.grpo_manager
        .generate_with_grpo(
            query,
            &req.messages,
            &req.model,
            provider_generate,
            req.groundtruth.as_deref(),
        )
        .await?;

    // Estimate usage (rough estimate)
    let prompt_tokens = req.messages.iter()
        .map(|m| m.content.len() / 4)
        .sum::<usize>() as i32;
    let completion_tokens = (best_response.len() / 4) as i32;

    Ok(ChatCompletionResponse {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: req.model.clone(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: best_response,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        grpo_metadata: Some(grpo_metadata),
    })
}
