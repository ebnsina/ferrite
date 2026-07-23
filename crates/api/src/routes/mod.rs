//! Router assembly. Route modules are registered here; cross-cutting middleware
//! (tracing, CORS, body limits) and the global 404 fallback are applied once.

mod assets;
mod health;
mod jobs;
mod live;
mod members;
mod playback;
mod profile;
mod session;
mod tenants;
mod usage;
mod webhooks;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::error::not_found;
use crate::state::AppState;

/// Build the full application router with shared state and middleware.
pub fn build(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/auth/signup", post(session::signup))
        .route("/auth/login", post(session::login))
        .route("/auth/forgot-password", post(session::forgot_password))
        .route("/auth/reset-password", post(session::reset_password))
        .route(
            "/api-keys",
            get(tenants::list_api_keys).post(tenants::create_api_key),
        )
        .route(
            "/api-keys/{id}",
            axum::routing::delete(tenants::revoke_api_key),
        )
        .route(
            "/members",
            get(members::list_members).post(members::invite_member),
        )
        .route(
            "/members/{id}",
            axum::routing::patch(members::update_member).delete(members::remove_member),
        )
        .route("/me", get(tenants::me))
        .route(
            "/profile",
            get(profile::get_profile).patch(profile::update_profile),
        )
        .route("/profile/password", post(profile::change_password))
        .route("/usage", get(usage::get_usage))
        .route(
            "/webhooks",
            get(webhooks::list_webhooks).post(webhooks::create_webhook),
        )
        .route(
            "/webhooks/{id}",
            axum::routing::delete(webhooks::delete_webhook),
        )
        .route(
            "/assets",
            get(assets::list_assets).post(assets::create_asset),
        )
        .route("/assets/{id}", get(assets::get_asset))
        .route("/assets/{id}/complete", post(assets::complete_asset))
        .route("/assets/{id}/clip", post(assets::clip_asset))
        .route("/jobs", get(jobs::list_jobs).post(jobs::create_job))
        .route("/jobs/batch", post(jobs::create_jobs_batch))
        .route("/jobs/{id}", get(jobs::get_job))
        .route("/jobs/{id}/events", get(jobs::job_events))
        .route(
            "/live/streams",
            get(live::list_streams).post(live::create_stream),
        )
        .route("/live/streams/{id}", get(live::get_stream));

    Router::new()
        .nest("/v1", api)
        // Also expose /health at the root for load-balancer defaults.
        .route("/health", get(health::health))
        // Token-authorized playback proxy (not API-key auth; scoped by signed token).
        .route("/playback/{job_id}/{*path}", get(playback::serve))
        // Ingest-server DVR callback (secret-gated) → archive live recording to VOD.
        .route("/internal/live/dvr", post(live::dvr_hook))
        // Prometheus scrape endpoint.
        .route(
            "/metrics",
            get(|state: axum::extract::State<AppState>| async move { state.render_metrics() }),
        )
        .fallback(not_found)
        .layer(axum::middleware::from_fn(crate::metrics::track))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        // Guard against oversized JSON bodies (uploads go direct-to-S3, not here).
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state)
}
