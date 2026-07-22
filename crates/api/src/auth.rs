//! API-key authentication and tenant context.
//!
//! Keys look like `frt_<40 hex>`. Only a SHA-256 hash is stored; the plaintext
//! is shown to the caller exactly once at creation. Requests authenticate with
//! `Authorization: Bearer frt_...`, which resolves to a [`TenantContext`].

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db;
use crate::error::ApiError;
use crate::state::AppState;

const KEY_PREFIX: &str = "frt_";
/// Chars of the full key kept in clear for display (`frt_` + 8 hex).
const DISPLAY_PREFIX_LEN: usize = KEY_PREFIX.len() + 8;

/// A freshly minted key. `plaintext` is returned to the user once and never stored.
pub struct GeneratedKey {
    pub plaintext: String,
    pub hash: String,
    pub prefix: String,
}

/// Generate a new API key: 20 random bytes hex-encoded behind the `frt_` prefix.
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

/// SHA-256, hex-encoded. Deterministic so we can look keys up by hash.
pub fn hash_key(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    hex::encode(digest)
}

/// Identifies the authenticated tenant for a request.
#[derive(Debug, Clone, Copy)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub api_key_id: Uuid,
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

        let hash = hash_key(token);
        let key = db::find_active_api_key(state.db(), &hash)
            .await?
            .ok_or(ApiError::Unauthorized)?;

        // Fire-and-forget usage timestamp.
        db::touch_api_key(state.db(), key.id).await;

        Ok(TenantContext {
            tenant_id: key.tenant_id,
            api_key_id: key.id,
        })
    }
}
