//! Integration tests for the sync protocol — two instances syncing.

use chrono::Utc;
use openhuman_memory::config::Config;
use openhuman_memory::sync_protocol::changelog::{
    record_change, get_changes_since, merge_clock, ChangePayload, OpType,
};
use openhuman_memory::sync_protocol::crypto::{encrypt, decrypt, derive_session_key, x25519_key_agreement};
use openhuman_memory::sync_protocol::device::get_or_create_identity;
use openhuman_memory::sync_protocol::merge::{merge_batch, MergeBatchResult};
use tempfile::TempDir;

fn make_config() -> (TempDir, Config) {
    let tmp = TempDir::new().unwrap();
    let mut cfg = Config::default();
    cfg.workspace_dir = tmp.path().to_path_buf();
    (tmp, cfg)
}

/// Test: two instances exchange changes and merge correctly.
#[test]
fn two_instance_sync_roundtrip() {
    let (_tmp_a, cfg_a) = make_config();
    let (_tmp_b, cfg_b) = make_config();

    // Device A writes some changes
    let payload = ChangePayload {
        store: "kv".into(),
        key: "greeting".into(),
        namespace: None,
        value: Some(serde_json::json!("hello")),
    };
    record_change(&cfg_a, "device-a", OpType::Insert, &payload).unwrap();
    record_change(&cfg_a, "device-a", OpType::Update, &ChangePayload {
        store: "kv".into(),
        key: "greeting".into(),
        namespace: None,
        value: Some(serde_json::json!("hello world")),
    }).unwrap();

    // Device B writes different changes
    record_change(&cfg_b, "device-b", OpType::Insert, &ChangePayload {
        store: "documents".into(),
        key: "doc-1".into(),
        namespace: Some("notes".into()),
        value: Some(serde_json::json!({"title": "My Note"})),
    }).unwrap();

    // Pull A's changes (simulating what B would receive)
    let a_changes = get_changes_since(&cfg_a, 0).unwrap();
    assert_eq!(a_changes.len(), 2);

    // B merges A's changes
    let b_clock = 1u64; // B has seen up to ts=1
    let result = merge_batch(&cfg_b, "device-b", &a_changes, b_clock).unwrap();
    // KV changes should be accepted (LWW, remote ts > local ts)
    assert!(result.accepted > 0 || result.kept_local > 0);

    // Merge B's clock with A's latest
    let merged_clock = merge_clock(&cfg_b, 2).unwrap();
    assert!(merged_clock >= 3);
}

/// Test: simulated network partition and reconnect.
#[test]
fn network_partition_and_reconnect() {
    let (_tmp_a, cfg_a) = make_config();
    let (_tmp_b, cfg_b) = make_config();

    // Both devices write while "offline" (no sync happening)
    for i in 0..5 {
        record_change(&cfg_a, "device-a", OpType::Insert, &ChangePayload {
            store: "chunks".into(),
            key: format!("chunk-a-{}", i),
            namespace: None,
            value: Some(serde_json::json!({"text": format!("from A {}", i)})),
        }).unwrap();
    }
    for i in 0..3 {
        record_change(&cfg_b, "device-b", OpType::Insert, &ChangePayload {
            store: "chunks".into(),
            key: format!("chunk-b-{}", i),
            namespace: None,
            value: Some(serde_json::json!({"text": format!("from B {}", i)})),
        }).unwrap();
    }

    // "Reconnect" — exchange all changes
    let a_changes = get_changes_since(&cfg_a, 0).unwrap();
    let b_changes = get_changes_since(&cfg_b, 0).unwrap();

    assert_eq!(a_changes.len(), 5);
    assert_eq!(b_changes.len(), 3);

    // B merges A's changes (chunks are idempotent)
    let result_b = merge_batch(&cfg_b, "device-b", &a_changes, 3).unwrap();
    assert_eq!(result_b.skipped, 5); // All chunk inserts are idempotent

    // A merges B's changes
    let result_a = merge_batch(&cfg_a, "device-a", &b_changes, 5).unwrap();
    assert_eq!(result_a.skipped, 3);
}

/// Test: encrypted sync payload roundtrip using a pre-shared key.
/// (Full X25519 DH agreement requires `x25519-dalek` for correctness;
///  the simplified impl is a structural placeholder.)
#[test]
fn encrypted_sync_payload() {
    let (_tmp_a, cfg_a) = make_config();
    let (_tmp_b, cfg_b) = make_config();

    // Generate device identities (verifies key generation works)
    let id_a = get_or_create_identity(&cfg_a).unwrap();
    let id_b = get_or_create_identity(&cfg_b).unwrap();
    assert_ne!(id_a.device_id, id_b.device_id);

    // Use a pre-shared secret (simulating successful DH agreement)
    let shared_secret = [0xAB; 32];
    let session_key = derive_session_key(&shared_secret, b"sync-session-1");

    // Record some changes
    record_change(&cfg_a, "device-a", OpType::Insert, &ChangePayload {
        store: "kv".into(),
        key: "secret-key".into(),
        namespace: None,
        value: Some(serde_json::json!("secret-value")),
    }).unwrap();

    // Encrypt the changes for transit
    let changes = get_changes_since(&cfg_a, 0).unwrap();
    let payload = serde_json::to_vec(&changes).unwrap();
    let encrypted = encrypt(&session_key, &payload);

    // Verify encrypted data is different from plaintext
    assert_ne!(&encrypted[24..], &payload[..]);

    // Decrypt on the other side
    let decrypted = decrypt(&session_key, &encrypted).unwrap();
    let recovered: Vec<serde_json::Value> = serde_json::from_slice(&decrypted).unwrap();
    assert_eq!(recovered.len(), 1);
}

/// Performance benchmark: 1000 changes should sync in < 2s.
#[test]
fn performance_1000_changes() {
    let (_tmp, cfg) = make_config();

    let start = std::time::Instant::now();

    for i in 0..1000 {
        record_change(&cfg, "device-perf", OpType::Insert, &ChangePayload {
            store: "kv".into(),
            key: format!("key-{}", i),
            namespace: None,
            value: Some(serde_json::json!({"value": i})),
        }).unwrap();
    }

    let write_time = start.elapsed();

    let start2 = std::time::Instant::now();
    let changes = get_changes_since(&cfg, 0).unwrap();
    let read_time = start2.elapsed();

    assert_eq!(changes.len(), 1000);

    let total = write_time + read_time;
    assert!(
        total.as_secs() < 2,
        "1000 changes took {:?} (> 2s)",
        total
    );
}
