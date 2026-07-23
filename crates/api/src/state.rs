//! Shared application state handed to every handler via `State`.

use std::sync::Arc;

use ferrite_queue::RedisQueue;
use ferrite_storage::Storage;
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;

use crate::config::Settings;
use crate::email::Mailer;

/// Cloneable handle to shared resources. Cheap to clone (inner `Arc`s / pools).
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    db: PgPool,
    storage: Storage,
    queue: RedisQueue,
    settings: Settings,
    metrics: PrometheusHandle,
    mailer: Mailer,
}

impl AppState {
    pub fn new(
        db: PgPool,
        storage: Storage,
        queue: RedisQueue,
        settings: Settings,
        metrics: PrometheusHandle,
        mailer: Mailer,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                db,
                storage,
                queue,
                settings,
                metrics,
                mailer,
            }),
        }
    }

    pub fn mailer(&self) -> &Mailer {
        &self.inner.mailer
    }

    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }

    pub fn storage(&self) -> &Storage {
        &self.inner.storage
    }

    pub fn queue(&self) -> &RedisQueue {
        &self.inner.queue
    }

    pub fn settings(&self) -> &Settings {
        &self.inner.settings
    }

    pub fn render_metrics(&self) -> String {
        self.inner.metrics.render()
    }
}
