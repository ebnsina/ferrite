//! Stage 1's "done when", against a real Postgres:
//! fairness holds under load, killed schedulers lose and duplicate nothing,
//! cancellation stops work, and one tenant with 10,000 items cannot starve
//! another. All on fake work — no video code exists yet.

use chrono::TimeDelta;
use ferrite_scheduler::admission::{Config, Scheduler};
use ferrite_scheduler::capacity::LaneShares;
use ferrite_scheduler::engine::RecordingEngine;
use ferrite_scheduler::model::{Lane, NewWork, WorkState};
use ferrite_scheduler::store::Store;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

async fn store_named(schema: &str) -> Option<Store> {
    let url = std::env::var("SCHED_DATABASE_URL").ok()?;
    Some(
        Store::connect_fresh_schema(&url, schema)
            .await
            .expect("connect"),
    )
}

fn config(total_slots: u32) -> Config {
    Config {
        total_slots,
        shares: LaneShares::default(),
        tick: Duration::from_millis(1),
        stall_after: TimeDelta::zero(),
        max_attempts: 3,
    }
}

async fn queue(store: &Store, tenant: Uuid, lane: Lane, count: usize, tag: &str) {
    for i in 0..count {
        store
            .submit(&NewWork {
                tenant_id: tenant,
                kind: ferrite_scheduler::model::WorkKind::Fake,
                ref_id: Uuid::now_v7(),
                spec: json!({"steps": 1}),
                lane,
                priority_key: 0,
                dedupe_key: format!("{tag}-{i}"),
            })
            .await
            .expect("submit");
    }
}

/// Finish everything currently holding a slot, as workers completing would.
async fn drain_running(store: &Store) -> usize {
    let running = store
        .list(None, Some(WorkState::Running), 10_000)
        .await
        .unwrap();
    for item in &running {
        store.finish(item.id, WorkState::Done, None).await.unwrap();
    }
    running.len()
}

#[tokio::test]
async fn ten_thousand_items_cannot_starve_a_tenant_with_one() {
    let Some(store) = store_named("a_starvation").await else {
        return;
    };
    let engine = Arc::new(RecordingEngine::new());

    let hog = Uuid::now_v7();
    let small = Uuid::now_v7();
    store.upsert_budget(hog, 1000, 100_000, 1.0).await.unwrap();
    store
        .upsert_budget(small, 1000, 100_000, 1.0)
        .await
        .unwrap();

    queue(&store, hog, Lane::Standard, 2_000, "hog").await;
    queue(&store, small, Lane::Standard, 1, "small").await;

    let mut scheduler = Scheduler::new(store.clone(), engine.clone(), config(16));
    scheduler.tick().await.unwrap();

    let started: Vec<Uuid> = engine.started().iter().map(|(id, _)| *id).collect();
    let mut small_served = 0;
    for id in &started {
        if store.get(*id).await.unwrap().unwrap().tenant_id == small {
            small_served += 1;
        }
    }
    assert_eq!(small_served, 1, "the one-item tenant waited behind 2,000");
}

#[tokio::test]
async fn fairness_holds_across_many_ticks_under_load() {
    let Some(store) = store_named("a_fairness_load").await else {
        return;
    };
    let engine = Arc::new(RecordingEngine::new());

    let premium = Uuid::now_v7();
    let free = Uuid::now_v7();
    store
        .upsert_budget(premium, 1000, 100_000, 5.0)
        .await
        .unwrap();
    store.upsert_budget(free, 1000, 100_000, 1.0).await.unwrap();

    queue(&store, premium, Lane::Standard, 600, "p").await;
    queue(&store, free, Lane::Standard, 600, "f").await;

    let mut scheduler = Scheduler::new(store.clone(), engine.clone(), config(12));
    for _ in 0..40 {
        scheduler.tick().await.unwrap();
        drain_running(&store).await;
    }

    let mut served: HashMap<Uuid, u32> = HashMap::new();
    for (id, _) in engine.started() {
        let item = store.get(id).await.unwrap().unwrap();
        *served.entry(item.tenant_id).or_default() += 1;
    }

    let p = served.get(&premium).copied().unwrap_or(0);
    let f = served.get(&free).copied().unwrap_or(0);
    assert!(f > 0, "the free tenant was never served");
    let ratio = f64::from(p) / f64::from(f);
    assert!(
        (4.0..=6.0).contains(&ratio),
        "ratio {ratio:.2} (premium {p}, free {f})"
    );
}

#[tokio::test]
async fn a_lane_never_exceeds_the_fleet_however_deep_the_queue() {
    let Some(store) = store_named("a_slot_ceiling").await else {
        return;
    };
    let engine = Arc::new(RecordingEngine::new());

    let t = Uuid::now_v7();
    store.upsert_budget(t, 100_000, 100_000, 1.0).await.unwrap();
    queue(&store, t, Lane::Realtime, 500, "r").await;

    let mut scheduler = Scheduler::new(store.clone(), engine, config(8));
    for _ in 0..5 {
        scheduler.tick().await.unwrap();
    }

    let holding = store.budget(t).await.unwrap().in_flight;
    assert_eq!(holding, 8, "the fleet was oversubscribed: {holding} of 8");
}

#[tokio::test]
async fn every_lane_keeps_its_guarantee_when_realtime_is_saturated() {
    let Some(store) = store_named("a_lane_guarantees").await else {
        return;
    };
    let engine = Arc::new(RecordingEngine::new());

    let t = Uuid::now_v7();
    store.upsert_budget(t, 100_000, 100_000, 1.0).await.unwrap();
    for (lane, tag) in [
        (Lane::Realtime, "r"),
        (Lane::Standard, "s"),
        (Lane::Bulk, "b"),
    ] {
        queue(&store, t, lane, 200, tag).await;
    }

    let mut scheduler = Scheduler::new(store.clone(), engine, config(100));
    scheduler.tick().await.unwrap();

    let mut per_lane: HashMap<Lane, u32> = HashMap::new();
    for item in store
        .list(Some(t), Some(WorkState::Running), 10_000)
        .await
        .unwrap()
    {
        *per_lane.entry(item.lane).or_default() += 1;
    }
    assert_eq!(per_lane.get(&Lane::Realtime).copied().unwrap_or(0), 40);
    assert_eq!(per_lane.get(&Lane::Standard).copied().unwrap_or(0), 50);
    assert_eq!(per_lane.get(&Lane::Bulk).copied().unwrap_or(0), 10);
}

#[tokio::test]
async fn a_scheduler_killed_between_claim_and_start_loses_nothing() {
    let Some(store) = store_named("a_kill_recovery").await else {
        return;
    };

    let t = Uuid::now_v7();
    store.upsert_budget(t, 100, 100_000, 1.0).await.unwrap();
    queue(&store, t, Lane::Standard, 5, "k").await;

    // Claim without starting anything — exactly what a process dying between
    // the two leaves behind.
    let claimed = store.claim(t, Lane::Standard, 5).await.unwrap();
    assert_eq!(claimed.len(), 5);
    assert_eq!(store.budget(t).await.unwrap().in_flight, 5);

    // A fresh scheduler sweeps them back and starts them exactly once.
    let engine = Arc::new(RecordingEngine::new());
    let mut scheduler = Scheduler::new(store.clone(), engine.clone(), config(64));
    assert_eq!(store.requeue_stalled(TimeDelta::zero()).await.unwrap(), 5);
    assert_eq!(store.budget(t).await.unwrap().in_flight, 0, "slots leaked");

    scheduler.tick().await.unwrap();

    let started: Vec<Uuid> = engine.started().iter().map(|(id, _)| *id).collect();
    let unique: std::collections::HashSet<_> = started.iter().copied().collect();
    assert_eq!(started.len(), 5, "work was lost");
    assert_eq!(unique.len(), 5, "work was started twice");
}

#[tokio::test]
async fn a_start_that_keeps_failing_is_requeued_then_failed_not_retried_forever() {
    let Some(store) = store_named("a_start_failure").await else {
        return;
    };

    let t = Uuid::now_v7();
    store.upsert_budget(t, 100, 100_000, 1.0).await.unwrap();
    queue(&store, t, Lane::Standard, 1, "f").await;

    let engine = Arc::new(RecordingEngine::new());
    engine.fail_next_starts(100);
    let mut scheduler = Scheduler::new(store.clone(), engine, config(64));

    let mut requeued = 0;
    let mut failed = 0;
    for _ in 0..10 {
        let t = scheduler.tick().await.unwrap();
        requeued += t.requeued;
        failed += t.failed;
    }

    assert!(requeued >= 2, "it gave up too early: {requeued} requeues");
    assert_eq!(failed, 1, "it never stopped retrying");
    let item = &store
        .list(Some(t), Some(WorkState::Failed), 10)
        .await
        .unwrap()[0];
    assert_eq!(item.attempts, 3, "attempts did not match max_attempts");
    assert_eq!(
        store.budget(t).await.unwrap().in_flight,
        0,
        "a failed start held a slot"
    );
}

#[tokio::test]
async fn cancelling_stops_the_workflow_and_returns_the_slot() {
    let Some(store) = store_named("a_cancel").await else {
        return;
    };

    let t = Uuid::now_v7();
    store.upsert_budget(t, 10, 100_000, 1.0).await.unwrap();
    queue(&store, t, Lane::Standard, 1, "c").await;

    let engine = Arc::new(RecordingEngine::new());
    let mut scheduler = Scheduler::new(store.clone(), engine.clone(), config(8));
    scheduler.tick().await.unwrap();

    let running = store
        .list(Some(t), Some(WorkState::Running), 10)
        .await
        .unwrap();
    assert_eq!(running.len(), 1);
    let item = &running[0];
    let workflow_id = item.workflow_id.clone().expect("workflow recorded");

    ferrite_scheduler::engine::WorkflowEngine::cancel(engine.as_ref(), &workflow_id)
        .await
        .unwrap();
    store
        .finish(item.id, WorkState::Canceled, None)
        .await
        .unwrap();

    assert_eq!(engine.canceled(), vec![workflow_id]);
    assert_eq!(
        store.get(item.id).await.unwrap().unwrap().state,
        WorkState::Canceled
    );
    assert_eq!(
        store.budget(t).await.unwrap().in_flight,
        0,
        "cancel leaked a slot"
    );
}

#[tokio::test]
async fn two_schedulers_on_one_queue_start_each_item_exactly_once() {
    let Some(store) = store_named("a_two_schedulers").await else {
        return;
    };

    let t = Uuid::now_v7();
    store.upsert_budget(t, 100_000, 100_000, 1.0).await.unwrap();
    queue(&store, t, Lane::Standard, 200, "d").await;

    let engine = Arc::new(RecordingEngine::new());
    let mut a = Scheduler::new(store.clone(), engine.clone(), config(200));
    let mut b = Scheduler::new(store.clone(), engine.clone(), config(200));

    for _ in 0..5 {
        let (ra, rb) = tokio::join!(a.tick(), b.tick());
        ra.unwrap();
        rb.unwrap();
    }

    let started: Vec<Uuid> = engine.started().iter().map(|(id, _)| *id).collect();
    let unique: std::collections::HashSet<_> = started.iter().copied().collect();
    assert_eq!(unique.len(), started.len(), "an item was started twice");
    assert_eq!(
        store.budget(t).await.unwrap().in_flight as usize,
        unique.len()
    );
}

#[tokio::test]
async fn ten_thousand_items_drain_without_losing_or_duplicating_any() {
    let Some(store) = store_named("a_ten_thousand").await else {
        return;
    };

    let tenants: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
    for (i, t) in tenants.iter().enumerate() {
        store
            .upsert_budget(*t, 1000, 1_000_000, 1.0 + i as f32)
            .await
            .unwrap();
        queue(&store, *t, Lane::Standard, 2_000, &format!("t{i}")).await;
    }

    let engine = Arc::new(RecordingEngine::new());
    let mut scheduler = Scheduler::new(store.clone(), engine.clone(), config(256));

    let mut ticks = 0;
    loop {
        scheduler.tick().await.unwrap();
        drain_running(&store).await;
        ticks += 1;

        let pending = store.list(None, Some(WorkState::Pending), 1).await.unwrap();
        if pending.is_empty() {
            break;
        }
        assert!(ticks < 500, "queue did not drain in {ticks} ticks");
    }

    let started: Vec<Uuid> = engine.started().iter().map(|(id, _)| *id).collect();
    let unique: std::collections::HashSet<_> = started.iter().copied().collect();
    assert_eq!(started.len(), 10_000, "items were lost");
    assert_eq!(unique.len(), 10_000, "items were started twice");

    for t in &tenants {
        assert_eq!(
            store.budget(*t).await.unwrap().in_flight,
            0,
            "a slot leaked"
        );
    }
    assert_eq!(
        store.reconcile_in_flight().await.unwrap(),
        0,
        "counters drifted"
    );
}
