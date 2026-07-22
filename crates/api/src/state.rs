//! Shared application state handed to every handler via `State`.

use std::sync::Arc;

use ferrite_queue::RedisQueue;
use ferrite_storage::Storage;
use sqlx::PgPool;

use crate::config::Settings;

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
}

impl AppState {
    pub fn new(db: PgPool, storage: Storage, queue: RedisQueue, settings: Settings) -> Self {
        Self {
            inner: Arc::new(Inner {
                db,
                storage,
                queue,
                settings,
            }),
        }
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
}
