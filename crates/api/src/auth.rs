//! Auth: dashboard sessions (email/password → JWT) and programmatic API keys.
//! Both resolve a request to a [`TenantContext`].

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db;
use crate::error::ApiError;
use crate::state::AppState;

const KEY_PREFIX: &str = "frt_";
const DISPLAY_PREFIX_LEN: usize = KEY_PREFIX.len() + 8;
const SESSION_TTL_SECS: u64 = 7 * 24 * 60 * 60;

// --- Passwords ---------------------------------------------------------------

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("password hash: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

// --- Session JWTs ------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid, // user id
    tenant_id: Uuid,
    role: String,
    exp: u64,
}

pub fn issue_session(secret: &str, user_id: Uuid, tenant_id: Uuid, role: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let exp = now + SESSION_TTL_SECS;
    let claims = Claims {
        sub: user_id,
        tenant_id,
        role: role.to_string(),
        exp,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("jwt encode")
}

fn decode_session(secret: &str, token: &str) -> Option<Claims> {
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

// --- API keys ----------------------------------------------------------------

/// A freshly minted key. `plaintext` is returned once and never stored.
pub struct GeneratedKey {
    pub plaintext: String,
    pub hash: String,
    pub prefix: String,
}

pub fn generate_key() -> GeneratedKey {
    let mut bytes = [0u8; 20];
    rand::rng().fill_bytes(&mut bytes);
    let plaintext = format!("{KEY_PREFIX}{}", hex::encode(bytes));
    let hash = hash_key(&plaintext);
    let prefix = plaintext[..DISPLAY_PREFIX_LEN].to_string();
    GeneratedKey {
        plaintext,
        hash,
        prefix,
    }
}

/// SHA-256, hex-encoded — deterministic so keys are looked up by hash.
pub fn hash_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

// --- Request context ---------------------------------------------------------

/// The authenticated principal for a request (a dashboard user or an API key).
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    /// Present for dashboard-user sessions; `None` for API keys.
    pub user_id: Option<Uuid>,
    pub role: String,
}

impl TenantContext {
    pub fn is_owner(&self) -> bool {
        self.role == "owner"
    }
}

impl FromRequestParts<AppState> for TenantContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(ApiError::Unauthorized)?;

        // API keys carry the frt_ prefix; everything else is a session JWT.
        if let Some(rest) = token.strip_prefix(KEY_PREFIX) {
            let _ = rest;
            let key = db::find_active_api_key(state.db(), &hash_key(token))
                .await?
                .ok_or(ApiError::Unauthorized)?;
            db::touch_api_key(state.db(), key.id).await;
            Ok(TenantContext {
                tenant_id: key.tenant_id,
                user_id: None,
                role: "service".to_string(),
            })
        } else {
            let claims = decode_session(&state.settings().auth_secret, token)
                .ok_or(ApiError::Unauthorized)?;
            Ok(TenantContext {
                tenant_id: claims.tenant_id,
                user_id: Some(claims.sub),
                role: claims.role,
            })
        }
    }
}
