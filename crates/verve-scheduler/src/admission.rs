//! The loop. Machines free up when jobs finish, and something must continuously
//! notice and release more work — which is why this is not a request handler.

use crate::capacity::{self, LaneShares};
use crate::engine::WorkflowEngine;
use crate::fairness::Fairness;
use crate::model::{Lane, WorkState};
use crate::store::{Store, StoreError};
use chrono::TimeDelta;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// How the loop is tuned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// Total slots across every worker.
    pub total_slots: u32,
    /// Per-lane guarantees.
    pub shares: LaneShares,
    /// Gap between ticks.
    pub tick: Duration,
    /// An `admitted` row with no workflow older than this is presumed abandoned.
    pub stall_after: TimeDelta,
    /// Attempts before an item is failed rather than requeued.
    pub max_attempts: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            total_slots: 64,
            shares: LaneShares::default(),
            tick: Duration::from_millis(250),
            stall_after: TimeDelta::minutes(5),
            max_attempts: 5,
        }
    }
}

/// Ticks between stall sweeps. The sweep is a write, so not every tick.
const SWEEP_EVERY_TICKS: u32 = 240;

/// What one tick did. Returned so tests can assert on it and so the loop has
/// something honest to log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tick {
    /// Items moved from `pending` to `running`.
    pub admitted: u32,
    /// Items whose workflow start failed and went back in the queue.
    pub requeued: u32,
    /// Items that ran out of attempts.
    pub failed: u32,
    /// Abandoned starts swept back into the queue.
    pub recovered: u64,
}

impl Tick {
    /// Whether anything happened. A quiet tick is not worth a log line.
    pub fn is_quiet(&self) -> bool {
        *self == Self::default()
    }
}

/// The admission loop.
#[derive(Debug)]
pub struct Scheduler {
    store: Store,
    engine: Arc<dyn WorkflowEngine>,
    config: Config,
    fairness: [Fairness; 3],
    ticks: u32,
}

impl Scheduler {
    /// A scheduler over `store`, starting work through `engine`.
    pub fn new(store: Store, engine: Arc<dyn WorkflowEngine>, config: Config) -> Self {
        Self {
            store,
            engine,
            config,
            fairness: [Fairness::new(), Fairness::new(), Fairness::new()],
            ticks: 0,
        }
    }

    /// The store, for the API layer to share.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Repair counters left wrong by a crash, then loop until cancelled.
    pub async fn run(mut self, shutdown: tokio::sync::watch::Receiver<bool>) -> anyhow::Result<()> {
        let repaired = self.store.reconcile_in_flight().await?;
        if repaired > 0 {
            warn!(
                repaired,
                "in_flight disagreed with the work rows at startup"
            );
        }
        info!(
            total_slots = self.config.total_slots,
            tick_ms = self.config.tick.as_millis() as u64,
            "admission loop started"
        );

        let mut ticker = tokio::time::interval(self.config.tick);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut shutdown = shutdown;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match self.tick().await {
                        Ok(t) if !t.is_quiet() => debug!(?t, "tick"),
                        Ok(_) => {}
                        // A tick that fails must not kill the loop: the next one
                        // sees the same queue and tries again.
                        Err(e) => warn!(error = %e, "tick failed"),
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("admission loop stopping");
                        return Ok(());
                    }
                }
            }
        }
    }

    /// One pass: work out how many slots each lane has, split them fairly, and
    /// start what was granted.
    pub async fn tick(&mut self) -> Result<Tick, StoreError> {
        let mut report = Tick::default();
        self.ticks = self.ticks.wrapping_add(1);

        if self.ticks.is_multiple_of(SWEEP_EVERY_TICKS) {
            report.recovered = self.store.requeue_stalled(self.config.stall_after).await?;
            if report.recovered > 0 {
                warn!(report.recovered, "requeued starts that never landed");
            }
        }

        let fleet = self
            .store
            .fleet_state(self.config.total_slots, self.config.shares)
            .await?;
        let grants = capacity::lane_grants(&fleet);

        for (i, lane) in Lane::ALL.into_iter().enumerate() {
            if grants[i] == 0 {
                continue;
            }
            let candidates = self.store.candidates(lane).await?;
            if candidates.is_empty() {
                continue;
            }

            for grant in self.fairness[i].distribute(grants[i], &candidates) {
                let claimed = self.store.claim(grant.tenant_id, lane, grant.count).await?;
                for item in claimed {
                    match self.engine.start(&item).await {
                        Ok(workflow_id) => {
                            self.store.mark_running(item.id, &workflow_id).await?;
                            report.admitted += 1;
                        }
                        Err(e) if e.is_retryable() && item.attempts < self.config.max_attempts => {
                            warn!(work = %item.id, error = %e, "start failed, requeuing");
                            self.store.release_to_pending(item.id).await?;
                            report.requeued += 1;
                        }
                        Err(e) => {
                            warn!(work = %item.id, error = %e, "start failed permanently");
                            self.store
                                .finish(item.id, WorkState::Failed, Some(&e.to_string()))
                                .await?;
                            report.failed += 1;
                        }
                    }
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_that_did_nothing_is_quiet() {
        assert!(Tick::default().is_quiet());
        assert!(
            !Tick {
                admitted: 1,
                ..Default::default()
            }
            .is_quiet()
        );
    }

    #[test]
    fn the_default_tick_is_the_documented_250ms() {
        assert_eq!(Config::default().tick, Duration::from_millis(250));
    }
}
