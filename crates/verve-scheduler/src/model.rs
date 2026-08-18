//! What the scheduler knows. Deliberately nothing about video: `spec` is opaque
//! bytes, so the whole thing is provable with fake work.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// A tenant. The unit fairness is measured in.
pub type TenantId = Uuid;
/// One queued item.
pub type WorkId = Uuid;

/// Priority lanes. Order is the admission order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    /// Fast path, premium first. Everything yields to it.
    Realtime,
    /// Quality-path ladders.
    Standard,
    /// Re-encodes, backfills, analysis. Gets the rest and all idle capacity.
    Bulk,
}

impl Lane {
    /// Every lane, highest priority first. Admission walks this order.
    pub const ALL: [Lane; 3] = [Lane::Realtime, Lane::Standard, Lane::Bulk];

    /// Stable string for storage and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Standard => "standard",
            Self::Bulk => "bulk",
        }
    }
}

impl FromStr for Lane {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        match s {
            "realtime" => Ok(Self::Realtime),
            "standard" => Ok(Self::Standard),
            "bulk" => Ok(Self::Bulk),
            other => Err(ParseError::new("lane", other)),
        }
    }
}

/// What a work item is for. The scheduler never looks inside `spec`, but it
/// does record the kind so `verve work list` is readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    /// One rung, fast, so the asset becomes playable.
    AssetFastpath,
    /// The rest of the ladder.
    AssetQuality,
    /// Job mode: one output.
    Job,
    /// A better encode of something already published.
    Reencode,
    /// New manifests over existing segments.
    Repackage,
    /// Not video. Exists so Stage 1 is provable before any video code lands.
    Fake,
}

impl WorkKind {
    /// Stable string for storage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AssetFastpath => "asset_fastpath",
            Self::AssetQuality => "asset_quality",
            Self::Job => "job",
            Self::Reencode => "reencode",
            Self::Repackage => "repackage",
            Self::Fake => "fake",
        }
    }
}

impl fmt::Display for WorkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        match s {
            "asset_fastpath" => Ok(Self::AssetFastpath),
            "asset_quality" => Ok(Self::AssetQuality),
            "job" => Ok(Self::Job),
            "reencode" => Ok(Self::Reencode),
            "repackage" => Ok(Self::Repackage),
            "fake" => Ok(Self::Fake),
            other => Err(ParseError::new("work kind", other)),
        }
    }
}

/// Where an item is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkState {
    /// Queued, waiting for a slot.
    Pending,
    /// A slot was granted and the workflow start is in progress.
    Admitted,
    /// The workflow is running.
    Running,
    /// Finished successfully.
    Done,
    /// Finished unsuccessfully and will not be retried.
    Failed,
    /// Cancelled by a customer or an operator.
    Canceled,
}

impl WorkState {
    /// Stable string for storage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Admitted => "admitted",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    /// Whether this state holds a concurrency slot.
    pub fn holds_slot(self) -> bool {
        matches!(self, Self::Admitted | Self::Running)
    }

    /// Whether nothing more will happen to this item.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Canceled)
    }
}

impl FromStr for WorkState {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        match s {
            "pending" => Ok(Self::Pending),
            "admitted" => Ok(Self::Admitted),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            other => Err(ParseError::new("work state", other)),
        }
    }
}

/// A string that should have been one of a fixed set and was not.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {0}")]
pub struct ParseError(String);

impl ParseError {
    fn new(what: &'static str, value: &str) -> Self {
        Self(format!("{what}: {value:?}"))
    }
}

/// A submission from `verve-assets`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewWork {
    /// Whose work this is.
    pub tenant_id: TenantId,
    /// What it is for.
    pub kind: WorkKind,
    /// The source, asset or job it belongs to.
    pub ref_id: Uuid,
    /// Opaque to the scheduler. Handed to the workflow untouched.
    pub spec: serde_json::Value,
    /// Which lane it queues in.
    pub lane: Lane,
    /// Lower runs first, within one tenant.
    #[serde(default)]
    pub priority_key: i32,
    /// Submitting the same key twice returns the first item, not a second one.
    pub dedupe_key: String,
}

/// One row of `work`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkItem {
    /// Primary key.
    pub id: WorkId,
    /// Whose work this is.
    pub tenant_id: TenantId,
    /// What it is for.
    pub kind: WorkKind,
    /// The source, asset or job it belongs to.
    pub ref_id: Uuid,
    /// Opaque payload.
    pub spec: serde_json::Value,
    /// Which lane it queues in.
    pub lane: Lane,
    /// Lower runs first.
    pub priority_key: i32,
    /// Copied from the plan at submit time, not looked up.
    pub fairness_weight: f32,
    /// Where it is in its life.
    pub state: WorkState,
    /// The Temporal workflow, once started.
    pub workflow_id: Option<String>,
    /// Idempotency key, unique per tenant.
    pub dedupe_key: String,
    /// How many times a start has been attempted.
    pub attempts: i32,
    /// When it was submitted.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When it was granted a slot.
    pub admitted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When it reached a terminal state.
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A tenant's plan limits, copied into `sched_db` so admission never reaches
/// across into `assets_db`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TenantBudget {
    /// Whose budget.
    pub tenant_id: TenantId,
    /// Ceiling on items holding a slot at once.
    pub max_concurrent_tasks: i32,
    /// How many currently hold one.
    pub in_flight: i32,
    /// Ceiling on admissions per minute.
    pub rate_limit_per_min: i32,
}

impl TenantBudget {
    /// Slots this tenant could still take.
    pub fn headroom(&self) -> u32 {
        (self.max_concurrent_tasks - self.in_flight).max(0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_are_ordered_by_priority() {
        assert!(Lane::Realtime < Lane::Standard);
        assert!(Lane::Standard < Lane::Bulk);
        assert_eq!(Lane::ALL[0], Lane::Realtime);
    }

    #[test]
    fn every_enum_round_trips_through_its_stored_string() {
        for lane in Lane::ALL {
            assert_eq!(lane.as_str().parse::<Lane>().unwrap(), lane);
        }
        for kind in [
            WorkKind::AssetFastpath,
            WorkKind::AssetQuality,
            WorkKind::Job,
            WorkKind::Reencode,
            WorkKind::Repackage,
            WorkKind::Fake,
        ] {
            assert_eq!(kind.as_str().parse::<WorkKind>().unwrap(), kind);
        }
        for state in [
            WorkState::Pending,
            WorkState::Admitted,
            WorkState::Running,
            WorkState::Done,
            WorkState::Failed,
            WorkState::Canceled,
        ] {
            assert_eq!(state.as_str().parse::<WorkState>().unwrap(), state);
        }
    }

    #[test]
    fn an_unknown_string_is_an_error_not_a_default() {
        assert!("urgent".parse::<Lane>().is_err());
        assert!("".parse::<WorkState>().is_err());
    }

    #[test]
    fn only_admitted_and_running_hold_a_slot() {
        assert!(WorkState::Admitted.holds_slot());
        assert!(WorkState::Running.holds_slot());
        assert!(!WorkState::Pending.holds_slot());
        assert!(!WorkState::Done.holds_slot());
    }

    #[test]
    fn headroom_never_goes_negative_when_a_plan_is_downgraded() {
        let budget = TenantBudget {
            tenant_id: Uuid::nil(),
            max_concurrent_tasks: 4,
            in_flight: 9,
            rate_limit_per_min: 60,
        };
        assert_eq!(budget.headroom(), 0);
    }
}
