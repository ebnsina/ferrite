//! Prometheus metrics: HTTP request counters/latency, plus periodic gauges for
//! job states and workspace totals. Scrape at `GET /metrics`.

use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use sqlx::PgPool;

/// Install the global Prometheus recorder and return a render handle.
pub fn install() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("install prometheus recorder")
}

/// Middleware: count requests and record latency, labelled by the matched route
/// (not the raw path — keeps cardinality bounded).
pub async fn track(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().as_str().to_owned();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status)
        .increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path)
        .record(start.elapsed().as_secs_f64());
    response
}

/// Periodically refresh operational gauges from the database.
pub async fn collect_loop(pool: PgPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tick.tick().await;
        if let Err(e) = refresh(&pool).await {
            tracing::warn!(error = %e, "metrics collection failed");
        }
    }
}

async fn refresh(pool: &PgPool) -> Result<(), sqlx::Error> {
    let by_state: Vec<(String, i64)> =
        sqlx::query_as("SELECT state, count(*) FROM jobs GROUP BY state")
            .fetch_all(pool)
            .await?;
    for (state, count) in by_state {
        metrics::gauge!("ferrite_jobs", "state" => state).set(count as f64);
    }

    for (metric, sql) in [
        ("ferrite_tenants_total", "SELECT count(*) FROM tenants"),
        ("ferrite_assets_total", "SELECT count(*) FROM assets"),
        (
            "ferrite_live_streams_total",
            "SELECT count(*) FROM live_streams",
        ),
    ] {
        let (n,): (i64,) = sqlx::query_as(sql).fetch_one(pool).await?;
        metrics::gauge!(metric).set(n as f64);
    }
    Ok(())
}
