//! Row-level security. These tests fail if the policies are dropped, which is
//! the only way to know the isolation is real rather than assumed.

use ferrite_scheduler::model::{Lane, NewWork, WorkKind};
use ferrite_scheduler::store::{Scope, Store};
use serde_json::json;
use uuid::Uuid;

async fn store(schema: &str) -> Option<Store> {
    let url = std::env::var("SCHED_DATABASE_URL").ok()?;
    Some(
        Store::connect_fresh_schema(&url, schema)
            .await
            .expect("connect"),
    )
}

async fn tenant_with_work(store: &Store, dedupe: &str) -> Uuid {
    let id = Uuid::now_v7();
    store.upsert_budget(id, 10, 600, 1.0).await.expect("budget");
    store
        .submit(&NewWork {
            tenant_id: id,
            kind: WorkKind::Fake,
            ref_id: Uuid::now_v7(),
            spec: json!({}),
            lane: Lane::Standard,
            priority_key: 0,
            dedupe_key: dedupe.to_string(),
        })
        .await
        .expect("submit");
    id
}

#[tokio::test]
async fn a_tenant_scope_cannot_see_another_tenants_work() {
    let Some(store) = store("rls_isolation").await else {
        return;
    };
    let a = tenant_with_work(&store, "a").await;
    let b = tenant_with_work(&store, "b").await;

    let mut tx = store.scoped(Scope::Tenant(a)).await.expect("scope");
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT tenant_id FROM work")
        .fetch_all(&mut *tx)
        .await
        .expect("select");

    assert!(!rows.is_empty(), "tenant A cannot see its own work");
    assert!(
        rows.iter().all(|(id,)| *id == a),
        "tenant A saw {} rows belonging to someone else",
        rows.iter().filter(|(id,)| *id != a).count()
    );
    assert!(!rows.iter().any(|(id,)| *id == b));
}

#[tokio::test]
async fn a_query_with_no_scope_at_all_returns_nothing() {
    // The failure this exists for: a handler that forgets to scope its query
    // must come back empty, never with everyone's rows.
    let Some(store) = store("rls_unscoped").await else {
        return;
    };
    tenant_with_work(&store, "a").await;

    let unscoped: i64 = sqlx::query_scalar("SELECT count(*) FROM work")
        .fetch_one(store.pool())
        .await
        .expect("count");
    assert_eq!(unscoped, 0, "an unscoped query saw {unscoped} rows");
}

#[tokio::test]
async fn a_tenant_scope_cannot_write_into_another_tenant() {
    let Some(store) = store("rls_write").await else {
        return;
    };
    let a = tenant_with_work(&store, "a").await;
    let b = tenant_with_work(&store, "b").await;

    let mut tx = store.scoped(Scope::Tenant(a)).await.expect("scope");
    let stolen = sqlx::query("UPDATE work SET priority_key = -99 WHERE tenant_id = $1")
        .bind(b)
        .execute(&mut *tx)
        .await
        .expect("update");
    assert_eq!(
        stolen.rows_affected(),
        0,
        "tenant A rewrote tenant B's work"
    );

    // And an insert claiming another tenant is refused outright.
    let forged = sqlx::query(
        "INSERT INTO work (id, tenant_id, kind, ref_id, spec, lane, state, dedupe_key)
         VALUES ($1, $2, 'fake', $1, '{}', 'standard', 'pending', 'forged')",
    )
    .bind(Uuid::now_v7())
    .bind(b)
    .execute(&mut *tx)
    .await;
    assert!(forged.is_err(), "tenant A inserted a row owned by tenant B");
}

#[tokio::test]
async fn the_service_scope_sees_every_tenant() {
    // Admission compares tenants against each other; it has to see them all.
    let Some(store) = store("rls_service").await else {
        return;
    };
    let a = tenant_with_work(&store, "a").await;
    let b = tenant_with_work(&store, "b").await;

    let mut tx = store.scoped(Scope::Service).await.expect("scope");
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT tenant_id FROM work")
        .fetch_all(&mut *tx)
        .await
        .expect("select");

    assert!(rows.iter().any(|(id,)| *id == a));
    assert!(rows.iter().any(|(id,)| *id == b));
}

#[tokio::test]
async fn a_scope_does_not_leak_to_the_next_transaction() {
    // SET LOCAL, not SET: pooled connections are reused constantly.
    let Some(store) = store("rls_leak").await else {
        return;
    };
    let a = tenant_with_work(&store, "a").await;

    let mut scoped = store.scoped(Scope::Tenant(a)).await.expect("scope");
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM work")
        .fetch_one(&mut *scoped)
        .await
        .expect("count");
    assert_eq!(seen, 1);
    scoped.commit().await.expect("commit");

    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM work")
        .fetch_one(store.pool())
        .await
        .expect("count");
    assert_eq!(after, 0, "a tenant scope outlived its transaction");
}

#[tokio::test]
async fn budgets_and_costs_are_protected_too_not_just_work() {
    let Some(store) = store("rls_tables").await else {
        return;
    };
    let a = tenant_with_work(&store, "a").await;
    tenant_with_work(&store, "b").await;

    let mut tx = store.scoped(Scope::Tenant(a)).await.expect("scope");
    for query in [
        "SELECT tenant_id FROM tenant_budgets",
        "SELECT tenant_id FROM work_cost",
    ] {
        let rows: Vec<(Uuid,)> = sqlx::query_as(sqlx::AssertSqlSafe(query))
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_else(|e| panic!("{query}: {e}"));
        assert!(
            rows.iter().all(|(id,)| *id == a),
            "{query} leaked across tenants"
        );
    }
}

#[tokio::test]
async fn every_tenant_table_has_rls_forced_on() {
    // FORCE matters: without it the table owner, which is what the service
    // connects as, bypasses every policy.
    let Some(store) = store("rls_forced").await else {
        return;
    };

    for table in ["work", "tenant_budgets", "work_cost"] {
        let (enabled, forced): (bool, bool) = sqlx::query_as(
            "SELECT relrowsecurity, relforcerowsecurity FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE c.relname = $1 AND n.nspname = 'rls_forced'",
        )
        .bind(table)
        .fetch_one(store.pool())
        .await
        .unwrap_or_else(|e| panic!("{table}: {e}"));

        assert!(enabled, "{table} has row-level security disabled");
        assert!(forced, "{table} does not force it on the owner");
    }
}
