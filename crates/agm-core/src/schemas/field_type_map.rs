//! Pure mapping from canonical AGM field name to its JSON Schema fragment.
//!
//! Every field that appears in either `UNIVERSAL_FIELDS` or any type-schema's
//! `required + recommended + allowed` list must have an entry here.

use serde_json::{Value, json};

/// Returns a JSON Schema fragment for the given canonical field name.
///
/// When `include_enums` is `true`, enum-backed fields emit `enum` clauses.
/// When `false`, they emit `{type: "string"}`.
///
/// Returns `{type: "string"}` for unknown field names as a safe fallback.
#[must_use]
pub fn field_schema(name: &str, include_enums: bool) -> Value {
    match name {
        // ---- Always-present scalar strings ----
        "type" => {
            // type field — callers override with const in builder
            json!({ "type": "string" })
        }
        "summary" => json!({ "type": "string", "description": "One-line description of the node" }),

        // ---- Enum fields (universal) ----
        "priority" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["critical", "high", "normal", "low"],
                    "description": "Execution priority"
                })
            } else {
                json!({ "type": "string", "description": "Execution priority" })
            }
        }
        "stability" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["high", "medium", "low", "volatile"],
                    "description": "How stable this node is"
                })
            } else {
                json!({ "type": "string", "description": "How stable this node is" })
            }
        }
        "confidence" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["high", "medium", "low", "inferred", "tentative"],
                    "description": "Confidence level in this node's content"
                })
            } else {
                json!({ "type": "string", "description": "Confidence level" })
            }
        }
        "status" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["active", "draft", "deprecated", "superseded"],
                    "description": "Lifecycle status of this node"
                })
            } else {
                json!({ "type": "string", "description": "Lifecycle status" })
            }
        }
        "execution_status" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["pending", "ready", "in_progress", "completed", "failed", "blocked", "skipped"],
                    "description": "Current execution state"
                })
            } else {
                json!({ "type": "string", "description": "Current execution state" })
            }
        }

        // ---- Ticket enum fields ----
        "action" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["create", "edit", "close", "archive", "split", "link"],
                    "description": "The intent of the ticket emission (spec §14.4.1)"
                })
            } else {
                json!({ "type": "string", "description": "Ticket action" })
            }
        }
        "sdd_phase" => {
            if include_enums {
                json!({
                    "type": "string",
                    "enum": ["backlog", "explore", "propose", "spec", "design", "tasks", "apply", "verify", "archive"],
                    "description": "SDD pipeline phase for this ticket (spec §14.4.2)"
                })
            } else {
                json!({ "type": "string", "description": "SDD pipeline phase" })
            }
        }

        // ---- List-of-string fields ----
        "tags" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Free-form tags for search and categorisation"
        }),
        "keywords" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Keywords used in search / retrieval"
        }),
        "aliases" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Alternative names for this node"
        }),
        "scope" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Contexts or domains where this node applies"
        }),
        "depends" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Node IDs this node depends on"
        }),
        "related_to" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Related node IDs (non-dependency)"
        }),
        "replaces" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Node IDs this node supersedes"
        }),
        "conflicts" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Node IDs this node conflicts with"
        }),
        "see_also" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Additional reference node IDs"
        }),
        "items" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "List of items (facts or rules)"
        }),
        "steps" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Ordered steps in the workflow"
        }),
        "fields" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Field definitions for entity nodes"
        }),
        "input" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Input parameters"
        }),
        "output" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Output values"
        }),
        "rationale" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Reasons supporting this decision"
        }),
        "tradeoffs" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Known trade-offs"
        }),
        "resolution" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Steps to resolve an exception"
        }),
        "labels" => json!({
            "type": "array",
            "items": { "type": "string" },
            "description": "Ticket labels (e.g. sprint tags)"
        }),

        // ---- Scalar string fields ----
        "detail" => json!({
            "type": "string",
            "description": "Extended description or body text"
        }),
        "notes" => json!({
            "type": "string",
            "description": "Freeform notes"
        }),
        "examples" => json!({
            "type": "string",
            "description": "Inline examples block"
        }),
        "applies_when" => json!({
            "type": "string",
            "description": "Condition under which this node applies"
        }),
        "valid_from" => json!({
            "type": "string",
            "description": "ISO 8601 date from which this node is valid"
        }),
        "valid_until" => json!({
            "type": "string",
            "description": "ISO 8601 date until which this node is valid"
        }),
        "executed_by" => json!({
            "type": "string",
            "description": "Agent or user that executed this node"
        }),
        "executed_at" => json!({
            "type": "string",
            "description": "ISO 8601 timestamp of execution"
        }),
        "execution_log" => json!({
            "type": "string",
            "description": "Log output from execution"
        }),
        "target" => json!({
            "type": "string",
            "description": "File or resource target"
        }),

        // ---- Integer field ----
        "retry_count" => json!({
            "type": "integer",
            "minimum": 0,
            "description": "Number of times execution has been retried"
        }),

        // ---- Ticket scalar strings ----
        "title" => json!({
            "type": "string",
            "description": "Short ticket title (ticket nodes)"
        }),
        "description" => json!({
            "type": "string",
            "description": "Detailed ticket description"
        }),
        "prompt" => json!({
            "type": "string",
            "description": "Agent prompt for automatic ticket handling"
        }),
        "assignee" => json!({
            "type": "string",
            "description": "Person or agent assigned to this ticket"
        }),
        "ticket_id" => json!({
            "type": "string",
            "description": "External tracker reference (e.g. JIRA-123)"
        }),

        // ---- Structured object fields (use $ref) ----
        "code" => json!({
            "$ref": "#/$defs/code_block",
            "description": "Single code block"
        }),
        "code_blocks" => json!({
            "type": "array",
            "items": { "$ref": "#/$defs/code_block" },
            "description": "Multiple named code blocks"
        }),
        "verify" => json!({
            "type": "array",
            "items": { "$ref": "#/$defs/verify_check" },
            "description": "Verification checks"
        }),
        "agent_context" => json!({
            "$ref": "#/$defs/agent_context",
            "description": "Additional context passed to the executing agent"
        }),
        "parallel_groups" => json!({
            "type": "array",
            "items": { "$ref": "#/$defs/parallel_group" },
            "description": "Parallel execution groups (orchestration nodes)"
        }),
        "memory" => json!({
            "type": "array",
            "items": { "$ref": "#/$defs/memory_entry" },
            "description": "Memory operations to execute"
        }),

        // ---- Fallback ----
        _ => json!({ "type": "string" }),
    }
}

/// Returns the `$defs` object containing schemas for structured field types.
///
/// This is included at the root of every generated schema so `$ref` links resolve.
#[must_use]
pub fn defs(include_enums: bool) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();

    // code_block
    map.insert("code_block".to_owned(), code_block_def(include_enums));
    // verify_check
    map.insert("verify_check".to_owned(), verify_check_def());
    // agent_context
    map.insert("agent_context".to_owned(), agent_context_def());
    // parallel_group
    map.insert(
        "parallel_group".to_owned(),
        parallel_group_def(include_enums),
    );
    // memory_entry
    map.insert("memory_entry".to_owned(), memory_entry_def(include_enums));

    map
}

fn code_block_def(include_enums: bool) -> Value {
    let action_schema = if include_enums {
        json!({
            "type": "string",
            "enum": ["create", "append", "prepend", "replace", "insert_before", "insert_after", "full"]
        })
    } else {
        json!({ "type": "string" })
    };
    json!({
        "type": "object",
        "description": "A code block with optional language, target, and action",
        "required": ["action", "body"],
        "properties": {
            "lang": { "type": "string", "description": "Programming language" },
            "target": { "type": "string", "description": "File path target" },
            "action": action_schema,
            "body": { "type": "string", "description": "Code content" },
            "anchor": { "type": "string", "description": "Insertion anchor" },
            "old": { "type": "string", "description": "Old code being replaced" }
        },
        "additionalProperties": false
    })
}

fn verify_check_def() -> Value {
    // VerifyCheck is a tagged enum serialized with `type` as discriminant.
    // We use oneOf to match each variant.
    json!({
        "description": "A verification check (tagged union keyed by 'type')",
        "oneOf": [
            {
                "type": "object",
                "required": ["type", "run"],
                "properties": {
                    "type": { "const": "command" },
                    "run": { "type": "string" },
                    "expect": { "type": "string" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["type", "file"],
                "properties": {
                    "type": { "const": "file_exists" },
                    "file": { "type": "string" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["type", "file", "pattern"],
                "properties": {
                    "type": { "const": "file_contains" },
                    "file": { "type": "string" },
                    "pattern": { "type": "string" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["type", "file", "pattern"],
                "properties": {
                    "type": { "const": "file_not_contains" },
                    "file": { "type": "string" },
                    "pattern": { "type": "string" }
                },
                "additionalProperties": false
            },
            {
                "type": "object",
                "required": ["type", "node", "status"],
                "properties": {
                    "type": { "const": "node_status" },
                    "node": { "type": "string" },
                    "status": { "type": "string" }
                },
                "additionalProperties": false
            }
        ]
    })
}

fn agent_context_def() -> Value {
    json!({
        "type": "object",
        "description": "Additional context for the executing agent (spec §S25)",
        "properties": {
            "system_hint": { "type": "string", "description": "Free-form hint for the agent" },
            "load_nodes": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Node IDs to load into agent context"
            },
            "load_files": {
                "type": "array",
                "description": "Files to load into agent context"
            },
            "max_tokens": { "type": "integer", "minimum": 0 },
            "load_memory": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "additionalProperties": true
    })
}

fn parallel_group_def(include_enums: bool) -> Value {
    let strategy_schema = if include_enums {
        json!({ "type": "string", "enum": ["sequential", "parallel"] })
    } else {
        json!({ "type": "string" })
    };
    json!({
        "type": "object",
        "description": "A group of nodes to execute in parallel or sequentially",
        "required": ["group", "nodes", "strategy"],
        "properties": {
            "group": { "type": "string", "description": "Group identifier" },
            "nodes": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Node IDs in this group"
            },
            "strategy": strategy_schema,
            "requires": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Group IDs that must complete before this group starts"
            },
            "max_concurrency": {
                "type": "integer",
                "minimum": 1,
                "description": "Maximum concurrent node executions within the group"
            }
        },
        "additionalProperties": false
    })
}

fn memory_entry_def(include_enums: bool) -> Value {
    let action_schema = if include_enums {
        json!({ "type": "string", "enum": ["get", "upsert", "delete", "list", "search"] })
    } else {
        json!({ "type": "string" })
    };
    let scope_schema = if include_enums {
        json!({ "type": "string", "enum": ["node", "session", "project", "global"] })
    } else {
        json!({ "type": "string" })
    };
    json!({
        "type": "object",
        "description": "A memory operation entry",
        "required": ["key", "topic", "action"],
        "properties": {
            "key": { "type": "string" },
            "topic": { "type": "string" },
            "action": action_schema,
            "value": { "type": "string" },
            "scope": scope_schema,
            "ttl": { "type": "string", "description": "permanent | session | duration:<ISO8601>" },
            "query": { "type": "string" },
            "max_results": { "type": "integer", "minimum": 1 }
        },
        "additionalProperties": false
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_canonical_field_has_schema() {
        // All fields that can appear in type schemas or UNIVERSAL_FIELDS
        let all_fields = [
            "type",
            "summary",
            "priority",
            "stability",
            "confidence",
            "status",
            "execution_status",
            "action",
            "sdd_phase",
            "tags",
            "keywords",
            "aliases",
            "scope",
            "depends",
            "related_to",
            "replaces",
            "conflicts",
            "see_also",
            "items",
            "steps",
            "fields",
            "input",
            "output",
            "rationale",
            "tradeoffs",
            "resolution",
            "labels",
            "detail",
            "notes",
            "examples",
            "applies_when",
            "valid_from",
            "valid_until",
            "executed_by",
            "executed_at",
            "execution_log",
            "target",
            "retry_count",
            "title",
            "description",
            "prompt",
            "assignee",
            "ticket_id",
            "code",
            "code_blocks",
            "verify",
            "agent_context",
            "parallel_groups",
            "memory",
        ];
        for field in all_fields {
            let schema = field_schema(field, true);
            assert!(
                schema.is_object(),
                "field_schema({field:?}) returned non-object"
            );
            // Must have either "type", "$ref", or "oneOf"
            assert!(
                schema.get("type").is_some()
                    || schema.get("$ref").is_some()
                    || schema.get("oneOf").is_some(),
                "field_schema({field:?}) missing type/$ref/oneOf"
            );
        }
    }

    #[test]
    fn test_enum_fields_match_display_variants() {
        // priority
        let s = field_schema("priority", true);
        let vals: Vec<&str> = s["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["critical", "high", "normal", "low"]);

        // stability
        let s = field_schema("stability", true);
        let vals: Vec<&str> = s["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["high", "medium", "low", "volatile"]);

        // status
        let s = field_schema("status", true);
        let vals: Vec<&str> = s["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(vals, vec!["active", "draft", "deprecated", "superseded"]);
    }

    #[test]
    fn test_retry_count_is_integer_min_zero() {
        let s = field_schema("retry_count", true);
        assert_eq!(s["type"].as_str(), Some("integer"));
        assert_eq!(s["minimum"].as_u64(), Some(0));
    }

    #[test]
    fn test_priority_enum_has_four_values() {
        let s = field_schema("priority", true);
        assert_eq!(s["enum"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_ticket_action_enum_has_six_values() {
        let s = field_schema("action", true);
        assert_eq!(s["enum"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn test_sdd_phase_enum_has_nine_values() {
        let s = field_schema("sdd_phase", true);
        assert_eq!(s["enum"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn test_enum_fields_without_enums_return_string_type() {
        for field in [
            "priority",
            "stability",
            "confidence",
            "status",
            "execution_status",
            "action",
            "sdd_phase",
        ] {
            let s = field_schema(field, false);
            assert_eq!(
                s["type"].as_str(),
                Some("string"),
                "field_schema({field:?}, false) should be {{type: string}}"
            );
            assert!(
                s.get("enum").is_none(),
                "should not have enum when include_enums=false"
            );
        }
    }

    #[test]
    fn test_unknown_field_returns_string_type() {
        let s = field_schema("some_unknown_field_xyz", true);
        assert_eq!(s["type"].as_str(), Some("string"));
    }

    #[test]
    fn test_defs_contains_all_structured_types() {
        let d = defs(true);
        assert!(d.contains_key("code_block"));
        assert!(d.contains_key("verify_check"));
        assert!(d.contains_key("agent_context"));
        assert!(d.contains_key("parallel_group"));
        assert!(d.contains_key("memory_entry"));
    }
}
