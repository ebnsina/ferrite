//! Minimal cookie helpers for the dashboard session.
//!
//! The session JWT lives in an **HttpOnly** cookie so page JavaScript (and thus
//! any XSS) can never read it. A companion non-HttpOnly `csrf` cookie carries a
//! random token the SPA echoes back in the `X-CSRF-Token` header — the
//! double-submit pattern that defeats CSRF on cookie-authenticated mutations.

use axum::http::header::{HeaderMap, COOKIE};
use rand::RngCore;

pub const SESSION_COOKIE: &str = "ferrite_session";
pub const CSRF_COOKIE: &str = "ferrite_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";

/// A 256-bit random token, hex-encoded (used for the CSRF value).
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Read a single cookie value from the request's `Cookie` header.
pub fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k.trim() == name).then(|| v.trim().to_string())
    })
}

/// `Set-Cookie` value for the HttpOnly session cookie. `Secure` is toggled off
/// for plain-HTTP dev so the cookie is actually delivered on localhost.
pub fn set_session(token: &str, secure: bool, ttl_secs: u64) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl_secs}{}",
        secure_attr(secure)
    )
}

/// `Set-Cookie` value for the readable CSRF cookie (not HttpOnly — the SPA must
/// read it to mirror it into a request header).
pub fn set_csrf(token: &str, secure: bool, ttl_secs: u64) -> String {
    format!(
        "{CSRF_COOKIE}={token}; Path=/; SameSite=Lax; Max-Age={ttl_secs}{}",
        secure_attr(secure)
    )
}

/// `Set-Cookie` value that immediately expires a cookie (logout).
pub fn clear(name: &str, secure: bool) -> String {
    format!(
        "{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_attr(secure)
    )
}

fn secure_attr(secure: bool) -> &'static str {
    if secure {
        "; Secure"
    } else {
        ""
    }
}
