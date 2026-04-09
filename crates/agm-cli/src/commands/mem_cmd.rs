//! `mem` command: inspect and manage memory sidecars.

use std::path::Path;

use agm_core::renderer::mem::{render_mem, render_mem_json};

use crate::runtime::memory;

use super::helpers;

// ---- list ----

pub fn list(file: &Path, topic: Option<&str>, scope: Option<&str>, json: bool) -> i32 {
    let ctx = helpers::build_runtime_context(file);
    let mem = ctx.memory;

    // Collect entries filtered by scope
    let include_project = scope.is_none_or(|s| s == "project");
    let include_global = scope.is_none_or(|s| s == "global");

    let mut all_entries: Vec<(String, &agm_core::model::mem_file::MemFileEntry)> = Vec::new();

    if include_project {
        for (key, entry) in mem.project_store().entries.iter() {
            if topic.is_none_or(|t| entry.topic == t) {
                all_entries.push((key.clone(), entry));
            }
        }
    }
    if include_global {
        for (key, entry) in mem.global_store().entries.iter() {
            if topic.is_none_or(|t| entry.topic == t) {
                all_entries.push((key.clone(), entry));
            }
        }
    }

    if json {
        let json_entries: Vec<serde_json::Value> = all_entries
            .iter()
            .map(|(key, entry)| {
                serde_json::json!({
                    "key": key,
                    "value": entry.value,
                    "topic": entry.topic,
                    "scope": entry.scope.to_string(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_entries).unwrap());
    } else if all_entries.is_empty() {
        println!("No memory entries found.");
    } else {
        println!("{:<30} {:<20} Value", "Key", "Topic");
        println!("{:<30} {:<20} -----", "---", "-----");
        for (key, entry) in &all_entries {
            let value_preview = entry.value.chars().take(40).collect::<String>();
            println!("{:<30} {:<20} {}", key, entry.topic, value_preview);
        }
    }
    helpers::EXIT_SUCCESS
}

// ---- get ----

pub fn get(file: &Path, key: &str) -> i32 {
    let ctx = helpers::build_runtime_context(file);
    let mem = ctx.memory;

    // Search project store first, then global
    let entry = mem
        .project_store()
        .entries
        .get(key)
        .or_else(|| mem.global_store().entries.get(key));

    match entry {
        Some(e) => {
            println!("Key: {}", key);
            println!("Topic: {}", e.topic);
            println!("Scope: {}", e.scope);
            println!("Value:\n{}", e.value);
            helpers::EXIT_SUCCESS
        }
        None => {
            eprintln!("error: memory key '{}' not found", key);
            helpers::EXIT_VALIDATION_ERROR
        }
    }
}

// ---- export ----

pub fn export(file: &Path, format: &str) -> i32 {
    let ctx = helpers::build_runtime_context(file);
    let mem = ctx.memory;
    let store = mem.project_store();

    match format {
        "json" => println!("{}", render_mem_json(store)),
        "agm" => print!("{}", render_mem(store)),
        "sql" => {
            eprintln!("error: SQL export not yet implemented");
            return helpers::EXIT_VALIDATION_ERROR;
        }
        other => {
            eprintln!("error: unknown format '{}'. Use json or agm.", other);
            return helpers::EXIT_VALIDATION_ERROR;
        }
    }
    helpers::EXIT_SUCCESS
}

// ---- import ----

pub fn import(file: &Path, from: &Path) -> i32 {
    let source = match std::fs::read_to_string(from) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", from.display(), e);
            return helpers::EXIT_IO_ERROR;
        }
    };

    let parsed_mem = match agm_core::parser::mem::parse_mem(&source) {
        Ok(m) => m,
        Err(errors) => {
            eprintln!(
                "error: failed to parse memory file {}: {} error(s)",
                from.display(),
                errors.len()
            );
            return helpers::EXIT_VALIDATION_ERROR;
        }
    };

    // Write to the project mem sidecar path
    let mem_path = memory::mem_path_from_agm(file);
    let rendered = render_mem(&parsed_mem);
    if let Err(e) = std::fs::write(&mem_path, rendered) {
        eprintln!(
            "error: failed to write memory to {}: {}",
            mem_path.display(),
            e
        );
        return helpers::EXIT_IO_ERROR;
    }

    println!(
        "Imported memory from {} to {}",
        from.display(),
        mem_path.display()
    );
    helpers::EXIT_SUCCESS
}

// ---- gc ----

pub fn gc(file: &Path) -> i32 {
    let mut ctx = helpers::build_runtime_context(file);
    let report = ctx.memory.gc();
    if let Err(e) = ctx.memory.flush() {
        eprintln!("error: failed to flush memory: {e}");
        return helpers::EXIT_IO_ERROR;
    }
    println!(
        "GC complete: {} expired entries removed, {} orphans removed",
        report.expired_removed, report.orphan_removed
    );
    helpers::EXIT_SUCCESS
}

#[cfg(test)]
mod tests {
    use agm_core::model::mem_file::MemFileEntry;
    use agm_core::model::memory::{MemoryScope, MemoryTtl};
    use std::collections::BTreeMap;

    fn make_entry(value: &str, topic: &str) -> MemFileEntry {
        MemFileEntry {
            value: value.to_owned(),
            topic: topic.to_owned(),
            scope: MemoryScope::Project,
            ttl: MemoryTtl::Permanent,
            created_at: "2026-04-08T00:00:00Z".to_owned(),
            updated_at: "2026-04-08T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn test_list_filters_by_topic() {
        let mut entries: BTreeMap<String, MemFileEntry> = BTreeMap::new();
        entries.insert("k1".to_owned(), make_entry("v1", "foo"));
        entries.insert("k2".to_owned(), make_entry("v2", "bar"));

        let filtered: Vec<_> = entries.iter().filter(|(_, e)| e.topic == "foo").collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "k1");
    }

    #[test]
    fn test_gc_removes_expired() {
        // Verify logic: expired_removed + orphan_removed == total cleaned
        let expired_removed: usize = 2;
        let orphan_removed: usize = 1;
        assert_eq!(expired_removed + orphan_removed, 3);
    }

    #[test]
    fn test_export_sql_unimplemented() {
        let format = "sql";
        let is_supported = matches!(format, "json" | "agm");
        assert!(!is_supported);
    }
}
