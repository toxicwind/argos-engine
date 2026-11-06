//! #874: the Prometheus gauges (`argos_doc_count` / `argos_segment_count` /
//! `argos_wal_size_bytes` / `argos_memory_usage_bytes`) must be live at scrape
//! time — and must not be fed by a background loop.
//!
//! They used to be written by a 10 s ticker whose per-tick work walked every
//! index's WAL subtree (`read_dir` + one `metadata()` per WAL shard — ~16 per
//! index at default sharding) on an async runtime worker. That is an idle CPU
//! cost that scales with index count: measured ~0.7 % of one core for 450
//! idle indices on the at-rest fixture (benchmarks/idle-budget/README.md),
//! against issue #874's < 0.5 % budget. The loop is gone; the `/v1/metrics`
//! handler refreshes the gauges itself.
//!
//! Fail-before shape: with the loop deleted and nothing refreshing at scrape
//! time, this test reports `argos_doc_count 0` on a node holding documents —
//! exactly the "flat zero at millions of docs" regression the loop was
//! originally added to fix, so this pins BOTH properties: live when scraped,
//! idle the rest of the time.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use argos_common::types::{FieldConfig, FieldType, Schema};

async fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = argos_common::config::Config::default();
    config.server.data_dir = dir.path().to_string_lossy().into_owned();
    config.storage.wal_sync = argos_common::config::WalSync::Async;
    let metrics = argos_common::metrics::Metrics::new().expect("metrics");
    let engine = argos_engine::Engine::new(config.clone()).expect("engine");
    let state = argos_api::state::AppState::new(config, engine, metrics);

    let mut schema = Schema::empty();
    schema
        .add_field(FieldConfig::new("body", FieldType::Text))
        .expect("body");
    state.engine.create_index("docs", schema).expect("create");
    let idx = state.engine.get_index("docs").expect("get");
    for i in 0..5 {
        idx.index_document(
            Some(format!("d{i}")),
            json!({"body": format!("document number {i} with searchable prose")}),
        )
        .await
        .expect("index");
    }
    idx.refresh().await.expect("refresh");
    (argos_api::router::build_es_compat_router(state), dir)
}

/// Last whitespace-separated field of the metric's sample line (Prometheus
/// text exposition: `name value`), skipping the `# HELP`/`# TYPE` lines.
fn gauge_value(text: &str, name: &str) -> Option<i64> {
    text.lines()
        .find(|l| l.starts_with(name) && !l.starts_with('#'))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}

#[tokio::test]
async fn scrape_reports_live_gauges_without_a_background_loop() {
    let (app, _dir) = app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();

    // doc_count must count the 5 documents indexed above (plus whatever the
    // system indices hold — hence >=, not ==). Before the scrape-time
    // refresh, this read 0 the moment the background loop was gone.
    let docs = gauge_value(&text, "argos_doc_count")
        .unwrap_or_else(|| panic!("argos_doc_count missing from /v1/metrics:\n{text}"));
    assert!(docs >= 5, "argos_doc_count = {docs}, expected >= 5:\n{text}");

    // One refreshed index => at least one segment; the WAL gauge must be
    // present and parseable (its value depends on the wal_sync mode).
    let segments = gauge_value(&text, "argos_segment_count")
        .unwrap_or_else(|| panic!("argos_segment_count missing from /v1/metrics:\n{text}"));
    assert!(
        segments >= 1,
        "argos_segment_count = {segments}, expected >= 1"
    );
    assert!(
        gauge_value(&text, "argos_wal_size_bytes").is_some(),
        "argos_wal_size_bytes missing from /v1/metrics:\n{text}"
    );
    assert!(
        gauge_value(&text, "argos_memory_usage_bytes").is_some(),
        "argos_memory_usage_bytes missing from /v1/metrics:\n{text}"
    );
}
