//! Public early-access waitlist. No auth — anyone can join; doubles as market
//! research (country, use case, expected volume, preferred payment).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct WaitlistRequest {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub whatsapp: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub use_case: Option<String>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
}

fn clip(s: Option<String>, max: usize) -> Option<String> {
    s.map(|v| v.trim().chars().take(max).collect::<String>())
        .filter(|v| !v.is_empty())
}

/// `POST /waitlist` — join the early-access list.
pub async fn join(State(state): State<AppState>, Json(b): Json<WaitlistRequest>) -> StatusCode {
    let name: String = b.name.trim().chars().take(120).collect();
    let email = b.email.trim().to_lowercase();
    if name.is_empty() || !email.contains('@') || email.len() > 254 {
        return StatusCode::BAD_REQUEST;
    }
    let res = crate::db::insert_waitlist(
        state.db(),
        &name,
        &email,
        clip(b.whatsapp, 40).as_deref(),
        clip(b.country, 60).as_deref(),
        clip(b.use_case, 1000).as_deref(),
        clip(b.volume, 60).as_deref(),
        clip(b.plan, 60).as_deref(),
        clip(b.payment, 60).as_deref(),
    )
    .await;
    match res {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
