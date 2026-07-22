//! Router assembly. Route modules are registered here; cross-cutting middleware
//! (tracing, CORS, body limits) and the global 404 fallback are applied once.

mod health;

use axum::routing::get;
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
        .route("/ready", get(health::ready));

    Router::new()
        .nest("/v1", api)
        // Also expose /health at the root for load-balancer defaults.
        .route("/health", get(health::health))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        // Guard against oversized JSON bodies (uploads go direct-to-S3, not here).
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state)
}
