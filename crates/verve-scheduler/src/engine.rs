//! Starting and cancelling the durable workflow behind a work item.
//!
//! A trait, not a Temporal call, because the public API and the video code must
//! never mention Temporal — that is what keeps it replaceable.

use crate::model::WorkItem;
use async_trait::async_trait;

/// Why a workflow could not be started or stopped.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The engine is unreachable or refused. The item goes back in the queue.
    #[error("engine unavailable: {0}")]
    Unavailable(String),
    /// The item can never start — a bad spec, say. Retrying will not help.
    #[error("permanent: {0}")]
    Permanent(String),
}

impl EngineError {
    /// Whether requeuing could plausibly succeed later.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
}

/// Whatever actually runs the work.
#[async_trait]
pub trait WorkflowEngine: Send + Sync + std::fmt::Debug {
    /// Start `item`, returning the workflow id to record against it.
    async fn start(&self, item: &WorkItem) -> Result<String, EngineError>;

    /// Stop a running workflow. Must be safe to call on one already gone.
    async fn cancel(&self, workflow_id: &str) -> Result<(), EngineError>;
}

/// Starts nothing and remembers everything. For tests and `--dry-run`.
#[derive(Debug, Default)]
pub struct RecordingEngine {
    started: std::sync::Mutex<Vec<(uuid::Uuid, String)>>,
    canceled: std::sync::Mutex<Vec<String>>,
    fail_next: std::sync::atomic::AtomicUsize,
}

impl RecordingEngine {
    /// An engine that always succeeds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Make the next `n` starts fail as retryable.
    pub fn fail_next_starts(&self, n: usize) {
        self.fail_next.store(n, std::sync::atomic::Ordering::SeqCst);
    }

    /// Every (work id, workflow id) started so far.
    pub fn started(&self) -> Vec<(uuid::Uuid, String)> {
        self.started.lock().expect("poisoned").clone()
    }

    /// Every workflow id cancelled so far.
    pub fn canceled(&self) -> Vec<String> {
        self.canceled.lock().expect("poisoned").clone()
    }
}

#[async_trait]
impl WorkflowEngine for RecordingEngine {
    async fn start(&self, item: &WorkItem) -> Result<String, EngineError> {
        use std::sync::atomic::Ordering;
        if self
            .fail_next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(EngineError::Unavailable("engine told to fail".into()));
        }
        let workflow_id = format!("verve-{}-{}", item.kind, item.id);
        self.started
            .lock()
            .expect("poisoned")
            .push((item.id, workflow_id.clone()));
        Ok(workflow_id)
    }

    async fn cancel(&self, workflow_id: &str) -> Result<(), EngineError> {
        self.canceled
            .lock()
            .expect("poisoned")
            .push(workflow_id.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Lane, WorkKind, WorkState};

    fn item() -> WorkItem {
        WorkItem {
            id: uuid::Uuid::now_v7(),
            tenant_id: uuid::Uuid::now_v7(),
            kind: WorkKind::Fake,
            ref_id: uuid::Uuid::now_v7(),
            spec: serde_json::json!({}),
            lane: Lane::Standard,
            priority_key: 0,
            fairness_weight: 1.0,
            state: WorkState::Admitted,
            workflow_id: None,
            dedupe_key: "k".into(),
            attempts: 1,
            created_at: chrono::Utc::now(),
            admitted_at: None,
            finished_at: None,
        }
    }

    #[tokio::test]
    async fn a_started_workflow_is_named_after_the_work() {
        let engine = RecordingEngine::new();
        let item = item();
        let workflow_id = engine.start(&item).await.unwrap();
        assert!(workflow_id.contains(&item.id.to_string()));
        assert_eq!(engine.started().len(), 1);
    }

    #[tokio::test]
    async fn an_unavailable_engine_is_retryable_and_a_permanent_error_is_not() {
        assert!(EngineError::Unavailable("x".into()).is_retryable());
        assert!(!EngineError::Permanent("x".into()).is_retryable());
    }

    #[tokio::test]
    async fn failures_are_injectable_and_stop_after_the_requested_count() {
        let engine = RecordingEngine::new();
        engine.fail_next_starts(2);
        assert!(engine.start(&item()).await.is_err());
        assert!(engine.start(&item()).await.is_err());
        assert!(engine.start(&item()).await.is_ok());
    }
}
