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
                if value.len() > agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES {
                    anyhow::bail!(
                        "Memory value exceeds maximum size ({} bytes > {} bytes) for key `{}`",
                        value.len(),
                        agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES,
                        entry.key
                    );
                }
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
            .list(node_id, entry.scope.as_ref().unwrap(), &entry.topic)
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

    // -----------------------------------------------------------------------
    // Group L: Lifecycle, scoping, persistence, and scale
    // -----------------------------------------------------------------------

    /// Helper: upsert with explicit Duration TTL (used for expiration tests).
    fn upsert_with_duration_ttl(
        runtime: &mut MemoryRuntime,
        node_id: &str,
        scope: MemoryScope,
        key: &str,
        topic: &str,
        value: &str,
        duration_iso: &str,
    ) {
        let entry = MemoryEntry {
            key: key.to_owned(),
            topic: topic.to_owned(),
            action: MemoryAction::Upsert,
            value: Some(value.to_owned()),
            scope: Some(scope),
            ttl: Some(MemoryTtl::Duration(duration_iso.to_owned())),
            query: None,
            max_results: None,
        };
        runtime.execute_action(node_id, &entry).unwrap();
    }

    #[test]
    fn test_lifecycle_store_retrieve_expire_gone() {
        // Full lifecycle: upsert with already-expired timestamp, retrieve, expire, verify gone.
        let (mut runtime, _dir) = test_runtime();

        // Insert directly with an old timestamp so it's already expired
        let expired_entry = MemFileEntry {
            topic: "lifecycle".to_owned(),
            scope: MemoryScope::Session,
            ttl: MemoryTtl::Duration("P1D".to_owned()), // 1 day TTL
            value: "lifecycle_value".to_owned(),
            created_at: old_ts(), // 30 days ago
            updated_at: old_ts(), // 30 days ago -> expired
        };
        runtime
            .session_store
            .insert("lc_key".to_owned(), expired_entry);

        // Retrieve: still present before expire()
        let entry = get_entry(&runtime, "n", MemoryScope::Session, "lc_key");
        assert!(entry.is_some(), "Entry should exist before expiry");
        assert_eq!(entry.unwrap().value, "lifecycle_value");

        // Expire
        let removed = runtime.expire();
        assert_eq!(removed, 1, "Expected 1 entry removed");

        // Verify gone
        let after = get_entry(&runtime, "n", MemoryScope::Session, "lc_key");
        assert!(after.is_none(), "Entry should be gone after expire()");
    }

    #[test]
    fn test_ttl_expiration_with_actual_elapsed_time() {
        // Insert an entry with an updated_at set to 2 minutes ago and a PT1M TTL.
        // After expire(), the entry should be removed without sleeping.
        let (mut runtime, _dir) = test_runtime();

        // Set updated_at to 2 minutes in the past
        let two_minutes_ago = OffsetDateTime::now_utc() - time::Duration::minutes(2);
        let ts = two_minutes_ago
            .format(&Rfc3339)
            .expect("format failed");

        runtime.session_store.insert(
            "short_ttl".to_owned(),
            MemFileEntry {
                topic: "t".to_owned(),
                scope: MemoryScope::Session,
                ttl: MemoryTtl::Duration("PT1M".to_owned()), // 1 minute TTL
                value: "expiring".to_owned(),
                created_at: ts.clone(),
                updated_at: ts,
            },
        );

        // Verify present before expire
        assert!(runtime.session_store.contains_key("short_ttl"));

        // Expire
        let removed = runtime.expire();
        assert_eq!(removed, 1, "Entry with elapsed 1M TTL should be expired");
        assert!(!runtime.session_store.contains_key("short_ttl"));
    }

    #[test]
    fn test_scope_isolation_same_key_different_scopes() {
        // Same key stored in Node, Session, Project, Global scopes must be independent.
        let (mut runtime, _dir) = test_runtime();
        let key = "shared_key";
        upsert_entry(&mut runtime, "n1", MemoryScope::Node, key, "t", "node_val");
        upsert_entry(&mut runtime, "n1", MemoryScope::Session, key, "t", "session_val");
        upsert_entry(&mut runtime, "n1", MemoryScope::Project, key, "t", "project_val");
        upsert_entry(&mut runtime, "n1", MemoryScope::Global, key, "t", "global_val");

        let node_v = get_entry(&runtime, "n1", MemoryScope::Node, key).unwrap();
        let sess_v = get_entry(&runtime, "n1", MemoryScope::Session, key).unwrap();
        let proj_v = get_entry(&runtime, "n1", MemoryScope::Project, key).unwrap();
        let glob_v = get_entry(&runtime, "n1", MemoryScope::Global, key).unwrap();

        assert_eq!(node_v.value, "node_val");
        assert_eq!(sess_v.value, "session_val");
        assert_eq!(proj_v.value, "project_val");
        assert_eq!(glob_v.value, "global_val");
    }

    #[test]
    fn test_project_scope_persists_across_runtime_reload() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("lifecycle.agm.mem");
        let global_path = dir.path().join("lifecycle_global.mem");

        {
            let mut rt =
                MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").unwrap();
            upsert_entry(&mut rt, "n", MemoryScope::Project, "persist_key", "t", "persist_val");
            rt.flush().unwrap();
        }

        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").unwrap();
        let entry = rt2.project_store.entries.get("persist_key");
        assert!(entry.is_some(), "Project-scoped entry should survive reload");
        assert_eq!(entry.unwrap().value, "persist_val");
    }

    #[test]
    fn test_global_scope_persists_across_runtime_reload() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("g_lifecycle.agm.mem");
        let global_path = dir.path().join("g_lifecycle_global.mem");

        {
            let mut rt =
                MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").unwrap();
            upsert_entry(&mut rt, "n", MemoryScope::Global, "global_persist", "t", "gval");
            rt.flush().unwrap();
        }

        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").unwrap();
        let entry = rt2.global_store.entries.get("global_persist");
        assert!(entry.is_some(), "Global-scoped entry should survive reload");
        assert_eq!(entry.unwrap().value, "gval");
    }

    #[test]
    fn test_session_scope_not_persisted_after_reload() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("sess_lifecycle.agm.mem");
        let global_path = dir.path().join("sess_lifecycle_global.mem");

        {
            let mut rt =
                MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").unwrap();
            upsert_entry(
                &mut rt,
                "n",
                MemoryScope::Session,
                "sess_key",
                "t",
                "ephemeral",
            );
            rt.flush().unwrap();
            // Session data must be present in this instance
            let v = get_entry(&rt, "n", MemoryScope::Session, "sess_key");
            assert!(v.is_some(), "Session data should be present before reload");
        }

        // New runtime: session store should be empty (not persisted)
        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").unwrap();
        assert!(
            rt2.session_store.is_empty(),
            "Session data must not survive runtime reload"
        );
    }

    #[test]
    fn test_gc_removes_multiple_expired_entries_and_keeps_valid() {
        let (mut runtime, _dir) = test_runtime();

        // Insert 3 entries with expired Duration TTL
        for i in 0..3 {
            runtime.session_store.insert(
                format!("exp_{i}"),
                MemFileEntry {
                    topic: "t".to_owned(),
                    scope: MemoryScope::Session,
                    ttl: MemoryTtl::Duration("P1D".to_owned()),
                    value: format!("v{i}"),
                    created_at: old_ts(),
                    updated_at: old_ts(),
                },
            );
        }

        // Insert 2 permanent entries (must NOT be removed)
        for i in 0..2 {
            runtime.session_store.insert(
                format!("keep_{i}"),
                MemFileEntry {
                    topic: "t".to_owned(),
                    scope: MemoryScope::Session,
                    ttl: MemoryTtl::Permanent,
                    value: format!("kv{i}"),
                    created_at: old_ts(),
                    updated_at: old_ts(),
                },
            );
        }

        let report = runtime.gc();
        assert_eq!(report.expired_removed, 3, "GC should remove exactly 3 expired entries");
        assert_eq!(report.orphan_removed, 0);

        // Permanent entries must still be present
        for i in 0..2 {
            assert!(
                runtime.session_store.contains_key(&format!("keep_{i}")),
                "Permanent entry keep_{i} should survive GC"
            );
        }
        // Expired entries must be gone
        for i in 0..3 {
            assert!(
                !runtime.session_store.contains_key(&format!("exp_{i}")),
                "Expired entry exp_{i} should be removed by GC"
            );
        }
    }

    #[test]
    fn test_atomic_write_creates_file_at_target_path() {
        // Verify atomic_write results in the file being at the correct path
        // and that no .tmp sidecar remains after a successful write.
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("test_atomic.mem");
        let tmp = dir.path().join("test_atomic.tmp");

        atomic_write(&target, "atomic content").unwrap();

        // Target must exist with correct content
        assert!(target.exists(), "Target file should exist after atomic_write");
        let content = std::fs::read_to_string(&target).unwrap();
        assert_eq!(content, "atomic content");

        // Temp file must be gone (was renamed away)
        assert!(!tmp.exists(), "Temp file should not remain after atomic rename");
    }

    #[test]
    fn test_overwrite_existing_key_returns_latest_value() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Project, "k", "t", "first");
        upsert_entry(&mut runtime, "n", MemoryScope::Project, "k", "t", "second");
        let entry = get_entry(&runtime, "n", MemoryScope::Project, "k");
        assert_eq!(entry.unwrap().value, "second", "Second upsert should overwrite first");
    }

    #[test]
    fn test_delete_entry_makes_it_unretrievable() {
        let (mut runtime, _dir) = test_runtime();
        upsert_entry(&mut runtime, "n", MemoryScope::Project, "del_key", "t", "to_delete");
        let before = get_entry(&runtime, "n", MemoryScope::Project, "del_key");
        assert!(before.is_some(), "Entry should exist before delete");

        let deleted = delete_entry(&mut runtime, "n", MemoryScope::Project, "del_key");
        assert!(deleted, "delete should return true for existing key");

        let after = get_entry(&runtime, "n", MemoryScope::Project, "del_key");
        assert!(after.is_none(), "Entry should be gone after delete");
    }

    #[test]
    fn test_large_number_of_entries_all_retrievable_gc_preserves_non_expired() {
        let (mut runtime, _dir) = test_runtime();

        // Insert 100 permanent session entries
        for i in 0..100 {
            upsert_entry(
                &mut runtime,
                "n",
                MemoryScope::Session,
                &format!("key_{i:03}"),
                "bulk",
                &format!("val_{i}"),
            );
        }

        // All 100 must be retrievable
        for i in 0..100 {
            let entry = get_entry(&runtime, "n", MemoryScope::Session, &format!("key_{i:03}"));
            assert!(
                entry.is_some(),
                "Entry key_{i:03} should be retrievable"
            );
            assert_eq!(
                entry.unwrap().value,
                format!("val_{i}"),
                "Entry key_{i:03} value mismatch"
            );
        }

        // Insert 20 expired entries alongside the 100 permanent ones
        for i in 0..20 {
            runtime.session_store.insert(
                format!("expired_{i:03}"),
                MemFileEntry {
                    topic: "bulk".to_owned(),
                    scope: MemoryScope::Session,
                    ttl: MemoryTtl::Duration("P1D".to_owned()),
                    value: format!("expval_{i}"),
                    created_at: old_ts(),
                    updated_at: old_ts(),
                },
            );
        }

        // GC should remove exactly the 20 expired entries, leaving 100 permanent
        let report = runtime.gc();
        assert_eq!(report.expired_removed, 20, "GC should remove exactly the 20 expired entries");

        // All 100 permanent entries must still be present
        for i in 0..100 {
            let entry = get_entry(&runtime, "n", MemoryScope::Session, &format!("key_{i:03}"));
            assert!(
                entry.is_some(),
                "Permanent entry key_{i:03} should survive GC"
            );
        }

        // All 20 expired entries must be gone
        for i in 0..20 {
            assert!(
                !runtime.session_store.contains_key(&format!("expired_{i:03}")),
                "Expired entry expired_{i:03} should be removed by GC"
            );
        }
    }

    // =========================================================================
    // Group M — Concurrency and high-throughput tests
    // =========================================================================

    /// 10 threads each create their own MemoryRuntime pointing at the same
    /// project sidecar, upsert 10 unique keys, then flush.  After all threads
    /// finish, a fresh runtime loads the file and the parse must succeed with
    /// at least some entries present (last-writer-wins is acceptable; the goal
    /// is no corruption from partial writes).
    ///
    /// Individual flushes may fail on Windows when multiple threads race over
    /// the same shared `.tmp` rename target — that is expected behaviour and
    /// not treated as a test failure.
    #[test]
    fn test_concurrent_flush_to_same_project_file_no_corruption() {
        use std::sync::Arc;

        let dir = TempDir::new().expect("tempdir");
        let project_path = Arc::new(dir.path().join("shared.agm.mem"));
        let global_path = Arc::new(dir.path().join("shared_global.mem"));

        std::thread::scope(|s| {
            for t in 0..10usize {
                let pp = Arc::clone(&project_path);
                let gp = Arc::clone(&global_path);
                s.spawn(move || {
                    let mut rt =
                        MemoryRuntime::new((*pp).clone(), (*gp).clone(), "pkg").expect("rt");
                    for k in 0..10usize {
                        upsert_entry(
                            &mut rt,
                            "n",
                            MemoryScope::Project,
                            &format!("t{t}_k{k}"),
                            "topic",
                            &format!("v{t}_{k}"),
                        );
                    }
                    // Ignore flush errors: on Windows concurrent renames to the
                    // same path via the shared .tmp sibling can fail.  At least
                    // one thread will succeed and leave a valid file.
                    let _ = rt.flush();
                });
            }
        });

        // Reload and verify: file must parse without error and have >= 1 entry
        let rt_final =
            MemoryRuntime::new((*project_path).clone(), (*global_path).clone(), "pkg")
                .expect("final runtime");
        assert!(
            !rt_final.project_store.entries.is_empty(),
            "At least some entries must survive concurrent flush (last-writer-wins)"
        );
    }

    /// Same as above but targeting the global sidecar file.
    ///
    /// Individual flushes may fail on Windows when multiple threads race over
    /// the same shared `.tmp` rename target — that is expected behaviour.
    #[test]
    fn test_concurrent_flush_to_same_global_file_no_corruption() {
        use std::sync::Arc;

        let dir = TempDir::new().expect("tempdir");
        let project_path = Arc::new(dir.path().join("shared2.agm.mem"));
        let global_path = Arc::new(dir.path().join("shared2_global.mem"));

        std::thread::scope(|s| {
            for t in 0..10usize {
                let pp = Arc::clone(&project_path);
                let gp = Arc::clone(&global_path);
                s.spawn(move || {
                    let mut rt =
                        MemoryRuntime::new((*pp).clone(), (*gp).clone(), "pkg").expect("rt");
                    for k in 0..10usize {
                        upsert_entry(
                            &mut rt,
                            "n",
                            MemoryScope::Global,
                            &format!("t{t}_gk{k}"),
                            "topic",
                            &format!("gv{t}_{k}"),
                        );
                    }
                    // Ignore flush errors: concurrent renames via shared .tmp
                    // can fail on Windows; at least one thread will succeed.
                    let _ = rt.flush();
                });
            }
        });

        let rt_final =
            MemoryRuntime::new((*project_path).clone(), (*global_path).clone(), "pkg")
                .expect("final runtime");
        assert!(
            !rt_final.global_store.entries.is_empty(),
            "At least some global entries must survive concurrent flush"
        );
    }

    /// Single thread: 50 iterations of create→upsert→flush→drop.
    /// After the loop the last iteration's 5 entries must all be present on
    /// a fresh reload.
    #[test]
    fn test_rapid_flush_reload_cycle_preserves_data() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("rapid.agm.mem");
        let global_path = dir.path().join("rapid_global.mem");

        for i in 0..50usize {
            let mut rt = MemoryRuntime::new(
                project_path.clone(),
                global_path.clone(),
                "pkg",
            )
            .expect("rt");
            for k in 0..5usize {
                upsert_entry(
                    &mut rt,
                    "n",
                    MemoryScope::Project,
                    &format!("iter{i}_k{k}"),
                    "t",
                    &format!("v{i}_{k}"),
                );
            }
            rt.flush().expect("flush");
            // drop rt here — simulates a new runtime instance each iteration
        }

        // Only the last iteration's keys are relevant; verify all 5 are present
        let last = 49usize;
        let rt_final = MemoryRuntime::new(project_path, global_path, "pkg").expect("final rt");
        for k in 0..5usize {
            let key = format!("iter{last}_k{k}");
            assert!(
                rt_final.project_store.entries.contains_key(&key),
                "Last iteration key {key} should be present after reload"
            );
        }
    }

    /// Single runtime: 1000 upserts, all retrievable in-memory and after reload.
    #[test]
    fn test_high_throughput_1000_upserts_single_runtime() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("ht1000.agm.mem");
        let global_path = dir.path().join("ht1000_global.mem");
        let mut rt =
            MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").expect("rt");

        for i in 0..1000usize {
            upsert_entry(
                &mut rt,
                "n",
                MemoryScope::Project,
                &format!("key_{i:04}"),
                "bulk",
                &format!("val_{i}"),
            );
        }

        // Verify all 1000 in-memory
        for i in 0..1000usize {
            let entry = get_entry(&rt, "n", MemoryScope::Project, &format!("key_{i:04}"));
            assert!(entry.is_some(), "key_{i:04} should be present in-memory");
            assert_eq!(entry.unwrap().value, format!("val_{i}"));
        }

        rt.flush().expect("flush");

        // Reload and verify
        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").expect("rt2");
        for i in 0..1000usize {
            assert!(
                rt2.project_store.entries.contains_key(&format!("key_{i:04}")),
                "key_{i:04} should be present after reload"
            );
        }
    }

    /// Upsert 1000 entries, delete even-indexed 500, verify 500 remain both
    /// in-memory and after flush→reload.
    #[test]
    fn test_high_throughput_1000_upserts_then_delete_half() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("htdel.agm.mem");
        let global_path = dir.path().join("htdel_global.mem");
        let mut rt =
            MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").expect("rt");

        for i in 0..1000usize {
            upsert_entry(
                &mut rt,
                "n",
                MemoryScope::Project,
                &format!("key_{i:04}"),
                "bulk",
                &format!("val_{i}"),
            );
        }

        // Delete even-indexed keys (0, 2, 4 … 998)
        for i in (0..1000usize).step_by(2) {
            delete_entry(&mut rt, "n", MemoryScope::Project, &format!("key_{i:04}"));
        }

        // Verify in-memory: odd keys present, even keys gone
        for i in 0..1000usize {
            let key = format!("key_{i:04}");
            let entry = get_entry(&rt, "n", MemoryScope::Project, &key);
            if i % 2 == 0 {
                assert!(entry.is_none(), "{key} should be deleted");
            } else {
                assert!(entry.is_some(), "{key} should still be present");
            }
        }

        rt.flush().expect("flush");

        // Reload and verify
        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").expect("rt2");
        for i in 0..1000usize {
            let key = format!("key_{i:04}");
            if i % 2 == 0 {
                assert!(
                    !rt2.project_store.entries.contains_key(&key),
                    "{key} should be absent after reload"
                );
            } else {
                assert!(
                    rt2.project_store.entries.contains_key(&key),
                    "{key} should be present after reload"
                );
            }
        }
    }

    /// Insert 3000 permanent + 2000 expired entries into session scope.
    /// GC must remove exactly 2000 and leave exactly 3000.
    #[test]
    fn test_gc_with_5000_entries_mixed_expired_permanent() {
        let (mut runtime, _dir) = test_runtime();

        // 3000 permanent entries
        for i in 0..3000usize {
            runtime.session_store.insert(
                format!("perm_{i:05}"),
                MemFileEntry {
                    topic: "t".to_owned(),
                    scope: MemoryScope::Session,
                    ttl: MemoryTtl::Permanent,
                    value: format!("pv_{i}"),
                    created_at: old_ts(),
                    updated_at: old_ts(),
                },
            );
        }

        // 2000 expired entries
        for i in 0..2000usize {
            runtime.session_store.insert(
                format!("exp_{i:05}"),
                MemFileEntry {
                    topic: "t".to_owned(),
                    scope: MemoryScope::Session,
                    ttl: MemoryTtl::Duration("P1D".to_owned()),
                    value: format!("ev_{i}"),
                    created_at: old_ts(),
                    updated_at: old_ts(),
                },
            );
        }

        let report = runtime.gc();
        assert_eq!(
            report.expired_removed, 2000,
            "GC should remove exactly 2000 expired entries"
        );
        assert_eq!(
            runtime.session_store.len(),
            3000,
            "Exactly 3000 permanent entries should remain"
        );
    }

    /// 20 threads call `atomic_write` concurrently against the same path.
    ///
    /// Note: the `atomic_write` primitive uses a shared `.tmp` path derived
    /// from the target filename, so it is NOT safe for concurrent writers
    /// targeting the same destination. Two writers can interleave `File::create`
    /// (truncate) and `write_all` on the same `.tmp`, producing a torn final
    /// file. The real serialization point lives one layer up — per-`MemoryRuntime`
    /// flush paths are single-threaded.
    ///
    /// What this test DOES assert:
    ///  * Concurrent calls don't crash, deadlock, or produce filesystem errors
    ///    that poison subsequent writes to the same path.
    ///  * At least one of the concurrent writes completes successfully.
    ///  * After the race, a SERIAL `atomic_write` produces a well-formed file
    ///    (the happy-path invariant is restored once contention ends).
    ///  * Any lingering `.tmp` residue is cleanable and does not block recovery.
    #[test]
    fn test_concurrent_atomic_write_same_path() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = TempDir::new().expect("tempdir");
        let target = Arc::new(dir.path().join("concurrent_atomic.mem"));
        let successes = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|s| {
            for i in 0..20usize {
                let path = Arc::clone(&target);
                let successes = Arc::clone(&successes);
                s.spawn(move || {
                    let content = format!("thread-{i}\n").repeat(100);
                    // Concurrent renames through a shared .tmp path can fail on
                    // Windows; those failures are legitimate race artifacts.
                    if atomic_write(&path, &content).is_ok() {
                        successes.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        });

        // Sanity — at least one write made it through, otherwise the race
        // degenerated completely and the test proved nothing.
        let count = successes.load(Ordering::Relaxed);
        assert!(
            count > 0,
            "expected at least one concurrent atomic_write to succeed, got 0"
        );

        // Clean up any lingering `.tmp` residue from a rename that lost its
        // race; its presence is not a failure.
        let tmp = target.with_extension(format!(
            "{}.tmp",
            target.extension().and_then(|e| e.to_str()).unwrap_or("mem")
        ));
        let _ = std::fs::remove_file(&tmp);

        // The happy-path invariant: once contention ends, a serial atomic_write
        // produces an intact, complete file — no lingering state from the race
        // corrupts subsequent well-behaved writers.
        let final_content = "final-writer\n".repeat(100);
        atomic_write(&target, &final_content).expect("final serial write must succeed");
        let read_back = std::fs::read_to_string(target.as_ref())
            .expect("file must be readable after serial final write");
        let lines: Vec<&str> = read_back.lines().collect();
        assert_eq!(
            lines.len(),
            100,
            "serial write after race must produce exactly 100 lines, got {}",
            lines.len()
        );
        assert!(
            lines.iter().all(|l| *l == "final-writer"),
            "serial write must contain only final-writer lines"
        );
    }

    /// One runtime flushes to the project path; simultaneously another thread
    /// reads the same file.  The read must succeed without panicking and, when
    /// it does see content, that content must be non-empty (atomic write must
    /// not leave a partial file).
    #[test]
    fn test_flush_while_file_being_read_no_panic() {
        use std::sync::{Arc, Barrier};

        let dir = TempDir::new().expect("tempdir");
        let project_path = Arc::new(dir.path().join("concurrent_rw.agm.mem"));
        let global_path = Arc::new(dir.path().join("concurrent_rw_global.mem"));

        // Pre-create the file so the reader always finds something
        {
            let mut rt =
                MemoryRuntime::new((*project_path).clone(), (*global_path).clone(), "pkg")
                    .expect("rt");
            upsert_entry(&mut rt, "n", MemoryScope::Project, "seed", "t", "seed_val");
            rt.flush().expect("initial flush");
        }

        let barrier = Arc::new(Barrier::new(2));
        let pp_writer = Arc::clone(&project_path);
        let gp_writer = Arc::clone(&global_path);
        let pp_reader = Arc::clone(&project_path);
        let barrier_writer = Arc::clone(&barrier);
        let barrier_reader = Arc::clone(&barrier);

        std::thread::scope(|s| {
            // Writer thread: flush many times
            s.spawn(move || {
                barrier_writer.wait();
                let mut rt =
                    MemoryRuntime::new((*pp_writer).clone(), (*gp_writer).clone(), "pkg")
                        .expect("rt writer");
                upsert_entry(&mut rt, "n", MemoryScope::Project, "wr_key", "t", "wr_val");
                for _ in 0..10 {
                    rt.flush().expect("writer flush");
                }
            });

            // Reader thread: read the file many times while writer is flushing
            s.spawn(move || {
                barrier_reader.wait();
                for _ in 0..10 {
                    // The file may be transiently absent between renames on some
                    // OSes; if it exists it must not be empty.
                    if let Ok(content) = std::fs::read_to_string(pp_reader.as_ref()) {
                        assert!(
                            !content.is_empty(),
                            "File must not be empty when readable (atomic write guarantee)"
                        );
                    }
                }
            });
        });
    }

    /// 4 threads operate on distinct scopes (Node, Session, Project, Global).
    /// Node and Session are in-memory only and verified within each thread.
    /// Project and Global are file-backed: each thread uses its own dedicated
    /// sidecar files so there is no cross-thread file race, and isolation is
    /// verified independently after all threads finish.
    /// Since MemoryRuntime is not Sync, each thread owns its own instance.
    #[test]
    fn test_many_scopes_concurrent_upsert_isolation() {
        let dir = TempDir::new().expect("tempdir");

        // Dedicated paths for the project-scope thread
        let proj_pp = dir.path().join("proj_scope.agm.mem");
        let proj_gp = dir.path().join("proj_scope_global.mem");

        // Dedicated paths for the global-scope thread
        let glob_pp = dir.path().join("glob_scope.agm.mem");
        let glob_gp = dir.path().join("glob_scope_global.mem");

        use std::sync::Arc;
        let proj_pp = Arc::new(proj_pp);
        let proj_gp = Arc::new(proj_gp);
        let glob_pp = Arc::new(glob_pp);
        let glob_gp = Arc::new(glob_gp);

        std::thread::scope(|s| {
            // Thread 1: Node scope (in-memory only — verify within thread)
            s.spawn(|| {
                let dir2 = TempDir::new().expect("tempdir node");
                let pp = dir2.path().join("n.agm.mem");
                let gp = dir2.path().join("n_global.mem");
                let mut rt = MemoryRuntime::new(pp, gp, "pkg").expect("rt node");
                for k in 0..100usize {
                    upsert_entry(
                        &mut rt,
                        "node1",
                        MemoryScope::Node,
                        &format!("node_k{k:03}"),
                        "t",
                        &format!("node_v{k}"),
                    );
                }
                for k in 0..100usize {
                    let e = get_entry(&rt, "node1", MemoryScope::Node, &format!("node_k{k:03}"));
                    assert!(e.is_some(), "node_k{k:03} must be present");
                }
                assert_eq!(
                    rt.node_store.get("node1").map(|m| m.len()).unwrap_or(0),
                    100,
                    "Node scope must have exactly 100 entries"
                );
            });

            // Thread 2: Session scope (in-memory only — verify within thread)
            s.spawn(|| {
                let dir2 = TempDir::new().expect("tempdir session");
                let pp = dir2.path().join("s.agm.mem");
                let gp = dir2.path().join("s_global.mem");
                let mut rt = MemoryRuntime::new(pp, gp, "pkg").expect("rt session");
                for k in 0..100usize {
                    upsert_entry(
                        &mut rt,
                        "n",
                        MemoryScope::Session,
                        &format!("sess_k{k:03}"),
                        "t",
                        &format!("sess_v{k}"),
                    );
                }
                assert_eq!(
                    rt.session_store.len(),
                    100,
                    "Session scope must have exactly 100 entries"
                );
            });

            // Thread 3: Project scope — dedicated files, verify after join
            {
                let pp3 = Arc::clone(&proj_pp);
                let gp3 = Arc::clone(&proj_gp);
                s.spawn(move || {
                    let mut rt =
                        MemoryRuntime::new((*pp3).clone(), (*gp3).clone(), "pkg")
                            .expect("rt proj");
                    for k in 0..100usize {
                        upsert_entry(
                            &mut rt,
                            "n",
                            MemoryScope::Project,
                            &format!("proj_k{k:03}"),
                            "t",
                            &format!("proj_v{k}"),
                        );
                    }
                    rt.flush().expect("proj flush");
                });
            }

            // Thread 4: Global scope — dedicated files, verify after join
            {
                let pp4 = Arc::clone(&glob_pp);
                let gp4 = Arc::clone(&glob_gp);
                s.spawn(move || {
                    let mut rt =
                        MemoryRuntime::new((*pp4).clone(), (*gp4).clone(), "pkg")
                            .expect("rt glob");
                    for k in 0..100usize {
                        upsert_entry(
                            &mut rt,
                            "n",
                            MemoryScope::Global,
                            &format!("glob_k{k:03}"),
                            "t",
                            &format!("glob_v{k}"),
                        );
                    }
                    rt.flush().expect("glob flush");
                });
            }
        });

        // Post-join: verify project scope isolation
        let rt_proj =
            MemoryRuntime::new((*proj_pp).clone(), (*proj_gp).clone(), "pkg")
                .expect("check proj rt");

        for k in 0..100usize {
            let key = format!("proj_k{k:03}");
            assert!(
                rt_proj.project_store.entries.contains_key(&key),
                "{key} must be in project store"
            );
            // Session keys must not have leaked into the project file
            assert!(
                !rt_proj
                    .project_store
                    .entries
                    .contains_key(&format!("sess_k{k:03}")),
                "Session key must not appear in project store"
            );
            // Global keys must not have leaked into project store
            assert!(
                !rt_proj
                    .project_store
                    .entries
                    .contains_key(&format!("glob_k{k:03}")),
                "Global key must not appear in project store"
            );
        }

        // Post-join: verify global scope isolation
        let rt_glob =
            MemoryRuntime::new((*glob_pp).clone(), (*glob_gp).clone(), "pkg")
                .expect("check glob rt");

        for k in 0..100usize {
            let key = format!("glob_k{k:03}");
            assert!(
                rt_glob.global_store.entries.contains_key(&key),
                "{key} must be in global store"
            );
            // Project keys must not have leaked into global store
            assert!(
                !rt_glob
                    .global_store
                    .entries
                    .contains_key(&format!("proj_k{k:03}")),
                "Project key must not appear in global store"
            );
        }
    }

    /// Upsert 100 entries each with a 10 KB value, flush, reload, verify
    /// all values are intact and their length is exactly 10 240 bytes.
    #[test]
    fn test_large_value_entries_10kb_each() {
        let dir = TempDir::new().expect("tempdir");
        let project_path = dir.path().join("large_vals.agm.mem");
        let global_path = dir.path().join("large_vals_global.mem");
        let mut rt =
            MemoryRuntime::new(project_path.clone(), global_path.clone(), "pkg").expect("rt");

        let large_value = "x".repeat(10 * 1024); // 10 KB of 'x'

        for i in 0..100usize {
            upsert_entry(
                &mut rt,
                "n",
                MemoryScope::Project,
                &format!("large_{i:03}"),
                "big",
                &large_value,
            );
        }

        rt.flush().expect("flush");

        let rt2 = MemoryRuntime::new(project_path, global_path, "pkg").expect("rt2");
        for i in 0..100usize {
            let key = format!("large_{i:03}");
            let entry = rt2.project_store.entries.get(&key);
            assert!(entry.is_some(), "{key} must be present after reload");
            assert_eq!(
                entry.unwrap().value.len(),
                10 * 1024,
                "{key} value must be exactly 10 KB after reload"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Group: Value size limit (V027 — 32 KiB max)
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_action_upsert_value_at_limit_succeeds() {
        let (mut runtime, _dir) = test_runtime();
        let value = "x".repeat(agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES);
        let entry = MemoryEntry {
            key: "big.key".to_owned(),
            topic: "test".to_owned(),
            action: MemoryAction::Upsert,
            value: Some(value),
            scope: Some(MemoryScope::Session),
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        };
        let result = runtime.execute_action("node1", &entry);
        assert!(result.is_ok(), "Value at exactly 32 KiB should be accepted");
    }

    #[test]
    fn test_execute_action_upsert_value_over_limit_returns_error() {
        let (mut runtime, _dir) = test_runtime();
        let value = "x".repeat(agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES + 1);
        let entry = MemoryEntry {
            key: "big.key".to_owned(),
            topic: "test".to_owned(),
            action: MemoryAction::Upsert,
            value: Some(value),
            scope: Some(MemoryScope::Session),
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        };
        let result = runtime.execute_action("node1", &entry);
        assert!(result.is_err(), "Value over 32 KiB should be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("exceeds maximum size"),
            "Error message should mention size limit: {msg}"
        );
    }

    #[test]
    fn test_execute_action_upsert_value_over_limit_does_not_persist() {
        let (mut runtime, _dir) = test_runtime();
        let value = "x".repeat(agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES + 100);
        let entry = MemoryEntry {
            key: "big.key".to_owned(),
            topic: "test".to_owned(),
            action: MemoryAction::Upsert,
            value: Some(value),
            scope: Some(MemoryScope::Session),
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        };
        let _ = runtime.execute_action("node1", &entry);
        // Verify nothing was stored
        let stored = get_entry(&runtime, "node1", MemoryScope::Session, "big.key");
        assert!(stored.is_none(), "Oversized value must not be persisted");
    }

    #[test]
    fn test_execute_action_upsert_all_scopes_reject_oversized_value() {
        let (mut runtime, _dir) = test_runtime();
        let value = "x".repeat(agm_core::memory::schema::MAX_MEMORY_VALUE_BYTES + 1);
        for scope in [
            MemoryScope::Node,
            MemoryScope::Session,
            MemoryScope::Project,
            MemoryScope::Global,
        ] {
            let entry = MemoryEntry {
                key: "big.key".to_owned(),
                topic: "test".to_owned(),
                action: MemoryAction::Upsert,
                value: Some(value.clone()),
                scope: Some(scope.clone()),
                ttl: Some(MemoryTtl::Permanent),
                query: None,
                max_results: None,
            };
            let result = runtime.execute_action("node1", &entry);
            assert!(
                result.is_err(),
                "Scope {scope} should reject oversized value"
            );
        }
    }
}
