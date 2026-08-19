//! The Temporal engine. Behind [`WorkflowEngine`], so nothing above it knows.
//!
//! Workflows are started *untyped*, by name, with `spec` passed through as
//! JSON: the scheduler must not link the video workflow types it dispatches to.

use crate::engine::{EngineError, WorkflowEngine};
use crate::model::WorkItem;
use async_trait::async_trait;
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, UntypedWorkflow, WorkflowCancelOptions,
    WorkflowStartOptions,
};
use temporalio_common::data_converters::{
    GenericPayloadConverter, PayloadConverter, RawValue, SerializationContext,
    SerializationContextData,
};

/// How to reach Temporal.
#[derive(Debug, Clone)]
pub struct TemporalConfig {
    /// Frontend address, e.g. `http://localhost:7233`.
    pub address: String,
    /// Namespace.
    pub namespace: String,
    /// Task queue workers poll.
    pub task_queue: String,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            address: "http://localhost:7253".into(),
            namespace: "default".into(),
            task_queue: "ferrite".into(),
        }
    }
}

/// Starts and cancels real Temporal workflows.
pub struct TemporalEngine {
    client: Client,
    config: TemporalConfig,
    converter: PayloadConverter,
}

impl std::fmt::Debug for TemporalEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemporalEngine")
            .field("address", &self.config.address)
            .field("namespace", &self.config.namespace)
            .field("task_queue", &self.config.task_queue)
            .finish()
    }
}

impl TemporalEngine {
    /// Connect.
    pub async fn connect(config: TemporalConfig) -> Result<Self, EngineError> {
        let url = config
            .address
            .parse::<url::Url>()
            .map_err(|e| EngineError::Permanent(format!("bad address: {e}")))?;

        let connection = Connection::connect(
            ConnectionOptions::new(url)
                .identity("ferrite-scheduler".to_string())
                .build(),
        )
        .await
        .map_err(|e| EngineError::Unavailable(e.to_string()))?;

        let client = Client::new(
            connection,
            ClientOptions::new(config.namespace.clone()).build(),
        )
        .map_err(|e| EngineError::Unavailable(e.to_string()))?;

        Ok(Self {
            client,
            config,
            converter: PayloadConverter::serde_json(),
        })
    }

    /// The workflow type name for a work kind, e.g. `ferrite.asset_fastpath`.
    ///
    /// A string, not a Rust type: a scheduler that had to be recompiled to
    /// dispatch a new kind of work would be the wrong shape.
    pub fn workflow_type(item: &WorkItem) -> String {
        format!("ferrite.{}", item.kind)
    }

    /// The workflow id for a work item. Deterministic, so a retried start
    /// collides with the original rather than running it twice — and so a
    /// worker can recover the work id without being handed it separately.
    pub fn workflow_id(item: &WorkItem) -> String {
        format!("ferrite-{}-{}", item.kind, item.id)
    }

    /// The work id inside a workflow id. The join key back to `sched_db`.
    pub fn work_id_from_workflow_id(workflow_id: &str) -> Option<uuid::Uuid> {
        workflow_id.rsplit_once('-').and_then(|(_, tail)| {
            // A UUID has its own hyphens, so take the last five segments.
            let parts: Vec<&str> = workflow_id.split('-').collect();
            let _ = tail;
            if parts.len() < 5 {
                return None;
            }
            parts[parts.len() - 5..].join("-").parse().ok()
        })
    }
}

#[async_trait]
impl WorkflowEngine for TemporalEngine {
    async fn start(&self, item: &WorkItem) -> Result<String, EngineError> {
        let workflow_id = Self::workflow_id(item);

        let payload = self
            .converter
            .to_payload(
                &SerializationContext {
                    data: &SerializationContextData::None,
                    converter: &self.converter,
                },
                &item.spec,
            )
            .map_err(|e| EngineError::Permanent(format!("spec is not serializable: {e}")))?;

        self.client
            .start_workflow(
                UntypedWorkflow::new(Self::workflow_type(item)),
                RawValue::new(vec![payload]),
                WorkflowStartOptions::new(self.config.task_queue.clone(), workflow_id.clone())
                    .build(),
            )
            .await
            .map_err(|e| EngineError::Unavailable(e.to_string()))?;

        Ok(workflow_id)
    }

    async fn cancel(&self, workflow_id: &str) -> Result<(), EngineError> {
        self.client
            .get_workflow_handle::<UntypedWorkflow>(workflow_id)
            .cancel(WorkflowCancelOptions::builder().build())
            .await
            .map_err(|e| EngineError::Unavailable(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Lane, WorkKind, WorkState};

    fn item(kind: WorkKind) -> WorkItem {
        WorkItem {
            id: uuid::Uuid::nil(),
            tenant_id: uuid::Uuid::nil(),
            kind,
            ref_id: uuid::Uuid::nil(),
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

    #[test]
    fn a_work_kind_maps_to_a_workflow_type_name_not_a_rust_type() {
        assert_eq!(
            TemporalEngine::workflow_type(&item(WorkKind::Fake)),
            "ferrite.fake"
        );
        assert_eq!(
            TemporalEngine::workflow_type(&item(WorkKind::AssetFastpath)),
            "ferrite.asset_fastpath"
        );
    }

    #[test]
    fn the_workflow_id_is_derived_so_a_retried_start_cannot_run_twice() {
        let item = item(WorkKind::Fake);
        assert_eq!(
            TemporalEngine::workflow_id(&item),
            TemporalEngine::workflow_id(&item)
        );
        assert!(TemporalEngine::workflow_id(&item).contains(&item.id.to_string()));
    }

    #[test]
    fn a_worker_can_recover_the_work_id_from_its_own_workflow_id() {
        for kind in [WorkKind::Fake, WorkKind::AssetFastpath, WorkKind::Repackage] {
            let mut it = item(kind);
            it.id = uuid::Uuid::now_v7();
            let wf = TemporalEngine::workflow_id(&it);
            assert_eq!(
                TemporalEngine::work_id_from_workflow_id(&wf),
                Some(it.id),
                "round trip failed for {wf}"
            );
        }
        assert_eq!(TemporalEngine::work_id_from_workflow_id("nonsense"), None);
    }
}
