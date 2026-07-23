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
pub const SESSION_TTL_SECS: u64 = 7 * 24 * 60 * 60;

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
    #[serde(default)]
    superadmin: bool,
    exp: u64,
}

/// Whether an email is a configured platform superadmin.
pub fn is_superadmin(list: &Option<String>, email: &str) -> bool {
    let email = email.trim().to_lowercase();
    list.as_deref()
        .map(|l| l.split(',').any(|e| e.trim().to_lowercase() == email))
        .unwrap_or(false)
}

pub fn issue_session(
    secret: &str,
    user_id: Uuid,
    tenant_id: Uuid,
    role: &str,
    superadmin: bool,
) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let exp = now + SESSION_TTL_SECS;
    let claims = Claims {
        sub: user_id,
        tenant_id,
        role: role.to_string(),
        superadmin,
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
    /// Platform superadmin (cross-tenant admin). API keys are never superadmin.
    pub superadmin: bool,
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
        // Two credential sources. Programmatic clients send `Authorization:
        // Bearer` (API keys, or a JWT from the login body); browsers send the
        // HttpOnly session cookie. Header wins so a stray cookie can't shadow an
        // explicit key.
        let bearer = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty());

        let (token, from_cookie) = match bearer {
            Some(t) => (t.to_string(), false),
            None => (
                crate::cookies::read(&parts.headers, crate::cookies::SESSION_COOKIE)
                    .filter(|t| !t.is_empty())
                    .ok_or(ApiError::Unauthorized)?,
                true,
            ),
        };

        // Cookie auth is ambient, so mutating requests must prove they originate
        // from our own page via the double-submit CSRF token. Header requests
        // (Bearer) aren't CSRF-able and are exempt.
        if from_cookie && is_mutating(&parts.method) {
            verify_csrf(&parts.headers)?;
        }

        // API keys carry the frt_ prefix; everything else is a session JWT.
        if token.starts_with(KEY_PREFIX) {
            let key = db::find_active_api_key(state.db(), &hash_key(&token))
                .await?
                .ok_or(ApiError::Unauthorized)?;
            db::touch_api_key(state.db(), key.id).await;
            Ok(TenantContext {
                tenant_id: key.tenant_id,
                user_id: None,
                role: "service".to_string(),
                superadmin: false,
            })
        } else {
            let claims = decode_session(&state.settings().auth_secret, &token)
                .ok_or(ApiError::Unauthorized)?;
            Ok(TenantContext {
                tenant_id: claims.tenant_id,
                user_id: Some(claims.sub),
                role: claims.role,
                superadmin: claims.superadmin,
            })
        }
    }
}

fn is_mutating(method: &axum::http::Method) -> bool {
    !matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    )
}

/// Double-submit check: the `X-CSRF-Token` header must be present and equal the
/// `ferrite_csrf` cookie. A cross-site attacker can neither read that cookie
/// (same-origin policy) nor set a custom header without a blocked preflight.
fn verify_csrf(headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    let header = headers
        .get(crate::cookies::CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let cookie = crate::cookies::read(headers, crate::cookies::CSRF_COOKIE);
    match (header, cookie) {
        (Some(h), Some(c)) if h == c => Ok(()),
        _ => Err(ApiError::Forbidden),
    }
}
