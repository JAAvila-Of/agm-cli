//! Scale and edge-case tests for `MemoryRuntime` (Gap 2).
//!
//! Covers: TTL/GC at scale, search-unsupported, scope filtering, topic
//! filtering, bulk delete, repeated upserts, and special-character values.

use agm_cli::runtime::memory::{GcReport, MemoryResult, MemoryRuntime};
use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_runtime(dir: &TempDir) -> MemoryRuntime {
    let project_path = dir.path().join("test.agm.mem");
    let global_path = dir.path().join("global.mem");
    MemoryRuntime::new(project_path, global_path, "test.pkg").expect("MemoryRuntime::new")
}

fn upsert(
    rt: &mut MemoryRuntime,
    node_id: &str,
    scope: MemoryScope,
    ttl: MemoryTtl,
    key: &str,
    topic: &str,
    value: &str,
) {
    let entry = MemoryEntry {
        key: key.to_owned(),
        topic: topic.to_owned(),
        action: MemoryAction::Upsert,
        value: Some(value.to_owned()),
        scope: Some(scope),
        ttl: Some(ttl),
        query: None,
        max_results: None,
    };
    let result = rt.execute_action(node_id, &entry).expect("upsert");
    assert_eq!(result, MemoryResult::Upserted);
}

fn delete(rt: &mut MemoryRuntime, node_id: &str, scope: MemoryScope, key: &str) -> bool {
    let entry = MemoryEntry {
        key: key.to_owned(),
        topic: String::new(),
        action: MemoryAction::Delete,
        value: None,
        scope: Some(scope),
        ttl: None,
        query: None,
        max_results: None,
    };
    match rt.execute_action(node_id, &entry).expect("delete") {
        MemoryResult::Deleted(b) => b,
        other => panic!("expected Deleted, got {other:?}"),
    }
}

fn list_scope(rt: &mut MemoryRuntime, node_id: &str, scope: MemoryScope, topic: &str) -> usize {
    let entry = MemoryEntry {
        key: String::new(),
        topic: topic.to_owned(),
        action: MemoryAction::List,
        value: None,
        scope: Some(scope),
        ttl: None,
        query: None,
        max_results: None,
    };
    match rt.execute_action(node_id, &entry).expect("list") {
        MemoryResult::List(v) => v.len(),
        other => panic!("expected List, got {other:?}"),
    }
}

fn get(rt: &mut MemoryRuntime, node_id: &str, scope: MemoryScope, key: &str) -> Option<String> {
    let entry = MemoryEntry {
        key: key.to_owned(),
        topic: String::new(),
        action: MemoryAction::Get,
        value: None,
        scope: Some(scope),
        ttl: None,
        query: None,
        max_results: None,
    };
    match rt.execute_action(node_id, &entry).expect("get") {
        MemoryResult::Value(Some(e)) => Some(e.value),
        MemoryResult::Value(None) => None,
        other => panic!("expected Value, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 2.2 TTL / GC at scale
// ---------------------------------------------------------------------------

/// Upsert 50 project-scoped entries: 25 with `P0D` (expires immediately) and
/// 25 permanent.  After calling `gc()`, exactly 25 should be removed and the
/// 25 permanent entries must remain.
#[test]
fn test_memory_gc_50_entries_25_expired_removes_exactly_25() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    // 25 entries that expire immediately (duration 0 seconds)
    for i in 0..25 {
        upsert(
            &mut rt,
            "node1",
            MemoryScope::Project,
            MemoryTtl::Duration("P0D".to_owned()),
            &format!("expired.key{i}"),
            "gc.topic",
            &format!("val{i}"),
        );
    }

    // 25 permanent entries
    for i in 0..25 {
        upsert(
            &mut rt,
            "node1",
            MemoryScope::Project,
            MemoryTtl::Permanent,
            &format!("permanent.key{i}"),
            "gc.topic",
            &format!("pval{i}"),
        );
    }

    // Verify all 50 are present before GC
    assert_eq!(
        list_scope(&mut rt, "node1", MemoryScope::Project, ""),
        50,
        "should have 50 entries before GC"
    );

    let report: GcReport = rt.gc();

    assert_eq!(
        report.expired_removed, 25,
        "GC should remove exactly 25 expired entries"
    );
    assert_eq!(
        report.orphan_removed, 0,
        "orphan_removed should be 0 (not implemented)"
    );

    // 25 permanent entries remain
    let remaining = list_scope(&mut rt, "node1", MemoryScope::Project, "");
    assert_eq!(remaining, 25, "25 permanent entries should remain after GC");

    // All remaining entries have Permanent TTL (spot-check first and last)
    for i in 0..25 {
        let val = get(
            &mut rt,
            "node1",
            MemoryScope::Project,
            &format!("permanent.key{i}"),
        );
        assert!(
            val.is_some(),
            "permanent.key{i} should still exist after GC"
        );
    }
}

// ---------------------------------------------------------------------------
// 2.3 Search unsupported
// ---------------------------------------------------------------------------

#[test]
fn test_memory_search_returns_unsupported() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    // Populate a few entries so the runtime is not empty
    upsert(
        &mut rt,
        "node1",
        MemoryScope::Session,
        MemoryTtl::Session,
        "search.key",
        "search.topic",
        "some value",
    );

    let search_entry = MemoryEntry {
        key: "search.key".to_owned(),
        topic: "search.topic".to_owned(),
        action: MemoryAction::Search,
        value: None,
        scope: Some(MemoryScope::Session),
        ttl: None,
        query: Some("some value".to_owned()),
        max_results: None,
    };

    let result = rt
        .execute_action("node1", &search_entry)
        .expect("execute search");
    assert_eq!(
        result,
        MemoryResult::SearchUnsupported,
        "Search should return SearchUnsupported"
    );
}

// ---------------------------------------------------------------------------
// 2.4 Scope filtering
// ---------------------------------------------------------------------------

/// Upsert 15 entries to project scope and 15 to session scope.
/// Listing each scope returns exactly 15 entries; no cross-contamination.
#[test]
fn test_memory_list_scope_filtering_30_mixed_entries() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    for i in 0..15 {
        upsert(
            &mut rt,
            "n",
            MemoryScope::Project,
            MemoryTtl::Permanent,
            &format!("proj.key{i}"),
            "scope.topic",
            &format!("pv{i}"),
        );
        upsert(
            &mut rt,
            "n",
            MemoryScope::Session,
            MemoryTtl::Session,
            &format!("sess.key{i}"),
            "scope.topic",
            &format!("sv{i}"),
        );
    }

    let project_count = list_scope(&mut rt, "n", MemoryScope::Project, "");
    let session_count = list_scope(&mut rt, "n", MemoryScope::Session, "");

    assert_eq!(project_count, 15, "project scope should have 15 entries");
    assert_eq!(session_count, 15, "session scope should have 15 entries");

    // No cross-contamination: project keys should not appear in session
    for i in 0..15 {
        assert!(
            get(&mut rt, "n", MemoryScope::Session, &format!("proj.key{i}")).is_none(),
            "proj.key{i} must not appear in session scope"
        );
        assert!(
            get(&mut rt, "n", MemoryScope::Project, &format!("sess.key{i}")).is_none(),
            "sess.key{i} must not appear in project scope"
        );
    }
}

/// Upsert 30 project-scoped entries across 3 topics (10 each).
/// List with a specific topic returns 10; list all (empty topic) returns 30.
#[test]
fn test_memory_list_topic_filtering_30_entries() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    let topics = ["alpha.topic", "beta.topic", "gamma.topic"];

    for (topic_idx, topic) in topics.iter().enumerate() {
        for i in 0..10 {
            upsert(
                &mut rt,
                "n",
                MemoryScope::Project,
                MemoryTtl::Permanent,
                &format!("key.t{topic_idx}.{i}"),
                topic,
                &format!("v{i}"),
            );
        }
    }

    // Each topic individually returns 10
    for topic in &topics {
        let count = list_scope(&mut rt, "n", MemoryScope::Project, topic);
        assert_eq!(count, 10, "topic '{topic}' should have 10 entries");
    }

    // Empty topic string returns all 30
    let total = list_scope(&mut rt, "n", MemoryScope::Project, "");
    assert_eq!(total, 30, "listing all (empty topic) should return 30 entries");
}

// ---------------------------------------------------------------------------
// 2.5 Delete at scale
// ---------------------------------------------------------------------------

/// Upsert 50 session-scoped entries, delete 20 of them.
/// Each delete returns `Deleted(true)`.  After deleting, 30 remain and the
/// deleted keys return `None` on get.
#[test]
fn test_memory_delete_20_of_50_keys_leaves_30() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    for i in 0..50 {
        upsert(
            &mut rt,
            "n",
            MemoryScope::Session,
            MemoryTtl::Session,
            &format!("del.key{i}"),
            "del.topic",
            &format!("v{i}"),
        );
    }

    // Delete entries 0..20
    for i in 0..20 {
        let removed = delete(&mut rt, "n", MemoryScope::Session, &format!("del.key{i}"));
        assert!(removed, "delete of del.key{i} should return true");
    }

    // 30 remain
    let remaining = list_scope(&mut rt, "n", MemoryScope::Session, "");
    assert_eq!(remaining, 30, "30 entries should remain after 20 deletes");

    // Deleted keys return None
    for i in 0..20 {
        let val = get(
            &mut rt,
            "n",
            MemoryScope::Session,
            &format!("del.key{i}"),
        );
        assert!(val.is_none(), "del.key{i} should be absent after delete");
    }

    // Surviving keys still exist
    for i in 20..50 {
        let val = get(
            &mut rt,
            "n",
            MemoryScope::Session,
            &format!("del.key{i}"),
        );
        assert!(val.is_some(), "del.key{i} should still exist");
    }
}

// ---------------------------------------------------------------------------
// 2.6 Repeated upsert
// ---------------------------------------------------------------------------

/// Upsert 50 keys. Update 25 with a new value. Update 10 of those again.
/// Verify: all 50 keys exist, updated keys hold the latest value, and
/// repeated upserts do not create duplicate entries.
#[test]
fn test_memory_upsert_50_keys_some_updated_multiple_times() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    // Initial upsert of 50 keys
    for i in 0..50 {
        upsert(
            &mut rt,
            "n",
            MemoryScope::Session,
            MemoryTtl::Session,
            &format!("rep.key{i}"),
            "rep.topic",
            "initial",
        );
    }

    // First update: keys 0..25
    for i in 0..25 {
        upsert(
            &mut rt,
            "n",
            MemoryScope::Session,
            MemoryTtl::Session,
            &format!("rep.key{i}"),
            "rep.topic",
            "updated-once",
        );
    }

    // Second update: keys 0..10
    for i in 0..10 {
        upsert(
            &mut rt,
            "n",
            MemoryScope::Session,
            MemoryTtl::Session,
            &format!("rep.key{i}"),
            "rep.topic",
            "updated-twice",
        );
    }

    // Total count: still 50 (no duplicates)
    let total = list_scope(&mut rt, "n", MemoryScope::Session, "");
    assert_eq!(total, 50, "upserts should not create duplicate entries");

    // Keys 0..10 → latest value = "updated-twice"
    for i in 0..10 {
        let val = get(&mut rt, "n", MemoryScope::Session, &format!("rep.key{i}"))
            .expect("key should exist");
        assert_eq!(
            val, "updated-twice",
            "rep.key{i} should have been updated twice"
        );
    }

    // Keys 10..25 → latest value = "updated-once"
    for i in 10..25 {
        let val = get(&mut rt, "n", MemoryScope::Session, &format!("rep.key{i}"))
            .expect("key should exist");
        assert_eq!(
            val, "updated-once",
            "rep.key{i} should have been updated once"
        );
    }

    // Keys 25..50 → still "initial"
    for i in 25..50 {
        let val = get(&mut rt, "n", MemoryScope::Session, &format!("rep.key{i}"))
            .expect("key should exist");
        assert_eq!(val, "initial", "rep.key{i} should still hold initial value");
    }
}

// ---------------------------------------------------------------------------
// 2.7 Special characters in values (flush + reload round-trip)
// ---------------------------------------------------------------------------

fn flush_and_reload(dir: &TempDir) -> MemoryRuntime {
    let project_path = dir.path().join("test.agm.mem");
    let global_path = dir.path().join("global.mem");
    MemoryRuntime::new(project_path, global_path, "test.pkg").expect("reload runtime")
}

/// A value containing newline characters must survive a flush-to-disk and
/// reload cycle unchanged.
#[test]
fn test_memory_upsert_value_with_newlines_roundtrips_correctly() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    let value = "line one\nline two\nline three";
    upsert(
        &mut rt,
        "n",
        MemoryScope::Project,
        MemoryTtl::Permanent,
        "special.newlines",
        "special.topic",
        value,
    );

    rt.flush().expect("flush");
    let mut rt2 = flush_and_reload(&dir);

    let recovered = get(&mut rt2, "n", MemoryScope::Project, "special.newlines")
        .expect("key should survive reload");
    assert_eq!(
        recovered, value,
        "value with newlines should roundtrip unchanged"
    );
}

/// A value containing colons must survive a flush-to-disk and reload cycle
/// unchanged.
#[test]
fn test_memory_upsert_value_with_colons_roundtrips_correctly() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    let value = "key: value: extra: suffix";
    upsert(
        &mut rt,
        "n",
        MemoryScope::Project,
        MemoryTtl::Permanent,
        "special.colons",
        "special.topic",
        value,
    );

    rt.flush().expect("flush");
    let mut rt2 = flush_and_reload(&dir);

    let recovered = get(&mut rt2, "n", MemoryScope::Project, "special.colons")
        .expect("key should survive reload");
    assert_eq!(
        recovered, value,
        "value with colons should roundtrip unchanged"
    );
}

/// A value containing quote characters must survive a flush-to-disk and
/// reload cycle unchanged.
#[test]
fn test_memory_upsert_value_with_quotes_roundtrips_correctly() {
    let dir = TempDir::new().unwrap();
    let mut rt = make_runtime(&dir);

    let value = r#"he said "hello" and she said 'bye'"#;
    upsert(
        &mut rt,
        "n",
        MemoryScope::Project,
        MemoryTtl::Permanent,
        "special.quotes",
        "special.topic",
        value,
    );

    rt.flush().expect("flush");
    let mut rt2 = flush_and_reload(&dir);

    let recovered = get(&mut rt2, "n", MemoryScope::Project, "special.quotes")
        .expect("key should survive reload");
    assert_eq!(
        recovered, value,
        "value with quotes should roundtrip unchanged"
    );
}
