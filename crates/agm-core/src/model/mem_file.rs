//! Memory sidecar model types for `.agm.mem` files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::memory::{MemoryScope, MemoryTtl};

/// Represents a parsed `.agm.mem` sidecar file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemFile {
    pub format_version: String,
    pub package: String,
    pub updated_at: String,
    pub entries: BTreeMap<String, MemFileEntry>,
}

/// A single memory entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemFileEntry {
    pub topic: String,
    pub scope: MemoryScope,
    pub ttl: MemoryTtl,
    pub value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(ttl: MemoryTtl) -> MemFileEntry {
        MemFileEntry {
            topic: "infrastructure".to_owned(),
            scope: MemoryScope::Project,
            ttl,
            value: "some value".to_owned(),
            created_at: "2026-04-08T10:00:00Z".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
        }
    }

    // -----------------------------------------------------------------------
    // A: Serde roundtrip — minimal (no entries)
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_file_serde_roundtrip_minimal() {
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries: BTreeMap::new(),
        };
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemFile = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, back);
    }

    // -----------------------------------------------------------------------
    // B: Serde roundtrip — permanent TTL
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_file_serde_roundtrip_permanent_ttl() {
        let mut entries = BTreeMap::new();
        entries.insert("key.one".to_owned(), make_entry(MemoryTtl::Permanent));
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemFile = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, back);
    }

    // -----------------------------------------------------------------------
    // C: Serde roundtrip — session TTL
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_file_serde_roundtrip_session_ttl() {
        let mut entries = BTreeMap::new();
        entries.insert("key.session".to_owned(), make_entry(MemoryTtl::Session));
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemFile = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, back);
    }

    // -----------------------------------------------------------------------
    // D: Serde roundtrip — duration TTL
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_file_serde_roundtrip_duration_ttl() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "key.duration".to_owned(),
            make_entry(MemoryTtl::Duration("P30D".to_owned())),
        );
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemFile = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, back);
    }

    // -----------------------------------------------------------------------
    // E: All scopes in a single file
    // -----------------------------------------------------------------------

    #[test]
    fn test_mem_file_serde_roundtrip_all_scopes() {
        let mut entries = BTreeMap::new();
        for (key, scope) in [
            ("k.node", MemoryScope::Node),
            ("k.session", MemoryScope::Session),
            ("k.project", MemoryScope::Project),
            ("k.global", MemoryScope::Global),
        ] {
            entries.insert(
                key.to_owned(),
                MemFileEntry {
                    topic: "test".to_owned(),
                    scope,
                    ttl: MemoryTtl::Permanent,
                    value: "val".to_owned(),
                    created_at: "2026-04-08T10:00:00Z".to_owned(),
                    updated_at: "2026-04-08T10:00:00Z".to_owned(),
                },
            );
        }
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemFile = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, back);
    }
}
