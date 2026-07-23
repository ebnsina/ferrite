//! Simulcast relay: while a stream is publishing to the ingest server, fan it
//! out to external RTMP destinations with one `ffmpeg -c copy` process each.
//! Processes are keyed by stream key and torn down on unpublish.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct RelayManager {
    inner: Arc<Mutex<HashMap<String, Vec<Child>>>>,
}

impl RelayManager {
    /// Start relaying `pull_url` to each `(url, key)` destination. Replaces any
    /// existing relays for this stream key (idempotent on republish).
    pub async fn start(&self, stream_key: &str, pull_url: &str, targets: Vec<(String, String)>) {
        self.stop(stream_key).await;
        if targets.is_empty() {
            return;
        }

        let mut children = Vec::new();
        for (url, key) in targets {
            let dest = format!("{}/{}", url.trim_end_matches('/'), key);
            match Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-i",
                    pull_url,
                    "-c",
                    "copy",
                    "-f",
                    "flv",
                    &dest,
                ])
                .kill_on_drop(true)
                .spawn()
            {
                Ok(child) => {
                    tracing::info!(stream = %stream_key, dest = %url, "simulcast relay started");
                    children.push(child);
                }
                Err(e) => tracing::error!(error = %e, "failed to spawn simulcast relay"),
            }
        }
        if !children.is_empty() {
            self.inner
                .lock()
                .await
                .insert(stream_key.to_string(), children);
        }
    }

    /// Kill all relays for a stream key.
    pub async fn stop(&self, stream_key: &str) {
        if let Some(mut children) = self.inner.lock().await.remove(stream_key) {
            for child in &mut children {
                let _ = child.start_kill();
            }
            tracing::info!(stream = %stream_key, n = children.len(), "simulcast relays stopped");
        }
    }
}
