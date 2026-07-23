//! Router assembly. Route modules are registered here; cross-cutting middleware
//! (tracing, CORS, body limits) and the global 404 fallback are applied once.

mod admin;
mod analytics;
mod assets;
mod brand;
mod health;
mod jobs;
mod live;
mod media;
mod members;
mod playback;
mod profile;
mod provenance;
mod search;
mod session;
mod tenants;
mod usage;
mod waitlist;
mod webhooks;

use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::error::not_found;
use crate::state::AppState;

/// OWASP baseline response headers, applied to every response.
async fn security_headers(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    // Don't let browsers MIME-sniff; block framing of API responses; trim the
    // referer on cross-origin; force HTTPS on any compliant client.
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );
    // Media/playback must remain loadable by cross-origin embed players.
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("cross-origin"),
    );
    res
}

/// Credentialed CORS for the dashboard: only the configured app origin may send
/// cookies. Falls back to permissive (dev) if the origin can't be parsed.
fn dashboard_cors(state: &AppState) -> CorsLayer {
    match state.settings().app_origin().parse::<HeaderValue>() {
        Ok(origin) => CorsLayer::new()
            .allow_origin(origin)
            .allow_credentials(true)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                CONTENT_TYPE,
                ACCEPT,
                AUTHORIZATION,
                HeaderName::from_static(crate::cookies::CSRF_HEADER),
            ]),
        Err(_) => {
            tracing::warn!(
                "FERRITE_APP_BASE_URL is not a valid CORS origin; using permissive CORS"
            );
            CorsLayer::permissive()
        }
    }
}

/// Build the full application router with shared state and middleware.
pub fn build(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/auth/signup", post(session::signup))
        .route("/auth/login", post(session::login))
        .route("/auth/forgot-password", post(session::forgot_password))
        .route("/auth/reset-password", post(session::reset_password))
        .route("/auth/logout", post(session::logout))
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
        .route("/admin/overview", get(admin::overview))
        .route("/admin/waitlist", get(admin::waitlist))
        .route("/brand", get(brand::get_brand))
        .route("/brand/logo", post(brand::upload_logo))
        .route(
            "/profile",
            get(profile::get_profile).patch(profile::update_profile),
        )
        .route("/profile/password", post(profile::change_password))
        .route("/search", get(search::search))
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
        .route("/assets/{id}/shorts", post(assets::shorts_asset))
        .route(
            "/assets/{id}/provenance",
            get(provenance::get_asset_provenance),
        )
        .route("/assets/{id}/moderation", get(provenance::get_moderation))
        .route("/provenance/key", get(provenance::public_key))
        .route("/jobs", get(jobs::list_jobs).post(jobs::create_job))
        .route("/jobs/batch", post(jobs::create_jobs_batch))
        .route("/jobs/{id}", get(jobs::get_job))
        .route("/jobs/{id}/events", get(jobs::job_events))
        .route("/jobs/{id}/transcript", get(jobs::job_transcript))
        .route("/jobs/{id}/analytics", get(analytics::job_analytics))
        .route("/jobs/{id}/embed", get(analytics::job_embed))
        .route("/jobs/{id}/translate", post(jobs::translate_captions))
        .route(
            "/live/streams",
            get(live::list_streams).post(live::create_stream),
        )
        .route("/live/streams/{id}", get(live::get_stream))
        .route("/live/streams/{id}/clip", post(live::clip_live))
        .route(
            "/live/streams/{id}/targets",
            get(live::list_targets).post(live::create_target),
        )
        .route(
            "/live/streams/{id}/targets/{target_id}",
            axum::routing::delete(live::delete_target),
        );

    // Dashboard (/v1) is cookie-authenticated → credentialed CORS to the app
    // origin only. Applied to the nested router so it isn't double-wrapped by
    // the public CORS below.
    let api = api.layer(dashboard_cors(&state));

    // Public routes: token- or secret-gated (embed player, media, waitlist,
    // ingest hooks, metrics) → permissive CORS, no credentials, any origin.
    let public = Router::new()
        // Also expose /health at the root for load-balancer defaults.
        .route("/health", get(health::health))
        // Token-authorized playback proxy (not API-key auth; scoped by signed token).
        .route("/playback/{job_id}/{*path}", get(playback::serve))
        // Signed, embeddable derived media (thumbnails + previews).
        .route("/media/{asset_id}/thumbnail", get(media::thumbnail))
        .route("/media/{asset_id}/preview", get(media::preview))
        // Public analytics beacon from the embed player (token-attributed).
        .route("/playback/beacon", post(analytics::beacon))
        // Public early-access waitlist.
        .route("/waitlist", post(waitlist::join))
        // Ingest-server DVR callback (secret-gated) → archive live recording to VOD.
        .route("/internal/live/dvr", post(live::dvr_hook))
        // Ingest-server publish hooks (secret-gated) → start/stop simulcast relays.
        .route("/internal/live/publish", post(live::publish_hook))
        .route("/internal/live/unpublish", post(live::unpublish_hook))
        // Prometheus scrape endpoint.
        .route(
            "/metrics",
            get(|state: axum::extract::State<AppState>| async move { state.render_metrics() }),
        )
        .layer(CorsLayer::permissive());

    Router::new()
        .nest("/v1", api)
        .merge(public)
        .fallback(not_found)
        // Non-CORS cross-cutting layers apply to both groups.
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(crate::metrics::track))
        .layer(TraceLayer::new_for_http())
        // Guard against oversized JSON bodies (uploads go direct-to-S3, not here).
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state)
}
