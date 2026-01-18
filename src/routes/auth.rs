use axum::{extract::State, Json};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,  // User ID
    exp: i64,     // Expiration time
    iat: i64,     // Issued at
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    // Check if user exists
    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Email already registered".to_string()));
    }

    // Hash password
    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {}", e)))?;

    // Create user
    let user: User = sqlx::query_as(
        "INSERT INTO users (id, email, password_hash, name, credits)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *"
    )
    .bind(Uuid::new_v4())
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.name)
    .bind(1000i64) // Initial credits: 1000 (= $10 if 1 credit = 1 cent)
    .fetch_one(&state.db)
    .await?;

    // Generate tokens
    let tokens = generate_tokens(&state.config.jwt_secret, &user.id)?;

    Ok(Json(tokens))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    // Find user
    let user: User = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Auth("Invalid credentials".to_string()))?;

    // Verify password
    let valid = verify(&req.password, &user.password_hash)
        .map_err(|e| AppError::Internal(format!("Failed to verify password: {}", e)))?;

    if !valid {
        return Err(AppError::Auth("Invalid credentials".to_string()));
    }

    // Generate tokens
    let tokens = generate_tokens(&state.config.jwt_secret, &user.id)?;

    Ok(Json(tokens))
}

pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Json(refresh): Json<serde_json::Value>,
) -> Result<Json<AuthResponse>> {
    let refresh_token = refresh["refresh_token"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("Missing refresh_token".to_string()))?;

    // Decode and validate refresh token
    let token_data = decode::<Claims>(
        refresh_token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| AppError::Auth(format!("Invalid refresh token: {}", e)))?;

    let user_id = Uuid::parse_str(&token_data.claims.sub)
        .map_err(|e| AppError::Auth(format!("Invalid user ID: {}", e)))?;

    // Generate new tokens
    let tokens = generate_tokens(&state.config.jwt_secret, &user_id)?;

    Ok(Json(tokens))
}

fn generate_tokens(jwt_secret: &str, user_id: &Uuid) -> Result<AuthResponse> {
    let now = Utc::now();
    let access_exp = now + Duration::hours(1);
    let refresh_exp = now + Duration::days(7);

    let access_claims = Claims {
        sub: user_id.to_string(),
        exp: access_exp.timestamp(),
        iat: now.timestamp(),
    };

    let refresh_claims = Claims {
        sub: user_id.to_string(),
        exp: refresh_exp.timestamp(),
        iat: now.timestamp(),
    };

    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to generate access token: {}", e)))?;

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to generate refresh token: {}", e)))?;

    Ok(AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: 3600,
    })
}

// Helper to extract user ID from Authorization header
pub async fn extract_user_id(
    auth_header: Option<&str>,
    jwt_secret: &str,
) -> Result<Uuid> {
    let token = auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Auth("Missing or invalid Authorization header".to_string()))?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| AppError::Auth(format!("Invalid token: {}", e)))?;

    Uuid::parse_str(&token_data.claims.sub)
        .map_err(|e| AppError::Auth(format!("Invalid user ID: {}", e)))
}
