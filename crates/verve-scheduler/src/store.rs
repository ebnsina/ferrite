//! `sched_db`. Every query the admission loop needs, and nothing else.

use crate::capacity::{FleetState, LaneLoad, LaneShares};
use crate::fairness::Candidate;
use crate::model::{Lane, NewWork, TenantBudget, TenantId, WorkId, WorkItem, WorkKind, WorkState};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

/// What went wrong talking to `sched_db`.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database said no.
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    /// A stored string is not one of the values the code knows.
    #[error("corrupt row {id}: {source}")]
    Corrupt {
        /// Which row.
        id: Uuid,
        /// What failed to parse.
        #[source]
        source: crate::model::ParseError,
    },
    /// The tenant has no budget row, so nothing can be admitted for it.
    #[error("tenant {0} has no budget")]
    NoBudget(TenantId),
    /// The tenant is suspended.
    #[error("tenant {0} is suspended")]
    Suspended(TenantId),
}

/// Result alias for store operations.
pub type Result<T, E = StoreError> = std::result::Result<T, E>;

/// A handle on `sched_db`.
#[derive(Debug, Clone)]
pub struct Store {
    pool: PgPool,
}

/// A submission's outcome. Dedupe means "already there" is a success.
#[derive(Debug, Clone, PartialEq)]
pub enum Submitted {
    /// A new row.
    Created(WorkItem),
    /// The dedupe key already existed; this is the original.
    Existing(WorkItem),
}

impl Submitted {
    /// The item, however it got there.
    pub fn item(&self) -> &WorkItem {
        match self {
            Self::Created(i) | Self::Existing(i) => i,
        }
    }
}

impl Store {
    /// Wrap an existing pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Connect and run migrations.
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPool::connect(url).await?;
        let store = Self::new(pool);
        store.migrate().await?;
        Ok(store)
    }

    /// Connect with every connection pinned to a freshly recreated `schema`.
    ///
    /// Test support. Two reasons it drops first: a suite sharing one database
    /// needs each test to see only its own rows, or sweeps like
    /// [`Store::requeue_stalled`] collide — and rows left by the *previous* run
    /// would otherwise occupy the fleet before this one starts.
    pub async fn connect_fresh_schema(url: &str, schema: &str) -> Result<Self> {
        if !schema
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(sqlx::Error::Configuration(
                format!("unsafe schema name {schema:?}").into(),
            )
            .into());
        }

        // Safe: the name was just checked against [A-Za-z0-9_].
        let bootstrap = PgPool::connect(url).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP SCHEMA IF EXISTS {schema} CASCADE"
        )))
        .execute(&bootstrap)
        .await?;
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&bootstrap)
            .await?;
        bootstrap.close().await;

        // A small pool: a whole test suite of these must not exhaust the
        // server's connection limit.
        let owned = schema.to_string();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .after_connect(move |conn, _| {
                let schema = owned.clone();
                Box::pin(async move {
                    sqlx::query(sqlx::AssertSqlSafe(format!("SET search_path TO {schema}")))
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(url)
            .await?;

        let store = Self::new(pool);
        store.migrate().await?;
        Ok(store)
    }

    /// Apply pending migrations.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(sqlx::Error::from)?;
        Ok(())
    }

    /// The underlying pool, for tests and health checks.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Set a tenant's limits. Called when a plan changes.
    pub async fn upsert_budget(
        &self,
        tenant_id: TenantId,
        max_concurrent_tasks: i32,
        rate_limit_per_min: i32,
        fairness_weight: f32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO tenant_budgets
               (tenant_id, max_concurrent_tasks, rate_limit_per_min, fairness_weight)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (tenant_id) DO UPDATE SET
               max_concurrent_tasks = EXCLUDED.max_concurrent_tasks,
               rate_limit_per_min   = EXCLUDED.rate_limit_per_min,
               fairness_weight      = EXCLUDED.fairness_weight,
               updated_at           = now()",
        )
        .bind(tenant_id)
        .bind(max_concurrent_tasks)
        .bind(rate_limit_per_min)
        .bind(fairness_weight)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Stop or resume admission for a tenant.
    pub async fn set_suspended(&self, tenant_id: TenantId, suspended: bool) -> Result<()> {
        sqlx::query(
            "UPDATE tenant_budgets SET suspended = $2, updated_at = now() WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .bind(suspended)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// A tenant's budget.
    pub async fn budget(&self, tenant_id: TenantId) -> Result<TenantBudget> {
        sqlx::query(
            "SELECT tenant_id, max_concurrent_tasks, in_flight, rate_limit_per_min
             FROM tenant_budgets WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| TenantBudget {
            tenant_id: row.get("tenant_id"),
            max_concurrent_tasks: row.get("max_concurrent_tasks"),
            in_flight: row.get("in_flight"),
            rate_limit_per_min: row.get("rate_limit_per_min"),
        })
        .ok_or(StoreError::NoBudget(tenant_id))
    }

    /// Queue an item.
    ///
    /// `fairness_weight` is copied onto the row, not looked up later, so a plan
    /// downgrade cannot retroactively demote work already queued.
    pub async fn submit(&self, new: &NewWork) -> Result<Submitted> {
        let mut tx = self.pool.begin().await?;

        let budget: Option<(f32, bool)> = sqlx::query_as(
            "SELECT fairness_weight, suspended FROM tenant_budgets WHERE tenant_id = $1",
        )
        .bind(new.tenant_id)
        .fetch_optional(&mut *tx)
        .await?;

        let (weight, suspended) = budget.ok_or(StoreError::NoBudget(new.tenant_id))?;
        if suspended {
            return Err(StoreError::Suspended(new.tenant_id));
        }

        let id = Uuid::now_v7();
        let inserted = sqlx::query(
            "INSERT INTO work
               (id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                state, dedupe_key)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', $9)
             ON CONFLICT (tenant_id, dedupe_key) DO NOTHING
             RETURNING id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                    state, workflow_id, dedupe_key, attempts, created_at,
                    admitted_at, finished_at",
        )
        .bind(id)
        .bind(new.tenant_id)
        .bind(new.kind.as_str())
        .bind(new.ref_id)
        .bind(&new.spec)
        .bind(new.lane.as_str())
        .bind(new.priority_key)
        .bind(weight)
        .bind(&new.dedupe_key)
        .fetch_optional(&mut *tx)
        .await?;

        let result = match inserted {
            Some(row) => Submitted::Created(work_from_row(&row)?),
            None => {
                let row = sqlx::query(
                    "SELECT id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                    state, workflow_id, dedupe_key, attempts, created_at,
                    admitted_at, finished_at FROM work WHERE tenant_id = $1 AND dedupe_key = $2",
                )
                .bind(new.tenant_id)
                .bind(&new.dedupe_key)
                .fetch_one(&mut *tx)
                .await?;
                Submitted::Existing(work_from_row(&row)?)
            }
        };

        tx.commit().await?;
        Ok(result)
    }

    /// One item by id.
    pub async fn get(&self, id: WorkId) -> Result<Option<WorkItem>> {
        let row = sqlx::query(
            "SELECT id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                    state, workflow_id, dedupe_key, attempts, created_at,
                    admitted_at, finished_at FROM work WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(work_from_row).transpose()
    }

    /// Items, filtered. Used by `verve work list` and the internal API.
    pub async fn list(
        &self,
        tenant_id: Option<TenantId>,
        state: Option<WorkState>,
        limit: i64,
    ) -> Result<Vec<WorkItem>> {
        let rows = sqlx::query(
            "SELECT id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                    state, workflow_id, dedupe_key, attempts, created_at,
                    admitted_at, finished_at FROM work
             WHERE ($1::uuid IS NULL OR tenant_id = $1)
               AND ($2::text IS NULL OR state = $2)
             ORDER BY created_at DESC
             LIMIT $3",
        )
        .bind(tenant_id)
        .bind(state.map(|s| s.as_str()))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(work_from_row).collect()
    }

    /// Per-lane running and pending counts, for the capacity planner.
    pub async fn fleet_state(&self, total_slots: u32, shares: LaneShares) -> Result<FleetState> {
        let rows = sqlx::query(
            "SELECT lane,
                    count(*) FILTER (WHERE state IN ('admitted', 'running')) AS running,
                    count(*) FILTER (WHERE state = 'pending') AS pending
             FROM work
             WHERE state IN ('pending', 'admitted', 'running')
             GROUP BY lane",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut state = FleetState {
            total_slots,
            lanes: Default::default(),
            shares,
        };
        for row in &rows {
            let lane: String = row.get("lane");
            let Ok(lane) = lane.parse::<Lane>() else {
                continue;
            };
            *state.lane_mut(lane) = LaneLoad {
                running: row.get::<i64, _>("running").max(0) as u32,
                pending: row.get::<i64, _>("pending").max(0) as u32,
            };
        }
        Ok(state)
    }

    /// Tenants with pending work in `lane` that are under every limit.
    ///
    /// Suspended tenants and tenants over their per-minute rate are excluded
    /// here rather than filtered later, so they never consume a fairness turn.
    pub async fn candidates(&self, lane: Lane) -> Result<Vec<Candidate>> {
        let rows = sqlx::query(
            "WITH pending AS (
               SELECT tenant_id, count(*) AS n, max(fairness_weight) AS weight
               FROM work WHERE state = 'pending' AND lane = $1
               GROUP BY tenant_id
             ),
             recent AS (
               SELECT tenant_id, count(*) AS n
               FROM work
               WHERE admitted_at > now() - interval '1 minute'
               GROUP BY tenant_id
             )
             SELECT p.tenant_id,
                    p.n AS pending,
                    p.weight,
                    b.max_concurrent_tasks - b.in_flight AS headroom,
                    b.rate_limit_per_min - coalesce(r.n, 0) AS rate_headroom
             FROM pending p
             JOIN tenant_budgets b ON b.tenant_id = p.tenant_id
             LEFT JOIN recent r ON r.tenant_id = p.tenant_id
             WHERE b.suspended = FALSE",
        )
        .bind(lane.as_str())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let headroom = row.get::<i32, _>("headroom").max(0) as u32;
                let rate_headroom = row.get::<i64, _>("rate_headroom").max(0) as u32;
                Candidate {
                    tenant_id: row.get("tenant_id"),
                    weight: row.get("weight"),
                    headroom: headroom.min(rate_headroom),
                    pending: row.get::<i64, _>("pending").max(0) as u32,
                }
            })
            .filter(|c| c.headroom > 0)
            .collect())
    }

    /// Take up to `count` pending items for one tenant and mark them admitted.
    ///
    /// `FOR UPDATE SKIP LOCKED` so several scheduler copies never hand the same
    /// row to two workers, and `in_flight` moves in the same transaction as the
    /// state change so the counter cannot disagree with reality.
    pub async fn claim(
        &self,
        tenant_id: TenantId,
        lane: Lane,
        count: u32,
    ) -> Result<Vec<WorkItem>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin().await?;

        let rows = sqlx::query(
            "WITH picked AS (
               SELECT id FROM work
               WHERE state = 'pending' AND tenant_id = $1 AND lane = $2
               ORDER BY priority_key, created_at
               LIMIT $3
               FOR UPDATE SKIP LOCKED
             )
             UPDATE work SET state = 'admitted', admitted_at = now(), attempts = attempts + 1
             WHERE id IN (SELECT id FROM picked)
             RETURNING id, tenant_id, kind, ref_id, spec, lane, priority_key, fairness_weight,
                    state, workflow_id, dedupe_key, attempts, created_at,
                    admitted_at, finished_at",
        )
        .bind(tenant_id)
        .bind(lane.as_str())
        .bind(i64::from(count))
        .fetch_all(&mut *tx)
        .await?;

        if !rows.is_empty() {
            sqlx::query(
                "UPDATE tenant_budgets SET in_flight = in_flight + $2, updated_at = now()
                 WHERE tenant_id = $1",
            )
            .bind(tenant_id)
            .bind(rows.len() as i32)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        // UPDATE ... RETURNING has no defined order; give callers the same one
        // the rows were picked in.
        let mut claimed: Vec<WorkItem> = rows.iter().map(work_from_row).collect::<Result<_>>()?;
        claimed.sort_unstable_by(|a, b| {
            a.priority_key
                .cmp(&b.priority_key)
                .then(a.created_at.cmp(&b.created_at))
        });
        Ok(claimed)
    }

    /// Record that the workflow started.
    pub async fn mark_running(&self, id: WorkId, workflow_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE work SET state = 'running', workflow_id = $2
             WHERE id = $1 AND state = 'admitted'",
        )
        .bind(id)
        .bind(workflow_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Move an item to a terminal state and give its slot back.
    ///
    /// The slot is only released if the row actually held one, so a repeated
    /// completion cannot drive `in_flight` below zero.
    pub async fn finish(&self, id: WorkId, state: WorkState, error: Option<&str>) -> Result<bool> {
        debug_assert!(state.is_terminal());
        let mut tx = self.pool.begin().await?;

        let released: Option<(Uuid,)> = sqlx::query_as(
            "UPDATE work SET state = $2, finished_at = now(), last_error = $3
             WHERE id = $1 AND state NOT IN ('done', 'failed', 'canceled')
             RETURNING tenant_id",
        )
        .bind(id)
        .bind(state.as_str())
        .bind(error)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((tenant_id,)) = released else {
            tx.commit().await?;
            return Ok(false);
        };

        sqlx::query(
            "UPDATE tenant_budgets SET in_flight = greatest(in_flight - 1, 0), updated_at = now()
             WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Put one item back in the queue and give its slot back.
    ///
    /// Used when starting the workflow failed: the work is not lost, and the
    /// attempt is already counted so a permanently broken item eventually fails.
    pub async fn release_to_pending(&self, id: WorkId) -> Result<bool> {
        let mut tx = self.pool.begin().await?;

        let released: Option<(Uuid,)> = sqlx::query_as(
            "UPDATE work SET state = 'pending', admitted_at = NULL, last_error = $2
             WHERE id = $1 AND state = 'admitted'
             RETURNING tenant_id",
        )
        .bind(id)
        .bind(Option::<String>::None)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((tenant_id,)) = released else {
            tx.commit().await?;
            return Ok(false);
        };

        sqlx::query(
            "UPDATE tenant_budgets SET in_flight = greatest(in_flight - 1, 0), updated_at = now()
             WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Return items whose workflow start never landed to the queue.
    ///
    /// A scheduler that dies between `claim` and `mark_running` leaves rows in
    /// `admitted` with no workflow. Nothing else would ever pick them up.
    pub async fn requeue_stalled(&self, older_than: TimeDelta) -> Result<u64> {
        let seconds = older_than.num_seconds().max(0) as f64;
        let mut tx = self.pool.begin().await?;

        let rows = sqlx::query(
            "UPDATE work SET state = 'pending', admitted_at = NULL
             WHERE state = 'admitted'
               AND workflow_id IS NULL
               AND admitted_at < now() - make_interval(secs => $1)
             RETURNING tenant_id",
        )
        .bind(seconds)
        .fetch_all(&mut *tx)
        .await?;

        for row in &rows {
            sqlx::query(
                "UPDATE tenant_budgets SET in_flight = greatest(in_flight - 1, 0)
                 WHERE tenant_id = $1",
            )
            .bind(row.get::<Uuid, _>("tenant_id"))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(rows.len() as u64)
    }

    /// Recompute `in_flight` from the work rows.
    ///
    /// The counter is a cache of a count. Run at startup, because a process
    /// killed mid-transaction is exactly when a cache stops matching.
    pub async fn reconcile_in_flight(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE tenant_budgets b
             SET in_flight = (
                   SELECT count(*)::int FROM work w
                   WHERE w.tenant_id = b.tenant_id AND w.state IN ('admitted', 'running')
                 ),
                 updated_at = now()
             WHERE b.in_flight IS DISTINCT FROM (
                   SELECT count(*)::int FROM work w
                   WHERE w.tenant_id = b.tenant_id AND w.state IN ('admitted', 'running')
                 )",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Record what a task cost. Workers report this; billing rolls it up.
    pub async fn record_cost(
        &self,
        id: WorkId,
        cpu_seconds: f64,
        bytes_written: i64,
        machine: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO work_cost (work_id, tenant_id, cpu_seconds, bytes_written, machine)
             SELECT $1, tenant_id, $2, $3, $4 FROM work WHERE id = $1
             ON CONFLICT (work_id) DO UPDATE SET
               cpu_seconds = work_cost.cpu_seconds + EXCLUDED.cpu_seconds,
               bytes_written = work_cost.bytes_written + EXCLUDED.bytes_written,
               machine = EXCLUDED.machine,
               recorded_at = now()",
        )
        .bind(id)
        .bind(cpu_seconds)
        .bind(bytes_written)
        .bind(machine)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Lane wait times and depths, for `verve capacity`.
    pub async fn lane_stats(&self) -> Result<Vec<LaneStats>> {
        let rows = sqlx::query(
            "SELECT lane,
                    count(*) FILTER (WHERE state = 'pending') AS waiting,
                    count(*) FILTER (WHERE state IN ('admitted', 'running')) AS running,
                    min(created_at) FILTER (WHERE state = 'pending') AS oldest
             FROM work
             WHERE state IN ('pending', 'admitted', 'running')
             GROUP BY lane",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .filter_map(|row| {
                let lane: String = row.get("lane");
                Some(LaneStats {
                    lane: lane.parse().ok()?,
                    waiting: row.get::<i64, _>("waiting").max(0) as u32,
                    running: row.get::<i64, _>("running").max(0) as u32,
                    oldest: row.get("oldest"),
                })
            })
            .collect())
    }
}

/// One lane's queue depth and age. Alert on age, not depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LaneStats {
    /// Which lane.
    pub lane: Lane,
    /// Items pending.
    pub waiting: u32,
    /// Items holding a slot.
    pub running: u32,
    /// When the oldest pending item was submitted.
    pub oldest: Option<DateTime<Utc>>,
}

impl LaneStats {
    /// How long the oldest pending item has waited.
    pub fn oldest_wait(&self, now: DateTime<Utc>) -> TimeDelta {
        self.oldest.map_or(TimeDelta::zero(), |t| now - t)
    }
}

fn work_from_row(row: &PgRow) -> Result<WorkItem> {
    let id: Uuid = row.get("id");
    let kind: String = row.get("kind");
    let lane: String = row.get("lane");
    let state: String = row.get("state");

    Ok(WorkItem {
        id,
        tenant_id: row.get("tenant_id"),
        kind: kind
            .parse::<WorkKind>()
            .map_err(|source| StoreError::Corrupt { id, source })?,
        ref_id: row.get("ref_id"),
        spec: row.get("spec"),
        lane: lane
            .parse::<Lane>()
            .map_err(|source| StoreError::Corrupt { id, source })?,
        priority_key: row.get("priority_key"),
        fairness_weight: row.get("fairness_weight"),
        state: state
            .parse::<WorkState>()
            .map_err(|source| StoreError::Corrupt { id, source })?,
        workflow_id: row.get("workflow_id"),
        dedupe_key: row.get("dedupe_key"),
        attempts: row.get("attempts"),
        created_at: row.get("created_at"),
        admitted_at: row.get("admitted_at"),
        finished_at: row.get("finished_at"),
    })
}
