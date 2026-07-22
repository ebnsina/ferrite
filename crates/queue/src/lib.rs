//! Durable job queue built on Redis Streams + consumer groups.
//!
//! Guarantees at-least-once delivery: a claimed job stays in the group's
//! pending-entries list until it is explicitly acked. A dead worker's entries
//! remain pending and are reclaimable via `XAUTOCLAIM` (stale-entry reclaim is
//! planned for Phase 2). Poison messages that exceed the retry budget are moved
//! to a dead-letter stream.

use std::time::Duration;

use ferrite_core::TranscodeJob;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::AsyncCommands;
use thiserror::Error;

/// How long a `claim` blocks server-side waiting for a job.
const CLAIM_BLOCK: Duration = Duration::from_secs(5);

const STREAM: &str = "ferrite:jobs";
const DLQ_STREAM: &str = "ferrite:jobs:dead";
const FIELD: &str = "payload";

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("malformed queue message: {0}")]
    Malformed(String),
}

/// A job claimed from the queue. The `id` must be acked once processing
/// succeeds, or the message will be redelivered.
#[derive(Debug, Clone)]
pub struct ClaimedJob {
    /// Redis stream entry id (used for XACK).
    pub id: String,
    pub job: TranscodeJob,
    /// How many times this entry has been delivered (for retry decisions).
    pub delivery_count: usize,
}

/// Behaviour every queue backend provides. Lets us swap Redis for NATS
/// JetStream later without touching the API or worker.
pub trait JobQueue: Send + Sync {
    fn enqueue(
        &self,
        job: &TranscodeJob,
    ) -> impl std::future::Future<Output = Result<String, QueueError>> + Send;

    fn claim(
        &self,
        consumer: &str,
        block: Duration,
    ) -> impl std::future::Future<Output = Result<Option<ClaimedJob>, QueueError>> + Send;

    fn ack(&self, id: &str) -> impl std::future::Future<Output = Result<(), QueueError>> + Send;
}

#[derive(Clone)]
pub struct RedisQueue {
    conn: ConnectionManager,
    group: String,
}

impl RedisQueue {
    /// Connect and ensure the consumer group exists (idempotent).
    pub async fn connect(redis_url: &str, group: &str) -> Result<Self, QueueError> {
        let client = redis::Client::open(redis_url)?;

        // The response timeout must exceed the blocking XREADGROUP window, or a
        // `claim` that legitimately blocks waiting for work is killed as a timeout.
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(5)))
            .set_response_timeout(Some(CLAIM_BLOCK + Duration::from_secs(5)));

        let mut conn = ConnectionManager::new_with_config(client, config).await?;

        // Create the group + stream if missing; ignore BUSYGROUP (already exists).
        let created: redis::RedisResult<()> = conn.xgroup_create_mkstream(STREAM, group, "$").await;
        if let Err(e) = created {
            let is_busygroup = e.code() == Some("BUSYGROUP");
            if !is_busygroup {
                return Err(e.into());
            }
        }

        Ok(Self {
            conn,
            group: group.to_string(),
        })
    }

    /// Cheap connectivity check for readiness probes.
    pub async fn ping(&self) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    /// Move a poison job to the dead-letter stream and ack the original so it
    /// stops being redelivered.
    pub async fn dead_letter(&self, id: &str, job: &TranscodeJob) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let payload = serde_json::to_string(job)?;
        let _: String = conn
            .xadd(DLQ_STREAM, "*", &[(FIELD, payload.as_str())])
            .await?;
        conn.xack::<_, _, _, ()>(STREAM, &self.group, &[id]).await?;
        Ok(())
    }
}

impl JobQueue for RedisQueue {
    async fn enqueue(&self, job: &TranscodeJob) -> Result<String, QueueError> {
        let mut conn = self.conn.clone();
        let payload = serde_json::to_string(job)?;
        let id: String = conn.xadd(STREAM, "*", &[(FIELD, payload.as_str())]).await?;
        Ok(id)
    }

    async fn claim(
        &self,
        consumer: &str,
        block: Duration,
    ) -> Result<Option<ClaimedJob>, QueueError> {
        let mut conn = self.conn.clone();
        let opts = StreamReadOptions::default()
            .group(&self.group, consumer)
            .count(1)
            .block(block.as_millis() as usize);

        let reply: StreamReadReply = conn.xread_options(&[STREAM], &[">"], &opts).await?;

        for key in reply.keys {
            for entry in key.ids {
                let payload: String = entry.get(FIELD).ok_or_else(|| {
                    QueueError::Malformed(format!("entry {} has no payload", entry.id))
                })?;
                let job: TranscodeJob = serde_json::from_str(&payload)?;
                return Ok(Some(ClaimedJob {
                    id: entry.id,
                    job,
                    delivery_count: 1,
                }));
            }
        }
        Ok(None)
    }

    async fn ack(&self, id: &str) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        conn.xack::<_, _, _, ()>(STREAM, &self.group, &[id]).await?;
        Ok(())
    }
}
