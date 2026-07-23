//! Fair multi-tenant job queue on Redis: per-tenant pending lists fed
//! round-robin (in-flight-capped) into a durable work stream with a consumer
//! group. Prevents one tenant's 10,000 jobs from starving another's 1.

use std::time::Duration;

use ferrite_core::TranscodeJob;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::streams::{
    StreamAutoClaimOptions, StreamAutoClaimReply, StreamClaimOptions, StreamClaimReply,
    StreamPendingCountReply, StreamReadOptions, StreamReadReply,
};
use redis::{AsyncCommands, Script};
use thiserror::Error;
use uuid::Uuid;

/// How long a `claim` blocks server-side waiting for a job.
const CLAIM_BLOCK: Duration = Duration::from_secs(5);

const STREAM: &str = "ferrite:jobs";
const DLQ_STREAM: &str = "ferrite:jobs:dead";
const ACTIVE_ZSET: &str = "ferrite:active";
const SEQ_KEY: &str = "ferrite:seq";
const PENDING_PREFIX: &str = "ferrite:pending:";
const INFLIGHT_PREFIX: &str = "ferrite:inflight:";
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

/// A job claimed from the queue. Ack it once processing succeeds, or it will be
/// redelivered.
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
    ) -> impl std::future::Future<Output = Result<(), QueueError>> + Send;

    fn claim(
        &self,
        consumer: &str,
        block: Duration,
    ) -> impl std::future::Future<Output = Result<Option<ClaimedJob>, QueueError>> + Send;

    fn ack(
        &self,
        claimed: &ClaimedJob,
    ) -> impl std::future::Future<Output = Result<(), QueueError>> + Send;
}

#[derive(Clone)]
pub struct RedisQueue {
    conn: ConnectionManager,
    /// Separate connection for blocking claims so a 5s XREADGROUP can't stall
    /// enqueue/dispatch/ack on the shared connection.
    claim_conn: ConnectionManager,
    group: String,
    /// Max jobs a single tenant may have in-flight (in the work stream) at once.
    max_inflight_per_tenant: usize,
    /// Idle time before a pending (unacked) entry may be reclaimed for retry.
    reclaim_min_idle: Duration,
}

impl RedisQueue {
    /// Connect and ensure the consumer group exists (idempotent).
    pub async fn connect(
        redis_url: &str,
        group: &str,
        max_inflight_per_tenant: usize,
        reclaim_min_idle: Duration,
    ) -> Result<Self, QueueError> {
        let client = redis::Client::open(redis_url)?;

        // The response timeout must exceed the blocking XREADGROUP window, or a
        // `claim` that legitimately blocks waiting for work is killed as a timeout.
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(5)))
            .set_response_timeout(Some(CLAIM_BLOCK + Duration::from_secs(5)));

        let mut conn = ConnectionManager::new_with_config(client.clone(), config.clone()).await?;
        let claim_conn = ConnectionManager::new_with_config(client, config).await?;

        // Create the group + stream if missing; ignore BUSYGROUP (already exists).
        let created: redis::RedisResult<()> = conn.xgroup_create_mkstream(STREAM, group, "$").await;
        if let Err(e) = created {
            if e.code() != Some("BUSYGROUP") {
                return Err(e.into());
            }
        }

        Ok(Self {
            conn,
            claim_conn,
            group: group.to_string(),
            max_inflight_per_tenant: max_inflight_per_tenant.max(1),
            reclaim_min_idle,
        })
    }

    /// Cheap connectivity check for readiness probes.
    pub async fn ping(&self) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    /// Claim one entry that has been pending (unacked) longer than the idle
    /// threshold — a job whose worker died or that failed a prior attempt.
    /// `delivery_count` reflects the real XPENDING count, so retries progress.
    async fn reclaim(&self, consumer: &str) -> Result<Option<ClaimedJob>, QueueError> {
        let mut conn = self.claim_conn.clone();
        let min_idle = self.reclaim_min_idle.as_millis() as usize;
        let opts = StreamAutoClaimOptions::default().count(1);
        let reply: StreamAutoClaimReply = conn
            .xautoclaim_options(STREAM, &self.group, consumer, min_idle, "0-0", opts)
            .await?;

        let Some(entry) = reply.claimed.into_iter().next() else {
            return Ok(None);
        };
        let payload: String = entry
            .get(FIELD)
            .ok_or_else(|| QueueError::Malformed(format!("entry {} has no payload", entry.id)))?;
        let job: TranscodeJob = serde_json::from_str(&payload)?;
        let delivery_count = self.times_delivered(&entry.id).await.unwrap_or(1);

        Ok(Some(ClaimedJob {
            id: entry.id,
            job,
            delivery_count,
        }))
    }

    /// Refresh ownership of an in-progress entry so its idle clock resets and no
    /// other worker reclaims it. `JUSTID` avoids bumping the delivery counter.
    /// Call periodically (interval < reclaim_min_idle) while processing.
    pub async fn heartbeat(&self, consumer: &str, entry_id: &str) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let opts = StreamClaimOptions::default().idle(0).with_justid();
        let _: StreamClaimReply = conn
            .xclaim_options(STREAM, &self.group, consumer, 0, &[entry_id], opts)
            .await?;
        Ok(())
    }

    /// How many times an entry has been delivered (via XPENDING).
    async fn times_delivered(&self, id: &str) -> Result<usize, QueueError> {
        let mut conn = self.claim_conn.clone();
        let reply: StreamPendingCountReply =
            conn.xpending_count(STREAM, &self.group, id, id, 1).await?;
        Ok(reply.ids.first().map(|p| p.times_delivered).unwrap_or(1))
    }

    /// Dispatch one job from the least-recently-served under-cap tenant into the
    /// work stream (atomic). Returns true if a job moved. At-cap tenants are
    /// skipped, not dropped, so they resume when capacity frees.
    pub async fn dispatch_tick(&self) -> Result<bool, QueueError> {
        let mut conn = self.conn.clone();
        let dispatched: i64 = DISPATCH_SCRIPT
            .key(ACTIVE_ZSET)
            .key(SEQ_KEY)
            .key(STREAM)
            .arg(self.max_inflight_per_tenant)
            .arg(INFLIGHT_PREFIX)
            .arg(PENDING_PREFIX)
            .arg(FIELD)
            .invoke_async(&mut conn)
            .await?;
        Ok(dispatched == 1)
    }

    /// Move a poison job to the dead-letter stream, ack the original, and release
    /// the tenant's in-flight slot.
    pub async fn dead_letter(&self, claimed: &ClaimedJob) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let payload = serde_json::to_string(&claimed.job)?;
        let _: String = conn
            .xadd(DLQ_STREAM, "*", &[(FIELD, payload.as_str())])
            .await?;
        conn.xack::<_, _, _, ()>(STREAM, &self.group, &[claimed.id.as_str()])
            .await?;
        self.release_inflight(claimed.job.tenant_id).await?;
        Ok(())
    }

    /// Decrement a tenant's in-flight counter, flooring at zero.
    async fn release_inflight(&self, tenant_id: Uuid) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        RELEASE_SCRIPT
            .arg(INFLIGHT_PREFIX)
            .arg(tenant_id.to_string())
            .invoke_async::<()>(&mut conn)
            .await?;
        Ok(())
    }
}

impl JobQueue for RedisQueue {
    async fn enqueue(&self, job: &TranscodeJob) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        let payload = serde_json::to_string(job)?;
        ENQUEUE_SCRIPT
            .key(ACTIVE_ZSET)
            .key(SEQ_KEY)
            .arg(PENDING_PREFIX)
            .arg(job.tenant_id.to_string())
            .arg(payload)
            .invoke_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    async fn claim(
        &self,
        consumer: &str,
        block: Duration,
    ) -> Result<Option<ClaimedJob>, QueueError> {
        // Prefer reclaiming a stuck entry (dead worker / prior failure) so retries
        // and dead-lettering actually progress; otherwise read a fresh entry.
        if let Some(job) = self.reclaim(consumer).await? {
            return Ok(Some(job));
        }

        let mut conn = self.claim_conn.clone();
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

    async fn ack(&self, claimed: &ClaimedJob) -> Result<(), QueueError> {
        let mut conn = self.conn.clone();
        conn.xack::<_, _, _, ()>(STREAM, &self.group, &[claimed.id.as_str()])
            .await?;
        self.release_inflight(claimed.job.tenant_id).await?;
        Ok(())
    }
}

// --- Lua scripts (atomic; safe under concurrent schedulers/replicas) ---------

use std::sync::LazyLock;

// KEYS[1]=active zset, KEYS[2]=seq; ARGV[1]=pending prefix, ARGV[2]=tenant, ARGV[3]=payload
static ENQUEUE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local active = KEYS[1]
local seq = KEYS[2]
local pending = ARGV[1] .. ARGV[2]
redis.call('RPUSH', pending, ARGV[3])
if redis.call('ZSCORE', active, ARGV[2]) == false then
  local s = redis.call('INCR', seq)
  redis.call('ZADD', active, s, ARGV[2])
end
return 1
"#,
    )
});

// KEYS[1]=active, KEYS[2]=seq, KEYS[3]=stream;
// ARGV[1]=max_inflight, ARGV[2]=inflight prefix, ARGV[3]=pending prefix, ARGV[4]=field
static DISPATCH_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local active = KEYS[1]
local seq = KEYS[2]
local stream = KEYS[3]
local max = tonumber(ARGV[1])
local inflp = ARGV[2]
local pendp = ARGV[3]
local field = ARGV[4]
local tenants = redis.call('ZRANGE', active, 0, -1)
for _, t in ipairs(tenants) do
  local infl = tonumber(redis.call('GET', inflp .. t) or '0')
  if infl < max then
    local pending = pendp .. t
    local payload = redis.call('LPOP', pending)
    if payload then
      redis.call('INCR', inflp .. t)
      redis.call('XADD', stream, '*', field, payload)
      local s = redis.call('INCR', seq)
      if redis.call('LLEN', pending) > 0 then
        redis.call('ZADD', active, s, t)
      else
        redis.call('ZREM', active, t)
      end
      return 1
    else
      redis.call('ZREM', active, t)
    end
  end
end
return 0
"#,
    )
});

// ARGV[1]=inflight prefix, ARGV[2]=tenant
static RELEASE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r#"
local key = ARGV[1] .. ARGV[2]
local v = redis.call('DECR', key)
if v < 0 then redis.call('SET', key, 0) end
return 1
"#,
    )
});
