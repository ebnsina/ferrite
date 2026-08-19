//! The internal API over real HTTP. Skips unless SCHED_DATABASE_URL is set.

use ferrite_scheduler::api::{ApiState, router};
use ferrite_scheduler::engine::RecordingEngine;
use ferrite_scheduler::store::Store;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

struct Server {
    base: String,
    http: reqwest::Client,
    store: Store,
    engine: Arc<RecordingEngine>,
}

async fn serve(schema: &str) -> Option<Server> {
    let url = std::env::var("SCHED_DATABASE_URL").ok()?;
    let store = Store::connect_fresh_schema(&url, schema)
        .await
        .expect("connect");
    let engine = Arc::new(RecordingEngine::new());

    let app = router(ApiState {
        store: store.clone(),
        engine: engine.clone(),
        total_slots: 64,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });

    Some(Server {
        base: format!("http://{addr}"),
        http: reqwest::Client::new(),
        store,
        engine,
    })
}

fn body(tenant: Uuid, dedupe: &str) -> serde_json::Value {
    json!({
        "tenant_id": tenant,
        "kind": "fake",
        "ref_id": Uuid::now_v7(),
        "spec": {"steps": 2},
        "lane": "standard",
        "dedupe_key": dedupe,
    })
}

async fn budget(s: &Server, tenant: Uuid) {
    let res = s
        .http
        .put(format!("{}/internal/budgets/{tenant}", s.base))
        .json(&json!({
            "max_concurrent_tasks": 10,
            "rate_limit_per_min": 600,
            "fairness_weight": 1.0,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
}

#[tokio::test]
async fn submitting_twice_creates_once_and_says_so_in_the_status() {
    let Some(s) = serve("api_dedupe").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;

    let first = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 201);
    let created: serde_json::Value = first.json().await.unwrap();

    let second = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 200, "a duplicate looked like a new item");
    let existing: serde_json::Value = second.json().await.unwrap();
    assert_eq!(created["id"], existing["id"]);
}

#[tokio::test]
async fn a_tenant_with_no_budget_is_refused_rather_than_queued() {
    let Some(s) = serve("api_no_budget").await else {
        return;
    };
    let res = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(Uuid::now_v7(), "k"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
    let err: serde_json::Value = res.json().await.unwrap();
    assert_eq!(err["error"], "no_budget");
}

#[tokio::test]
async fn a_suspended_tenant_is_refused_with_403() {
    let Some(s) = serve("api_suspended").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;
    s.store.set_suspended(t, true).await.unwrap();

    let res = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn an_unknown_id_is_404_not_an_empty_body() {
    let Some(s) = serve("api_missing").await else {
        return;
    };
    let res = s
        .http
        .get(format!("{}/internal/work/{}", s.base, Uuid::now_v7()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn listing_filters_by_tenant_and_state() {
    let Some(s) = serve("api_list").await else {
        return;
    };
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    budget(&s, a).await;
    budget(&s, b).await;

    for i in 0..3 {
        s.http
            .post(format!("{}/internal/work", s.base))
            .json(&body(a, &format!("a{i}")))
            .send()
            .await
            .unwrap();
    }
    s.http
        .post(format!("{}/internal/work", s.base))
        .json(&body(b, "b0"))
        .send()
        .await
        .unwrap();

    let mine: Vec<serde_json::Value> = s
        .http
        .get(format!(
            "{}/internal/work?tenant_id={a}&state=pending",
            s.base
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mine.len(), 3);
    assert!(mine.iter().all(|w| w["tenant_id"] == a.to_string()));
}

#[tokio::test]
async fn an_unknown_state_filter_is_rejected_rather_than_ignored() {
    let Some(s) = serve("api_bad_state").await else {
        return;
    };
    let res = s
        .http
        .get(format!("{}/internal/work?state=nearly", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn cancelling_reaches_the_engine_before_the_row_changes() {
    let Some(s) = serve("api_cancel").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;

    let created: serde_json::Value = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    let claimed = s
        .store
        .claim(t, ferrite_scheduler::model::Lane::Standard, 1)
        .await
        .unwrap();
    s.store
        .mark_running(claimed[0].id, "wf-cancel-1")
        .await
        .unwrap();

    let res = s
        .http
        .post(format!("{}/internal/work/{id}/cancel", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let after: serde_json::Value = res.json().await.unwrap();

    assert_eq!(after["state"], "canceled");
    assert_eq!(s.engine.canceled(), vec!["wf-cancel-1".to_string()]);
    assert_eq!(
        s.store.budget(t).await.unwrap().in_flight,
        0,
        "cancel leaked a slot"
    );
}

#[tokio::test]
async fn cancelling_something_already_finished_is_not_an_error() {
    let Some(s) = serve("api_cancel_done").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;

    let created: serde_json::Value = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    s.store
        .finish(id, ferrite_scheduler::model::WorkState::Done, None)
        .await
        .unwrap();

    let res = s
        .http
        .post(format!("{}/internal/work/{id}/cancel", s.base))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let after: serde_json::Value = res.json().await.unwrap();
    assert_eq!(after["state"], "done", "cancel overwrote a finished item");
    assert!(
        s.engine.canceled().is_empty(),
        "it cancelled a workflow that had ended"
    );
}

#[tokio::test]
async fn finishing_releases_the_slot_and_records_what_it_cost() {
    let Some(s) = serve("api_finish").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;

    let created: serde_json::Value = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    s.store
        .claim(t, ferrite_scheduler::model::Lane::Standard, 1)
        .await
        .unwrap();

    let res = s
        .http
        .post(format!("{}/internal/work/{id}/finish", s.base))
        .json(&json!({
            "state": "done",
            "cpu_seconds": 42.5,
            "bytes_written": 1024,
            "machine": "worker-7",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    assert_eq!(s.store.budget(t).await.unwrap().in_flight, 0);
    let (cpu, machine): (f64, String) =
        sqlx::query_as("SELECT cpu_seconds, machine FROM work_cost WHERE work_id = $1")
            .bind(id)
            .fetch_one(s.store.pool())
            .await
            .unwrap();
    assert_eq!(cpu, 42.5);
    assert_eq!(machine, "worker-7");
}

#[tokio::test]
async fn finishing_into_a_non_terminal_state_is_refused() {
    let Some(s) = serve("api_finish_bad").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;
    let created: serde_json::Value = s
        .http
        .post(format!("{}/internal/work", s.base))
        .json(&body(t, "k"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();

    let res = s
        .http
        .post(format!("{}/internal/work/{id}/finish", s.base))
        .json(&json!({"state": "running"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn capacity_reports_every_lane_even_when_empty() {
    let Some(s) = serve("api_capacity").await else {
        return;
    };
    let t = Uuid::now_v7();
    budget(&s, t).await;
    for i in 0..4 {
        s.http
            .post(format!("{}/internal/work", s.base))
            .json(&body(t, &format!("k{i}")))
            .send()
            .await
            .unwrap();
    }
    s.store
        .claim(t, ferrite_scheduler::model::Lane::Standard, 2)
        .await
        .unwrap();

    let report: serde_json::Value = s
        .http
        .get(format!("{}/internal/capacity", s.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let lanes = report["lanes"].as_array().unwrap();
    assert_eq!(lanes.len(), 3, "a lane went missing when it had no work");
    assert_eq!(lanes[0]["lane"], "realtime");
    let standard = lanes.iter().find(|l| l["lane"] == "standard").unwrap();
    assert_eq!(standard["waiting"], 2);
    assert_eq!(standard["running"], 2);
    assert_eq!(report["total_slots"], 64);
    assert!(report["utilization"].as_f64().unwrap() > 0.0);
}
