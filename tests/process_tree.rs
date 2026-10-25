#[path = "../src/process.rs"]
mod process;
use std::time::Duration;
const FIXTURE: &str = env!("CARGO_BIN_EXE_process-fixture");
#[tokio::test]
async fn fixture_stdout_is_bounded_and_timeout_enforced() {
    assert!(
        process::output(FIXTURE, &["flood"], Duration::from_secs(2), 1024)
            .await
            .unwrap_err()
            .contains("limit")
    );
    assert!(
        process::output(FIXTURE, &["stderr-flood"], Duration::from_secs(2), 1024)
            .await
            .unwrap_err()
            .contains("limit")
    );
    assert!(
        process::output(FIXTURE, &["hold"], Duration::from_millis(100), 1024)
            .await
            .unwrap_err()
            .contains("timed out")
    );
    let value = process::output(FIXTURE, &["result"], Duration::from_secs(2), 4096)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&value).unwrap()["ping"]["latency"],
        5
    );
}
#[tokio::test]
async fn cancellation_kills_descendant_even_when_parent_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("heartbeat");
    let child_path = path.clone();
    let task = tokio::spawn(async move {
        process::output(
            FIXTURE,
            &["family", child_path.to_str().unwrap()],
            Duration::from_secs(10),
            4096,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let length = std::fs::metadata(&path).unwrap().len();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        length,
        "descendant still running"
    );
}
