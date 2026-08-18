//! Stage 0's "Rust can write to MinIO". Skips unless VERVE_S3_BUCKET is set,
//! so `cargo test` stays green on a machine with nothing running.

use verve_worker::storage::{self, Backend};

fn backend() -> Option<Backend> {
    std::env::var("VERVE_S3_BUCKET").ok()?;
    Backend::from_env().ok()
}

#[tokio::test]
async fn a_worker_can_write_read_list_and_delete() {
    let Some(backend) = backend() else {
        eprintln!("skipped: set VERVE_S3_BUCKET to run");
        return;
    };
    let op = storage::operator(&backend).expect("operator");

    let prefix = storage::rendition_prefix("t_spike", "as_stage0", "720p");
    let key = format!("{prefix}seg-00001.m4s");
    let body = b"not really a segment".to_vec();

    op.write(&key, body.clone()).await.expect("write");

    let read = op.read(&key).await.expect("read").to_vec();
    assert_eq!(read, body, "bytes changed in transit");

    let listed = op.list(&prefix).await.expect("list");
    assert!(
        listed.iter().any(|e| e.path() == key),
        "wrote {key} but listing {prefix} did not contain it"
    );

    op.delete(&key).await.expect("delete");
    assert!(
        !op.exists(&key).await.expect("exists"),
        "delete left the object"
    );
}

#[tokio::test]
async fn a_missing_object_is_an_error_not_empty_bytes() {
    let Some(backend) = backend() else { return };
    let op = storage::operator(&backend).expect("operator");
    assert!(op.read("t_spike/definitely/not/here").await.is_err());
}
