//! Memory runtime for AGM orchestration (Phase 2, spec S28).
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use agm_core::model::mem_file::{MemFile, MemFileEntry};
use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
use agm_core::parser::mem::parse_mem;
use agm_core::renderer::mem::render_mem;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_DAY: u64 = 86_400;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// The outcome of a memory operation.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryResult {
    Value(Option<MemFileEntry>),
    Upserted,
    Deleted(bool),
    List(Vec<(String, MemFileEntry)>),
    SearchUnsupported,
}

/// Report from a garbage-collection pass.
#[derive(Debug, Clone, PartialEq)]
pub struct GcReport {
    pub expired_removed: usize,
    pub orphan_removed: usize,
}

// ---------------------------------------------------------------------------
// MemoryRuntime struct
// ---------------------------------------------------------------------------

/// Runtime manager for all memory scopes (node, session, project, global).
pub struct MemoryRuntime {
    /// Per-node in-memory key/value stores (scope = Node).
    node_store: HashMap<String, HashMap<String, MemFileEntry>>,
    /// Session-scoped in-memory key/value store.
    session_store: HashMap<String, MemFileEntry>,
    /// Path of the project `.agm.mem` file.
    project_path: PathBuf,
    /// Loaded/cached project-scoped store.
    project_store: MemFile,
    /// Path of the global `~/.agm/global.mem` file.
    global_path: PathBuf,
    /// Loaded/cached global-scoped store.
    global_store: MemFile,
    /// AGM package identifier for new MemFile headers.
    package: String,
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn now_iso8601() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC 3339 formatting cannot fail")
}

/// Returns `path` with `.mem` appended, e.g. `foo.agm` → `foo.agm.mem`.
fn mem_path(agm_path: &Path) -> PathBuf {
    let mut p = agm_path.as_os_str().to_owned();
    p.push(".mem");
    PathBuf::from(p)
}

/// Derives the `.agm.mem` sidecar path from the `.agm` file path (public for CLI).
pub fn mem_path_from_agm(agm_path: &Path) -> PathBuf {
    mem_path(agm_path)
}

/// Returns `~/.agm/global.mem`, using `HOME` (Unix) or `USERPROFILE` (Windows)
/// as the home directory.
pub fn global_mem_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Cannot determine home directory (HOME / USERPROFILE not set)")?;
    Ok(PathBuf::from(home).join(".agm").join("global.mem"))
}

fn empty_mem_file(package: &str) -> MemFile {
    MemFile {
        format_version: "1.0".to_owned(),
        package: package.to_owned(),
        updated_at: now_iso8601(),
        entries: std::collections::BTreeMap::new(),
    }
}

fn load_mem_file(path: &Path, package: &str) -> Result<MemFile> {
    if !path.exists() {
        return Ok(empty_mem_file(package));
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read mem file: {}", path.display()))?;
    parse_mem(&content).map_err(|errors| {
        anyhow::anyhow!(
            "Failed to parse mem file {}: {}",
            path.display(),
            errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })
}

/// Atomically writes `content` to `path` via a `.tmp` sibling file.
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("mem")
    ));
    {
        let mut file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("Failed to create tmp file: {}", tmp_path.display()))?;
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to write tmp file: {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync tmp file: {}", tmp_path.display()))?;
    }
    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "Failed to rename {} -> {}",
            tmp_path.display(),
            path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Creates a new [`MemoryRuntime`], loading existing sidecar files if present.
    ///
    /// Creates the `~/.agm/` directory (parent of `global_path`) if it does not exist.
    pub fn new(project_path: PathBuf, global_path: PathBuf, package: &str) -> Result<Self> {
        // Ensure the global directory exists
        if let Some(parent) = global_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create global memory directory: {}",
                    parent.display()
                )
            })?;
        }

        let project_store = load_mem_file(&project_path, package)?;
        let global_store = load_mem_file(&global_path, package)?;

        Ok(Self {
            node_store: HashMap::new(),
            session_store: HashMap::new(),
            project_path,
            project_store,
            global_path,
            global_store,
            package: package.to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// Core operations
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    fn get(&self, node_id: &str, scope: &MemoryScope, key: &str) -> Result<MemoryResult> {
        let entry = match scope {
            MemoryScope::Node => self
                .node_store
                .get(node_id)
                .and_then(|m| m.get(key))
                .cloned(),
            MemoryScope::Session => self.session_store.get(key).cloned(),
            MemoryScope::Project => self.project_store.entries.get(key).cloned(),
            MemoryScope::Global => self.global_store.entries.get(key).cloned(),
        };
        Ok(MemoryResult::Value(entry))
    }

    fn upsert(
        &mut self,
        node_id: &str,
        scope: &MemoryScope,
        key: &str,
        topic: &str,
        ttl: &MemoryTtl,
        value: &str,
    ) -> Result<MemoryResult> {
        let now = now_iso8601();

        match scope {
            MemoryScope::Node => {
                let node_map = self.node_store.entry(node_id.to_owned()).or_default();
                let created_at = node_map
                    .get(key)
                    .map(|e| e.created_at.clone())
                    .unwrap_or_else(|| now.clone());
                node_map.insert(
                    key.to_owned(),
                    MemFileEntry {
                        topic: topic.to_owned(),
                        scope: scope.clone(),
                        ttl: ttl.clone(),
                        value: value.to_owned(),
                        created_at,
                        updated_at: now,
                    },
                );
            }
            MemoryScope::Session => {
                let created_at = self
                    .session_store
                    .get(key)
                    .map(|e| e.created_at.clone())
                    .unwrap_or_else(|| now.clone());
                self.session_store.insert(
                    key.to_owned(),
                    MemFileEntry {
                        topic: topic.to_owned(),
                        scope: scope.clone(),
                        ttl: ttl.clone(),
                        value: value.to_owned(),
                        created_at,
                        updated_at: now,
                    },
                );
            }
            MemoryScope::Project => {
                let created_at = self
                    .project_store
                    .entries
                    .get(key)
                    .map(|e| e.created_at.clone())
                    .unwrap_or_else(|| now.clone());
                self.project_store.entries.insert(
                    key.to_owned(),
                    MemFileEntry {
                        topic: topic.to_owned(),
                        scope: scope.clone(),
                        ttl: ttl.clone(),
                        value: value.to_owned(),
                        created_at,
                        updated_at: now.clone(),
                    },
                );
                self.project_store.updated_at = now;
            }
            MemoryScope::Global => {
                let created_at = self
                    .global_store
                    .entries
                    .get(key)
                    .map(|e| e.created_at.clone())
                    .unwrap_or_else(|| now.clone());
                self.global_store.entries.insert(
                    key.to_owned(),
                    MemFileEntry {
                        topic: topic.to_owned(),
                        scope: scope.clone(),
                        ttl: ttl.clone(),
                        value: value.to_owned(),
                        created_at,
                        updated_at: now.clone(),
                    },
                );
                self.global_store.updated_at = now;
            }
        }

        Ok(MemoryResult::Upserted)
    }

    fn delete(&mut self, node_id: &str, scope: &MemoryScope, key: &str) -> Result<MemoryResult> {
        let removed = match scope {
            MemoryScope::Node => self
                .node_store
                .get_mut(node_id)
                .and_then(|m| m.remove(key))
                .is_some(),
            MemoryScope::Session => self.session_store.remove(key).is_some(),
            MemoryScope::Project => self.project_store.entries.remove(key).is_some(),
            MemoryScope::Global => self.global_store.entries.remove(key).is_some(),
        };
        Ok(MemoryResult::Deleted(removed))
    }

    fn list(&self, node_id: &str, scope: &MemoryScope, topic: &str) -> Result<MemoryResult> {
        let entries: Vec<(String, MemFileEntry)> = match scope {
            MemoryScope::Node => self
                .node_store
                .get(node_id)
                .map(|m| {
                    m.iter()
                        .filter(|(_, e)| topic.is_empty() || e.topic == topic)
                        .map(|(k, e)| (k.clone(), e.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            MemoryScope::Session => self
                .session_store
                .iter()
                .filter(|(_, e)| topic.is_empty() || e.topic == topic)
                .map(|(k, e)| (k.clone(), e.clone()))
                .collect(),
            MemoryScope::Project => self
                .project_store
                .entries
                .iter()
                .filter(|(_, e)| topic.is_empty() || e.topic == topic)
                .map(|(k, e)| (k.clone(), e.clone()))
                .collect(),
            MemoryScope::Global => self
                .global_store
                .entries
                .iter()
                .filter(|(_, e)| topic.is_empty() || e.topic == topic)
                .map(|(k, e)| (k.clone(), e.clone()))
                .collect(),
        };
        Ok(MemoryResult::List(entries))
    }
}

// ---------------------------------------------------------------------------
// execute_action dispatch
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Dispatches a [`MemoryEntry`] to the appropriate core operation.
    ///
    /// Defaults: scope = Session, TTL = Session when `None`.
    pub fn execute_action(&mut self, node_id: &str, entry: &MemoryEntry) -> Result<MemoryResult> {
        let scope = entry.scope.as_ref().unwrap_or(&MemoryScope::Session);
        let ttl = entry.ttl.as_ref().unwrap_or(&MemoryTtl::Session);

        match &entry.action {
            MemoryAction::Get => self.get(node_id, scope, &entry.key),
            MemoryAction::Upsert => {
                let value = entry.value.as_deref().unwrap_or("");
                self.upsert(node_id, scope, &entry.key, &entry.topic, ttl, value)
            }
            MemoryAction::Delete => self.delete(node_id, scope, &entry.key),
            MemoryAction::List => self.list(node_id, scope, &entry.topic),
            MemoryAction::Search => Ok(MemoryResult::SearchUnsupported),
        }
    }
}

// ---------------------------------------------------------------------------
// Scope clearing
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Removes all node-scoped entries for `node_id`.
    pub fn clear_node_scope(&mut self, node_id: &str) {
        self.node_store.remove(node_id);
    }

    /// Removes all session-scoped entries.
    pub fn clear_session_scope(&mut self) {
        self.session_store.clear();
    }

    /// Removes all node-scoped entries for `node_id` (used on node reset).
    pub fn clear_node_on_reset(&mut self, node_id: &str) {
        self.clear_node_scope(node_id);
    }

    /// Returns all `(key, value)` pairs across session, project, and global
    /// stores that belong to `topic`. Node-scoped entries are excluded because
    /// they are ephemeral within a single node execution.
    pub fn list_topic(&self, topic: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        for (key, entry) in &self.session_store {
            if entry.topic == topic {
                results.push((key.clone(), entry.value.clone()));
            }
        }
        for (key, entry) in &self.project_store.entries {
            if entry.topic == topic {
                results.push((key.clone(), entry.value.clone()));
            }
        }
        for (key, entry) in &self.global_store.entries {
            if entry.topic == topic {
                results.push((key.clone(), entry.value.clone()));
            }
        }
        results
    }
}

// ---------------------------------------------------------------------------
// ISO 8601 duration parser
// ---------------------------------------------------------------------------

/// Parses a limited subset of ISO 8601 duration strings into seconds.
///
/// Supported patterns:
/// - `P{n}D` — days
/// - `PT{n}H` — hours
/// - `PT{n}M` — minutes
/// - `P{n}DT{n}H` — days + hours
/// - `P{n}DT{n}H{n}M` — days + hours + minutes
/// - `PT{n}H{n}M` — hours + minutes
///
/// Rejected: months (`P1M` before T), years (`P1Y`), empty (`P` or `PT`), seconds.
fn parse_iso8601_duration(input: &str) -> Result<u64, String> {
    if !input.starts_with('P') {
        return Err(format!("Duration must start with 'P': {input}"));
    }

    let rest = &input[1..]; // strip leading 'P'

    // Reject years and months (letters before 'T')
    let t_pos = rest.find('T');
    let date_part = match t_pos {
        Some(pos) => &rest[..pos],
        None => rest,
    };

    if date_part.contains('Y') {
        return Err(format!("Years not supported in duration: {input}"));
    }
    if date_part.contains('M') {
        return Err(format!("Months not supported in duration: {input}"));
    }

    let mut total_secs: u64 = 0;

    // Parse date part for days
    if !date_part.is_empty() {
        if let Some(stripped) = date_part.strip_suffix('D') {
            let days: u64 = stripped
                .parse()
                .map_err(|_| format!("Invalid day count in: {input}"))?;
            total_secs += days * SECS_PER_DAY;
        } else {
            return Err(format!("Unrecognised date part in duration: {input}"));
        }
    }

    // Parse time part
    match t_pos {
        None => {
            // No 'T' — only days allowed; but empty (just "P") is an error
            if date_part.is_empty() {
                return Err(format!("Empty duration: {input}"));
            }
        }
        Some(pos) => {
            let time_part = &rest[pos + 1..]; // strip 'T'
            if time_part.is_empty() {
                return Err(format!("Empty time part in duration: {input}"));
            }

            // Reject seconds (anything ending in 'S')
            if time_part.contains('S') {
                return Err(format!("Seconds not supported in duration: {input}"));
            }

            // Parse hours and minutes from time_part
            // Valid combos: {n}H, {n}M, {n}H{n}M
            let (hours, minutes) = parse_time_part(time_part, input)?;
            total_secs += hours * SECS_PER_HOUR + minutes * SECS_PER_MINUTE;
        }
    }

    Ok(total_secs)
}

/// Parses the time component of an ISO 8601 duration (the part after `T`).
///
/// Returns `(hours, minutes)`.
fn parse_time_part(time_part: &str, original: &str) -> Result<(u64, u64), String> {
    let mut hours = 0u64;
    let mut minutes = 0u64;

    let mut remaining = time_part;

    if let Some(h_pos) = remaining.find('H') {
        let h_str = &remaining[..h_pos];
        hours = h_str
            .parse()
            .map_err(|_| format!("Invalid hour count in: {original}"))?;
        remaining = &remaining[h_pos + 1..];
    }

    if let Some(m_pos) = remaining.find('M') {
        let m_str = &remaining[..m_pos];
        minutes = m_str
            .parse()
            .map_err(|_| format!("Invalid minute count in: {original}"))?;
        remaining = &remaining[m_pos + 1..];
    }

    if !remaining.is_empty() {
        return Err(format!(
            "Unrecognised time component in duration: {original}"
        ));
    }

    if hours == 0 && minutes == 0 && time_part != "0H" && time_part != "0M" {
        // Ensure at least one component was parsed
        if !time_part.contains('H') && !time_part.contains('M') {
            return Err(format!(
                "Empty or unrecognised time part in duration: {original}"
            ));
        }
    }

    Ok((hours, minutes))
}

// ---------------------------------------------------------------------------
// TTL expiration
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Removes all entries whose `Duration` TTL has expired.
    ///
    /// Returns the number of entries removed.
    pub fn expire(&mut self) -> usize {
        let now = OffsetDateTime::now_utc();
        let mut removed = 0usize;

        // Node store
        for node_map in self.node_store.values_mut() {
            node_map.retain(|_, entry| {
                if !is_expired(entry, now) {
                    return true;
                }
                removed += 1;
                false
            });
        }

        // Session store
        self.session_store.retain(|_, entry| {
            if !is_expired(entry, now) {
                return true;
            }
            removed += 1;
            false
        });

        // Project store
        let before = self.project_store.entries.len();
        self.project_store.entries.retain(|_, entry| {
            if !is_expired(entry, now) {
                return true;
            }
            false
        });
        let project_removed = before - self.project_store.entries.len();
        if project_removed > 0 {
            self.project_store.updated_at = now_iso8601();
        }
        removed += project_removed;

        // Global store
        let before = self.global_store.entries.len();
        self.global_store.entries.retain(|_, entry| {
            if !is_expired(entry, now) {
                return true;
            }
            false
        });
        let global_removed = before - self.global_store.entries.len();
        if global_removed > 0 {
            self.global_store.updated_at = now_iso8601();
        }
        removed += global_removed;

        removed
    }
}

/// Returns `true` if the entry has a `Duration` TTL that has expired.
///
/// Skips entries with unparseable durations or `updated_at` timestamps.
fn is_expired(entry: &MemFileEntry, now: OffsetDateTime) -> bool {
    let duration_str = match &entry.ttl {
        MemoryTtl::Duration(d) => d.as_str(),
        _ => return false, // Permanent and Session TTLs are never expired here
    };

    let secs = match parse_iso8601_duration(duration_str) {
        Ok(s) => s,
        Err(_) => return false, // Skip unparseable durations
    };

    let updated = match OffsetDateTime::parse(&entry.updated_at, &Rfc3339) {
        Ok(ts) => ts,
        Err(_) => return false, // Skip unparseable timestamps
    };

    let elapsed = now - updated;
    elapsed.whole_seconds() >= 0 && elapsed.whole_seconds() as u64 >= secs
}

// ---------------------------------------------------------------------------
// GC
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Runs garbage collection: expires duration-TTL entries.
    ///
    /// `orphan_removed` is always 0 (orphan detection not implemented yet).
    pub fn gc(&mut self) -> GcReport {
        let expired_removed = self.expire();
        GcReport {
            expired_removed,
            orphan_removed: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

impl MemoryRuntime {
    /// Flushes project and global stores to disk atomically.
    ///
    /// Updates `updated_at` on both stores before rendering.
    pub fn flush(&mut self) -> Result<()> {
        let now = now_iso8601();
        self.project_store.updated_at = now.clone();
        self.global_store.updated_at = now;

        // Ensure global directory exists
        if let Some(parent) = self.global_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create global memory directory: {}",
                    parent.display()
                )
            })?;
        }

        let project_content = render_mem(&self.project_store);
        atomic_write(&self.project_path, &project_content).with_context(|| {
            format!(
                "Failed to flush project store: {}",
                self.project_path.display()
            )
        })?;

        let global_content = render_mem(&self.global_store);
        atomic_write(&self.global_path, &global_content).with_context(|| {
            format!(
                "Failed to flush global store: {}",
                self.global_path.display()
            )
        })?;

        Ok(())
    }

    /// Returns a reference to the project-scoped [`MemFile`].
    pub fn project_store(&self) -> &MemFile {
        &self.project_store
    }

    /// Returns a reference to the global-scoped [`MemFile`].
    pub fn global_store(&self) -> &MemFile {
        &self.global_store
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn test_runtime() -> (MemoryRuntime, TempDir) {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let runtime =
            MemoryRuntime::new(project_path, global_path, "test.pkg").expect("MemoryRuntime::new");
        (runtime, dir)
    }

    fn upsert_entry(
        runtime: &mut MemoryRuntime,
        node_id: &str,
        scope: MemoryScope,
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
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        };
        runtime.execute_action(node_id, &entry).unwrap();
    }

    fn get_entry(
        runtime: &MemoryRuntime,
        node_id: &str,
        scope: MemoryScope,
        key: &str,
    ) -> Option<MemFileEntry> {
        match runtime.get(node_id, &scope, key).unwrap() {
            MemoryResult::Value(v) => v,
            _ => panic!("expected Value result"),
        }
    }

    fn delete_entry(
        runtime: &mut MemoryRuntime,
        node_id: &str,
        scope: MemoryScope,
        key: &str,
    ) -> bool {
        let entry = MemoryEntry {
            key: key.to_owned(),
            topic: "t".to_owned(),
            action: MemoryAction::Delete,
            value: None,
            scope: Some(scope),
            ttl: None,
            query: None,
            max_results: None,
        };
        match runtime.execute_action(node_id, &entry).unwrap() {
            MemoryResult::Deleted(b) => b,
            _ => panic!("expected Deleted result"),
        }
    }

    fn list_entry(
        runtime: &MemoryRuntime,
        node_id: &str,
        scope: MemoryScope,
        topic: &str,
    ) -> Vec<(String, MemFileEntry)> {
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
        // list uses &self through get
        match runtime
            .list(node_id, &entry.scope.as_ref().unwrap(), &entry.topic)
            .unwrap()
        {
            MemoryResult::List(v) => v,
            _ => panic!("expected List result"),
        }
    }

    // -----------------------------------------------------------------------
    // Group A: Constructor
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_path_appends_mem_extension() {
        let p = Path::new("/some/path/file.agm");
        let result = mem_path(p);
        assert_eq!(result, PathBuf::from("/some/path/file.agm.mem"));
    }

    #[test]
    fn test_new_no_existing_files_creates_empty_stores() {
        let (runtime, _dir) = test_runtime();
        assert!(runtime.project_store.entries.is_empty());
        assert!(runtime.global_store.entries.is_empty());
        assert!(runtime.node_store.is_empty());
        assert!(runtime.session_store.is_empty());
    }

    #[test]
    fn test_new_loads_existing_project_file() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");

        // Write a project file first
        {
            let mut runtime =
                MemoryRuntime::new(project_path.clone(), global_path.clone(), "test.pkg")
                    .expect("first runtime");
            runtime
                .upsert(
                    "node1",
                    &MemoryScope::Project,
                    "k1",
                    "topic1",
                    &MemoryTtl::Permanent,
                    "v1",
                )
                .unwrap();
            runtime.flush().unwrap();
        }

        // Load a new runtime — should see the persisted entry
        let runtime2 =
            MemoryRuntime::new(project_path, global_path, "test.pkg").expect("second runtime");
        assert!(runtime2.project_store.entries.contains_key("k1"));
    }

    // -----------------------------------------------------------------------
    // Group B: Upsert + Get roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_upsert_get_node_scope_roundtrip() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "node1",
            MemoryScope::Node,
            "key1",
            "topic",
            "value1",
        );
        let entry = get_entry(&runtime, "node1", MemoryScope::Node, "key1");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, "value1");
    }

    #[test]
    fn test_upsert_get_session_scope_roundtrip() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "node1",
            MemoryScope::Session,
            "key1",
            "topic",
            "hello",
        );
        let entry = get_entry(&runtime, "node1", MemoryScope::Session, "key1");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, "hello");
    }

    #[test]
    fn test_upsert_get_project_scope_roundtrip() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "node1",
            MemoryScope::Project,
            "pk1",
            "t",
            "proj_val",
        );
        let entry = get_entry(&runtime, "node1", MemoryScope::Project, "pk1");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, "proj_val");
    }

    #[test]
    fn test_upsert_get_global_scope_roundtrip() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "node1",
            MemoryScope::Global,
            "gk1",
            "t",
            "global_val",
        );
        let entry = get_entry(&runtime, "node1", MemoryScope::Global, "gk1");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, "global_val");
    }

    #[test]
    fn test_get_node_scope_isolates_by_node_id() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "nodeA",
            MemoryScope::Node,
            "k",
            "t",
            "a_value",
        );
        upsert_entry(
            &mut runtime,
            "nodeB",
            MemoryScope::Node,
            "k",
            "t",
            "b_value",
        );

        let a = get_entry(&runtime, "nodeA", MemoryScope::Node, "k");
        let b = get_entry(&runtime, "nodeB", MemoryScope::Node, "k");
        assert_eq!(a.unwrap().value, "a_value");
        assert_eq!(b.unwrap().value, "b_value");
    }

    // -----------------------------------------------------------------------
    // Group C: Upsert update
    // -----------------------------------------------------------------------

    #[test]
    fn test_upsert_existing_key_updates_value() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k", "t", "v1");
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k", "t", "v2");
        let entry = get_entry(&runtime, "n", MemoryScope::Session, "k");
        assert_eq!(entry.unwrap().value, "v2");
    }

    #[test]
    fn test_upsert_existing_key_preserves_created_at() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k", "t", "v1");
        let first = get_entry(&runtime, "n", MemoryScope::Session, "k").unwrap();
        let original_created = first.created_at.clone();

        // Small sleep is unavoidable here to ensure timestamps differ
        std::thread::sleep(std::time::Duration::from_millis(10));

        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k", "t", "v2");
        let second = get_entry(&runtime, "n", MemoryScope::Session, "k").unwrap();
        assert_eq!(second.created_at, original_created);
    }

    // -----------------------------------------------------------------------
    // Group D: Delete
    // -----------------------------------------------------------------------

    #[test]
    fn test_delete_existing_key_returns_true() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k", "t", "v");
        let removed = delete_entry(&mut runtime, "n", MemoryScope::Session, "k");
        assert!(removed);
    }

    #[test]
    fn test_delete_nonexistent_key_returns_false() {
        let (mut runtime, _dir) = test_runtime();
        let removed = delete_entry(&mut runtime, "n", MemoryScope::Session, "nonexistent");
        assert!(!removed);
    }

    // -----------------------------------------------------------------------
    // Group E: List
    // -----------------------------------------------------------------------

    #[test]
    fn test_list_filters_by_topic() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(
            &mut runtime,
            "n",
            MemoryScope::Session,
            "k1",
            "topicA",
            "v1",
        );
        upsert_entry(
            &mut runtime,
            "n",
            MemoryScope::Session,
            "k2",
            "topicB",
            "v2",
        );
        upsert_entry(
            &mut runtime,
            "n",
            MemoryScope::Session,
            "k3",
            "topicA",
            "v3",
        );

        let results = list_entry(&runtime, "n", MemoryScope::Session, "topicA");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, e)| e.topic == "topicA"));
    }

    #[test]
    fn test_list_empty_scope_returns_empty() {
        let (runtime, _dir) = test_runtime();
        let results = list_entry(&runtime, "n", MemoryScope::Session, "anything");
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // Group F: Scope clearing
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_node_scope_removes_only_target_node() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "nodeA", MemoryScope::Node, "k", "t", "a");
        upsert_entry(&mut runtime, "nodeB", MemoryScope::Node, "k", "t", "b");
        runtime.clear_node_scope("nodeA");
        assert!(
            runtime
                .node_store
                .get("nodeA")
                .map(|m| m.is_empty())
                .unwrap_or(true)
        );
        assert!(
            runtime
                .node_store
                .get("nodeB")
                .map(|m| !m.is_empty())
                .unwrap_or(false)
        );
    }

    #[test]
    fn test_clear_session_scope_removes_all_session_entries() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k1", "t", "v1");
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "k2", "t", "v2");
        runtime.clear_session_scope();
        assert!(runtime.session_store.is_empty());
    }

    #[test]
    fn test_clear_session_scope_preserves_project_and_global() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Project, "pk", "t", "pv");
        upsert_entry(&mut runtime, "n", MemoryScope::Global, "gk", "t", "gv");
        upsert_entry(&mut runtime, "n", MemoryScope::Session, "sk", "t", "sv");
        runtime.clear_session_scope();
        assert!(runtime.session_store.is_empty());
        assert!(!runtime.project_store.entries.is_empty());
        assert!(!runtime.global_store.entries.is_empty());
    }

    // -----------------------------------------------------------------------
    // Group G: TTL expiration
    // -----------------------------------------------------------------------

    fn old_ts() -> String {
        // 30 days ago
        let past = OffsetDateTime::now_utc() - time::Duration::days(30);
        past.format(&Rfc3339).expect("format failed")
    }

    #[test]
    fn test_expire_removes_duration_expired_entries() {
        let (mut runtime, _dir) = test_runtime();

        // Insert an entry with Duration TTL already expired (updated_at 30 days ago)
        let expired_entry = MemFileEntry {
            topic: "t".to_owned(),
            scope: MemoryScope::Session,
            ttl: MemoryTtl::Duration("P7D".to_owned()),
            value: "expired".to_owned(),
            created_at: old_ts(),
            updated_at: old_ts(),
        };
        runtime
            .session_store
            .insert("expired_key".to_owned(), expired_entry);

        let removed = runtime.expire();
        assert_eq!(removed, 1);
        assert!(!runtime.session_store.contains_key("expired_key"));
    }

    #[test]
    fn test_expire_leaves_permanent_entries() {
        let (mut runtime, _dir) = test_runtime();

        let perm_entry = MemFileEntry {
            topic: "t".to_owned(),
            scope: MemoryScope::Session,
            ttl: MemoryTtl::Permanent,
            value: "keep".to_owned(),
            created_at: old_ts(),
            updated_at: old_ts(),
        };
        runtime
            .session_store
            .insert("perm_key".to_owned(), perm_entry);

        let removed = runtime.expire();
        assert_eq!(removed, 0);
        assert!(runtime.session_store.contains_key("perm_key"));
    }

    #[test]
    fn test_expire_leaves_session_ttl_entries() {
        let (mut runtime, _dir) = test_runtime();

        let sess_entry = MemFileEntry {
            topic: "t".to_owned(),
            scope: MemoryScope::Session,
            ttl: MemoryTtl::Session,
            value: "keep_session".to_owned(),
            created_at: old_ts(),
            updated_at: old_ts(),
        };
        runtime
            .session_store
            .insert("sess_key".to_owned(), sess_entry);

        let removed = runtime.expire();
        assert_eq!(removed, 0);
        assert!(runtime.session_store.contains_key("sess_key"));
    }

    // -----------------------------------------------------------------------
    // Group H: ISO 8601 parser
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_duration_p7d_returns_seconds() {
        let secs = parse_iso8601_duration("P7D").unwrap();
        assert_eq!(secs, 7 * SECS_PER_DAY);
    }

    #[test]
    fn test_parse_duration_pt1h_returns_seconds() {
        let secs = parse_iso8601_duration("PT1H").unwrap();
        assert_eq!(secs, SECS_PER_HOUR);
    }

    #[test]
    fn test_parse_duration_pt30m_returns_seconds() {
        let secs = parse_iso8601_duration("PT30M").unwrap();
        assert_eq!(secs, 30 * SECS_PER_MINUTE);
    }

    #[test]
    fn test_parse_duration_p30dt12h_returns_seconds() {
        let secs = parse_iso8601_duration("P30DT12H").unwrap();
        assert_eq!(secs, 30 * SECS_PER_DAY + 12 * SECS_PER_HOUR);
    }

    #[test]
    fn test_parse_duration_p1m_returns_error() {
        let result = parse_iso8601_duration("P1M");
        assert!(result.is_err(), "P1M (months) should be rejected");
    }

    #[test]
    fn test_parse_duration_empty_returns_error() {
        let result = parse_iso8601_duration("P");
        assert!(result.is_err(), "Empty duration 'P' should be rejected");
        let result2 = parse_iso8601_duration("PT");
        assert!(result2.is_err(), "Empty duration 'PT' should be rejected");
    }

    // -----------------------------------------------------------------------
    // Group I: GC
    // -----------------------------------------------------------------------

    #[test]
    fn test_gc_returns_correct_report() {
        let (mut runtime, _dir) = test_runtime();

        // Insert one expired entry
        let expired = MemFileEntry {
            topic: "t".to_owned(),
            scope: MemoryScope::Session,
            ttl: MemoryTtl::Duration("P1D".to_owned()),
            value: "old".to_owned(),
            created_at: old_ts(),
            updated_at: old_ts(),
        };
        runtime.session_store.insert("e".to_owned(), expired);

        let report = runtime.gc();
        assert_eq!(report.expired_removed, 1);
        assert_eq!(report.orphan_removed, 0);
    }

    // -----------------------------------------------------------------------
    // Group J: File I/O
    // -----------------------------------------------------------------------

    #[test]
    fn test_flush_and_reload_project_roundtrip() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("proj.agm.mem");
        let global_path = dir.path().join("global.mem");

        let mut runtime =
            MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").unwrap();
        upsert_entry(
            &mut runtime,
            "n",
            MemoryScope::Project,
            "k1",
            "topic",
            "hello",
        );
        runtime.flush().unwrap();

        let runtime2 = MemoryRuntime::new(project_path, global_path, "pkg").unwrap();
        assert_eq!(
            runtime2
                .project_store
                .entries
                .get("k1")
                .map(|e| e.value.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn test_flush_and_reload_global_roundtrip() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("proj.agm.mem");
        let global_path = dir.path().join("global.mem");

        let mut runtime =
            MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").unwrap();
        upsert_entry(
            &mut runtime,
            "n",
            MemoryScope::Global,
            "gk",
            "topic",
            "world",
        );
        runtime.flush().unwrap();

        let runtime2 = MemoryRuntime::new(project_path, global_path, "pkg").unwrap();
        assert_eq!(
            runtime2
                .global_store
                .entries
                .get("gk")
                .map(|e| e.value.as_str()),
            Some("world")
        );
    }

    #[test]
    fn test_flush_creates_global_directory() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("proj.agm.mem");
        // Global path in a subdirectory that doesn't exist yet
        let global_dir = dir.path().join("subdir").join(".agm");
        let global_path = global_dir.join("global.mem");

        let mut runtime = MemoryRuntime::new(project_path, global_path.clone(), "pkg").unwrap();
        runtime.flush().unwrap();

        assert!(global_path.exists());
    }

    // -----------------------------------------------------------------------
    // Group K: execute_action
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_action_defaults_scope_to_session() {
        let (mut runtime, _dir) = test_runtime();

        let entry = MemoryEntry {
            key: "k".to_owned(),
            topic: "t".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("v".to_owned()),
            scope: None, // no scope — should default to Session
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        };
        runtime.execute_action("node1", &entry).unwrap();

        // Must be in session_store, not node_store
        assert!(runtime.session_store.contains_key("k"));
        assert!(!runtime.node_store.contains_key("node1"));
    }

    #[test]
    fn test_execute_action_defaults_ttl_to_session() {
        let (mut runtime, _dir) = test_runtime();

        let entry = MemoryEntry {
            key: "k".to_owned(),
            topic: "t".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("v".to_owned()),
            scope: Some(MemoryScope::Session),
            ttl: None, // no TTL — should default to Session
            query: None,
            max_results: None,
        };
        runtime.execute_action("node1", &entry).unwrap();

        let stored = runtime.session_store.get("k").unwrap();
        assert_eq!(stored.ttl, MemoryTtl::Session);
    }

    #[test]
    fn test_execute_action_search_returns_unsupported() {
        let (mut runtime, _dir) = test_runtime();

        let entry = MemoryEntry {
            key: "k".to_owned(),
            topic: "t".to_owned(),
            action: MemoryAction::Search,
            value: None,
            scope: None,
            ttl: None,
            query: Some("something".to_owned()),
            max_results: Some(5),
        };
        let result = runtime.execute_action("node1", &entry).unwrap();
        assert_eq!(result, MemoryResult::SearchUnsupported);
    }
}
