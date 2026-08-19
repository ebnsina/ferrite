//! Store behaviour against a real Postgres. Skips unless SCHED_DATABASE_URL is
//! set, so `cargo test` stays green on a machine with nothing running.

use chrono::TimeDelta;
use ferrite_scheduler::capacity::LaneShares;
use ferrite_scheduler::model::{Lane, NewWork, WorkKind, WorkState};
use ferrite_scheduler::store::{Scope, Store, StoreError, Submitted};
use serde_json::json;
use uuid::Uuid;

/// Its own schema per test, so a global sweep in one cannot touch another.
async fn store_named(schema: &str) -> Option<Store> {
    let url = std::env::var("SCHED_DATABASE_URL").ok()?;
    Some(
        Store::connect_fresh_schema(&url, schema)
            .await
            .expect("connect"),
    )
}

fn tenant() -> Uuid {
    Uuid::now_v7()
}

fn work(tenant_id: Uuid, lane: Lane, dedupe: &str) -> NewWork {
    NewWork {
        tenant_id,
        kind: WorkKind::Fake,
        ref_id: Uuid::now_v7(),
        spec: json!({"steps": 3}),
        lane,
        priority_key: 0,
        dedupe_key: dedupe.to_string(),
    }
}

async fn tenant_with_budget(store: &Store, max: i32, rate: i32, weight: f32) -> Uuid {
    let id = tenant();
    store
        .upsert_budget(id, max, rate, weight)
        .await
        .expect("budget");
    id
}

#[tokio::test]
async fn submitting_the_same_key_twice_returns_the_first_item() {
    let Some(store) = store_named("s_submitting_the_same_key_twice_returns_the_first_item").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;

    let first = store
        .submit(&work(t, Lane::Standard, "dedupe-me"))
        .await
        .unwrap();
    let second = store
        .submit(&work(t, Lane::Standard, "dedupe-me"))
        .await
        .unwrap();

    assert!(matches!(first, Submitted::Created(_)));
    assert!(matches!(second, Submitted::Existing(_)));
    assert_eq!(
        first.item().id,
        second.item().id,
        "a second row was created"
    );
}

#[tokio::test]
async fn the_same_key_under_a_different_tenant_is_a_different_item() {
    let Some(store) =
        store_named("s_the_same_key_under_a_different_tenant_is_a_different_item").await
    else {
        return;
    };
    let a = tenant_with_budget(&store, 10, 600, 1.0).await;
    let b = tenant_with_budget(&store, 10, 600, 1.0).await;

    let one = store
        .submit(&work(a, Lane::Standard, "shared"))
        .await
        .unwrap();
    let two = store
        .submit(&work(b, Lane::Standard, "shared"))
        .await
        .unwrap();
    assert_ne!(one.item().id, two.item().id, "dedupe leaked across tenants");
}

#[tokio::test]
async fn a_tenant_without_a_budget_cannot_queue_anything() {
    let Some(store) = store_named("s_a_tenant_without_a_budget_cannot_queue_anything").await else {
        return;
    };
    let err = store
        .submit(&work(tenant(), Lane::Standard, "k"))
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::NoBudget(_)), "{err}");
}

#[tokio::test]
async fn a_suspended_tenant_is_refused_at_submit() {
    let Some(store) = store_named("s_a_suspended_tenant_is_refused_at_submit").await else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.set_suspended(t, true).await.unwrap();

    let err = store
        .submit(&work(t, Lane::Standard, "k"))
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::Suspended(_)), "{err}");
}

#[tokio::test]
async fn the_plan_weight_is_copied_not_looked_up() {
    let Some(store) = store_named("s_the_plan_weight_is_copied_not_looked_up").await else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 5.0).await;
    let item = store.submit(&work(t, Lane::Standard, "k")).await.unwrap();
    assert_eq!(item.item().fairness_weight, 5.0);

    // Downgrade the plan; already-queued work keeps the weight it was given.
    store.upsert_budget(t, 10, 600, 1.0).await.unwrap();
    let after = store.get(item.item().id).await.unwrap().unwrap();
    assert_eq!(
        after.fairness_weight, 5.0,
        "a downgrade demoted queued work"
    );
}

#[tokio::test]
async fn claiming_takes_priority_then_oldest_and_moves_in_flight() {
    let Some(store) =
        store_named("s_claiming_takes_priority_then_oldest_and_moves_in_flight").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;

    for (i, priority) in [5, 1, 5, 1].into_iter().enumerate() {
        let mut w = work(t, Lane::Standard, &format!("k{i}"));
        w.priority_key = priority;
        store.submit(&w).await.unwrap();
    }

    let claimed = store.claim(t, Lane::Standard, 2).await.unwrap();
    assert_eq!(claimed.len(), 2);
    assert!(
        claimed.iter().all(|c| c.priority_key == 1),
        "priority was ignored"
    );
    assert!(claimed.iter().all(|c| c.state == WorkState::Admitted));
    assert!(
        claimed[0].created_at <= claimed[1].created_at,
        "not oldest first"
    );

    assert_eq!(store.budget(t).await.unwrap().in_flight, 2);
}

#[tokio::test]
async fn a_claim_never_crosses_lanes() {
    let Some(store) = store_named("s_a_claim_never_crosses_lanes").await else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.submit(&work(t, Lane::Bulk, "b")).await.unwrap();

    assert!(store.claim(t, Lane::Realtime, 5).await.unwrap().is_empty());
    assert_eq!(store.claim(t, Lane::Bulk, 5).await.unwrap().len(), 1);
}

#[tokio::test]
async fn two_schedulers_never_claim_the_same_row() {
    let Some(store) = store_named("s_two_schedulers_never_claim_the_same_row").await else {
        return;
    };
    let t = tenant_with_budget(&store, 100, 6000, 1.0).await;
    for i in 0..40 {
        store
            .submit(&work(t, Lane::Standard, &format!("k{i}")))
            .await
            .unwrap();
    }

    // Eight concurrent claimers, as many scheduler copies would be.
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let store = store.clone();
        set.spawn(async move { store.claim(t, Lane::Standard, 5).await.unwrap() });
    }

    let mut ids = Vec::new();
    while let Some(res) = set.join_next().await {
        ids.extend(res.unwrap().into_iter().map(|w| w.id));
    }

    let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "a row was handed out twice");
    assert_eq!(store.budget(t).await.unwrap().in_flight as usize, ids.len());
}

#[tokio::test]
async fn finishing_gives_the_slot_back_exactly_once() {
    let Some(store) = store_named("s_finishing_gives_the_slot_back_exactly_once").await else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.submit(&work(t, Lane::Standard, "k")).await.unwrap();
    let claimed = store.claim(t, Lane::Standard, 1).await.unwrap();
    let id = claimed[0].id;

    assert_eq!(store.budget(t).await.unwrap().in_flight, 1);
    assert!(store.finish(id, WorkState::Done, None).await.unwrap());
    assert_eq!(store.budget(t).await.unwrap().in_flight, 0);

    // A duplicate completion must not drive the counter negative.
    assert!(!store.finish(id, WorkState::Done, None).await.unwrap());
    assert_eq!(store.budget(t).await.unwrap().in_flight, 0);
}

#[tokio::test]
async fn a_start_that_died_before_the_workflow_landed_is_requeued() {
    let Some(store) =
        store_named("s_a_start_that_died_before_the_workflow_landed_is_requeued").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.submit(&work(t, Lane::Standard, "k")).await.unwrap();
    let claimed = store.claim(t, Lane::Standard, 1).await.unwrap();
    let id = claimed[0].id;

    // Nothing is stalled yet — a claim from a second ago is normal.
    assert_eq!(
        store.requeue_stalled(TimeDelta::seconds(60)).await.unwrap(),
        0
    );

    // With a zero window the same row looks abandoned.
    assert_eq!(store.requeue_stalled(TimeDelta::zero()).await.unwrap(), 1);
    let after = store.get(id).await.unwrap().unwrap();
    assert_eq!(after.state, WorkState::Pending, "work was lost");
    assert_eq!(store.budget(t).await.unwrap().in_flight, 0, "slot leaked");
}

#[tokio::test]
async fn work_already_running_is_never_requeued_underneath_its_worker() {
    let Some(store) =
        store_named("s_work_already_running_is_never_requeued_underneath_its_work").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.submit(&work(t, Lane::Standard, "k")).await.unwrap();
    let id = store.claim(t, Lane::Standard, 1).await.unwrap()[0].id;
    store.mark_running(id, "wf-1").await.unwrap();

    assert_eq!(store.requeue_stalled(TimeDelta::zero()).await.unwrap(), 0);
    assert_eq!(
        store.get(id).await.unwrap().unwrap().state,
        WorkState::Running
    );
}

#[tokio::test]
async fn reconcile_repairs_a_counter_a_crash_left_wrong() {
    let Some(store) = store_named("s_reconcile_repairs_a_counter_a_crash_left_wrong").await else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    for i in 0..3 {
        store
            .submit(&work(t, Lane::Standard, &format!("k{i}")))
            .await
            .unwrap();
    }
    store.claim(t, Lane::Standard, 3).await.unwrap();

    // Simulate a counter that drifted from the rows.
    sqlx::query("UPDATE tenant_budgets SET in_flight = 99 WHERE tenant_id = $1")
        .bind(t)
        .execute(store.pool())
        .await
        .unwrap();

    store.reconcile_in_flight().await.unwrap();
    assert_eq!(store.budget(t).await.unwrap().in_flight, 3);
}

#[tokio::test]
async fn a_tenant_at_its_concurrency_limit_is_not_a_candidate() {
    let Some(store) = store_named("s_a_tenant_at_its_concurrency_limit_is_not_a_candidate").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 2, 600, 1.0).await;
    for i in 0..5 {
        store
            .submit(&work(t, Lane::Standard, &format!("k{i}")))
            .await
            .unwrap();
    }
    store.claim(t, Lane::Standard, 2).await.unwrap();

    let mine: Vec<_> = store
        .candidates(Lane::Standard)
        .await
        .unwrap()
        .into_iter()
        .filter(|c| c.tenant_id == t)
        .collect();
    assert!(
        mine.is_empty(),
        "a tenant at its limit was offered a turn: {mine:?}"
    );
}

#[tokio::test]
async fn the_per_minute_rate_limit_caps_headroom() {
    let Some(store) = store_named("s_the_per_minute_rate_limit_caps_headroom").await else {
        return;
    };
    let t = tenant_with_budget(&store, 100, 3, 1.0).await;
    for i in 0..10 {
        store
            .submit(&work(t, Lane::Standard, &format!("k{i}")))
            .await
            .unwrap();
    }
    store.claim(t, Lane::Standard, 2).await.unwrap();

    let mine = store
        .candidates(Lane::Standard)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.tenant_id == t)
        .expect("still a candidate");
    assert_eq!(mine.headroom, 1, "rate limit did not cap headroom");
}

#[tokio::test]
async fn a_suspended_tenant_stops_being_admitted_even_with_work_queued() {
    let Some(store) =
        store_named("s_a_suspended_tenant_stops_being_admitted_even_with_work_que").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    store.submit(&work(t, Lane::Standard, "k")).await.unwrap();
    store.set_suspended(t, true).await.unwrap();

    let found = store.candidates(Lane::Standard).await.unwrap();
    assert!(
        !found.iter().any(|c| c.tenant_id == t),
        "suspension did not stop admission"
    );
}

#[tokio::test]
async fn fleet_state_counts_running_and_pending_per_lane() {
    let Some(store) = store_named("s_fleet_state_counts_running_and_pending_per_lane").await else {
        return;
    };
    let t = tenant_with_budget(&store, 100, 6000, 1.0).await;
    for i in 0..4 {
        store
            .submit(&work(t, Lane::Realtime, &format!("r{i}")))
            .await
            .unwrap();
    }
    store.claim(t, Lane::Realtime, 1).await.unwrap();

    let state = store.fleet_state(64, LaneShares::default()).await.unwrap();
    assert!(state.lane(Lane::Realtime).running >= 1);
    assert!(state.lane(Lane::Realtime).pending >= 3);
}

#[tokio::test]
async fn cost_is_recorded_against_the_work_and_its_tenant() {
    let Some(store) = store_named("s_cost_is_recorded_against_the_work_and_its_tenant").await
    else {
        return;
    };
    let t = tenant_with_budget(&store, 10, 600, 1.0).await;
    let id = store
        .submit(&work(t, Lane::Standard, "k"))
        .await
        .unwrap()
        .item()
        .id;

    store.record_cost(id, 12.5, 4096, "worker-1").await.unwrap();
    store.record_cost(id, 2.5, 1024, "worker-1").await.unwrap();

    // Scoped, because an unscoped read of a tenant table returns nothing.
    let mut tx = store.scoped(Scope::Tenant(t)).await.unwrap();
    let (tenant_id, cpu, bytes): (Uuid, f64, i64) = sqlx::query_as(
        "SELECT tenant_id, cpu_seconds, bytes_written FROM work_cost WHERE work_id = $1",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(tenant_id, t);
    assert_eq!(cpu, 15.0, "repeated reports must accumulate");
    assert_eq!(bytes, 5120);
}
