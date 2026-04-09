//! Built-in type schema definitions (spec §14.1) and universal field list (spec §14.2).

use crate::model::fields::NodeType;
use crate::model::schema::TypeSchema;

/// Universal fields (spec §14.2) — always allowed on every node type, never flagged.
pub const UNIVERSAL_FIELDS: &[&str] = &[
    "type",
    "summary",
    "priority",
    "stability",
    "confidence",
    "status",
    "tags",
    "keywords",
    "aliases",
    "scope",
    "applies_when",
    "valid_from",
    "valid_until",
    "depends",
    "related_to",
    "replaces",
    "conflicts",
    "see_also",
    "notes",
    "execution_status",
    "executed_by",
    "executed_at",
    "execution_log",
    "retry_count",
    "memory",
];

/// Returns the type schema for a built-in node type, or `None` for `Custom` types.
#[must_use]
pub fn get_schema(node_type: &NodeType) -> Option<TypeSchema> {
    match node_type {
        NodeType::Facts => Some(facts_schema()),
        NodeType::Rules => Some(rules_schema()),
        NodeType::Workflow => Some(workflow_schema()),
        NodeType::Entity => Some(entity_schema()),
        NodeType::Decision => Some(decision_schema()),
        NodeType::Exception => Some(exception_schema()),
        NodeType::Example => Some(example_schema()),
        NodeType::Glossary => Some(glossary_schema()),
        NodeType::AntiPattern => Some(anti_pattern_schema()),
        NodeType::Orchestration => Some(orchestration_schema()),
        NodeType::Custom(_) => None,
    }
}

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn facts_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["items", "stability", "priority"]),
        allowed: s(&["detail", "tags", "fields"]),
        disallowed: s(&["steps", "rationale", "resolution", "parallel_groups"]),
    }
}

fn rules_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary", "items"]),
        recommended: s(&["stability", "priority"]),
        allowed: s(&["detail", "scope", "applies_when"]),
        disallowed: s(&["steps", "fields", "parallel_groups"]),
    }
}

fn workflow_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["steps", "input", "output", "stability", "priority"]),
        allowed: s(&[
            "detail",
            "code",
            "code_blocks",
            "verify",
            "agent_context",
            "target",
        ]),
        disallowed: s(&["fields", "parallel_groups"]),
    }
}

fn entity_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary", "fields"]),
        recommended: s(&["stability", "priority"]),
        allowed: s(&["detail", "related_to"]),
        disallowed: s(&[
            "steps",
            "rationale",
            "resolution",
            "parallel_groups",
            "code",
        ]),
    }
}

fn decision_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary", "rationale"]),
        recommended: s(&["stability", "priority", "tradeoffs"]),
        allowed: s(&["detail", "depends", "conflicts", "replaces"]),
        disallowed: s(&["steps", "fields", "parallel_groups", "code"]),
    }
}

fn exception_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["resolution", "stability", "priority"]),
        allowed: s(&["detail", "depends"]),
        disallowed: s(&["steps", "fields", "rationale", "parallel_groups", "code"]),
    }
}

fn example_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["depends"]),
        allowed: s(&["detail", "code", "code_blocks"]),
        disallowed: s(&["rationale", "resolution", "parallel_groups"]),
    }
}

fn glossary_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["aliases"]),
        allowed: s(&["detail", "related_to"]),
        disallowed: s(&[
            "steps",
            "fields",
            "rationale",
            "resolution",
            "code",
            "parallel_groups",
        ]),
    }
}

fn anti_pattern_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary"]),
        recommended: s(&["detail", "conflicts"]),
        allowed: s(&["related_to", "resolution"]),
        disallowed: s(&["steps", "fields", "rationale", "code", "parallel_groups"]),
    }
}

fn orchestration_schema() -> TypeSchema {
    TypeSchema {
        required: s(&["summary", "parallel_groups"]),
        recommended: s(&["detail"]),
        allowed: s(&["depends"]),
        disallowed: s(&[
            "steps",
            "items",
            "fields",
            "code",
            "code_blocks",
            "verify",
            "rationale",
            "resolution",
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_schema_facts_returns_correct_schema() {
        let schema = get_schema(&NodeType::Facts).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(schema.recommended, vec!["items", "stability", "priority"]);
        assert_eq!(schema.allowed, vec!["detail", "tags", "fields"]);
        assert_eq!(
            schema.disallowed,
            vec!["steps", "rationale", "resolution", "parallel_groups"]
        );
    }

    #[test]
    fn test_get_schema_rules_returns_correct_schema() {
        let schema = get_schema(&NodeType::Rules).unwrap();
        assert_eq!(schema.required, vec!["summary", "items"]);
        assert_eq!(schema.recommended, vec!["stability", "priority"]);
        assert_eq!(schema.allowed, vec!["detail", "scope", "applies_when"]);
        assert_eq!(
            schema.disallowed,
            vec!["steps", "fields", "parallel_groups"]
        );
    }

    #[test]
    fn test_get_schema_workflow_returns_correct_schema() {
        let schema = get_schema(&NodeType::Workflow).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(
            schema.recommended,
            vec!["steps", "input", "output", "stability", "priority"]
        );
        assert_eq!(
            schema.allowed,
            vec![
                "detail",
                "code",
                "code_blocks",
                "verify",
                "agent_context",
                "target"
            ]
        );
        assert_eq!(schema.disallowed, vec!["fields", "parallel_groups"]);
    }

    #[test]
    fn test_get_schema_entity_returns_correct_schema() {
        let schema = get_schema(&NodeType::Entity).unwrap();
        assert_eq!(schema.required, vec!["summary", "fields"]);
        assert_eq!(schema.recommended, vec!["stability", "priority"]);
        assert_eq!(schema.allowed, vec!["detail", "related_to"]);
        assert_eq!(
            schema.disallowed,
            vec![
                "steps",
                "rationale",
                "resolution",
                "parallel_groups",
                "code"
            ]
        );
    }

    #[test]
    fn test_get_schema_decision_returns_correct_schema() {
        let schema = get_schema(&NodeType::Decision).unwrap();
        assert_eq!(schema.required, vec!["summary", "rationale"]);
        assert_eq!(
            schema.recommended,
            vec!["stability", "priority", "tradeoffs"]
        );
        assert_eq!(
            schema.allowed,
            vec!["detail", "depends", "conflicts", "replaces"]
        );
        assert_eq!(
            schema.disallowed,
            vec!["steps", "fields", "parallel_groups", "code"]
        );
    }

    #[test]
    fn test_get_schema_exception_returns_correct_schema() {
        let schema = get_schema(&NodeType::Exception).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(
            schema.recommended,
            vec!["resolution", "stability", "priority"]
        );
        assert_eq!(schema.allowed, vec!["detail", "depends"]);
        assert_eq!(
            schema.disallowed,
            vec!["steps", "fields", "rationale", "parallel_groups", "code"]
        );
    }

    #[test]
    fn test_get_schema_example_returns_correct_schema() {
        let schema = get_schema(&NodeType::Example).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(schema.recommended, vec!["depends"]);
        assert_eq!(schema.allowed, vec!["detail", "code", "code_blocks"]);
        assert_eq!(
            schema.disallowed,
            vec!["rationale", "resolution", "parallel_groups"]
        );
    }

    #[test]
    fn test_get_schema_glossary_returns_correct_schema() {
        let schema = get_schema(&NodeType::Glossary).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(schema.recommended, vec!["aliases"]);
        assert_eq!(schema.allowed, vec!["detail", "related_to"]);
        assert_eq!(
            schema.disallowed,
            vec![
                "steps",
                "fields",
                "rationale",
                "resolution",
                "code",
                "parallel_groups"
            ]
        );
    }

    #[test]
    fn test_get_schema_anti_pattern_returns_correct_schema() {
        let schema = get_schema(&NodeType::AntiPattern).unwrap();
        assert_eq!(schema.required, vec!["summary"]);
        assert_eq!(schema.recommended, vec!["detail", "conflicts"]);
        assert_eq!(schema.allowed, vec!["related_to", "resolution"]);
        assert_eq!(
            schema.disallowed,
            vec!["steps", "fields", "rationale", "code", "parallel_groups"]
        );
    }

    #[test]
    fn test_get_schema_orchestration_returns_correct_schema() {
        let schema = get_schema(&NodeType::Orchestration).unwrap();
        assert_eq!(schema.required, vec!["summary", "parallel_groups"]);
        assert_eq!(schema.recommended, vec!["detail"]);
        assert_eq!(schema.allowed, vec!["depends"]);
        assert_eq!(
            schema.disallowed,
            vec![
                "steps",
                "items",
                "fields",
                "code",
                "code_blocks",
                "verify",
                "rationale",
                "resolution"
            ]
        );
    }

    #[test]
    fn test_get_schema_custom_type_returns_none() {
        assert!(get_schema(&NodeType::Custom("my_type".to_owned())).is_none());
    }

    #[test]
    fn test_universal_fields_count_is_26() {
        assert_eq!(UNIVERSAL_FIELDS.len(), 25);
        assert!(UNIVERSAL_FIELDS.contains(&"type"));
        assert!(UNIVERSAL_FIELDS.contains(&"summary"));
        assert!(UNIVERSAL_FIELDS.contains(&"priority"));
        assert!(UNIVERSAL_FIELDS.contains(&"stability"));
        assert!(UNIVERSAL_FIELDS.contains(&"confidence"));
        assert!(UNIVERSAL_FIELDS.contains(&"status"));
        assert!(UNIVERSAL_FIELDS.contains(&"tags"));
        assert!(UNIVERSAL_FIELDS.contains(&"keywords"));
        assert!(UNIVERSAL_FIELDS.contains(&"aliases"));
        assert!(UNIVERSAL_FIELDS.contains(&"scope"));
        assert!(UNIVERSAL_FIELDS.contains(&"applies_when"));
        assert!(UNIVERSAL_FIELDS.contains(&"valid_from"));
        assert!(UNIVERSAL_FIELDS.contains(&"valid_until"));
        assert!(UNIVERSAL_FIELDS.contains(&"depends"));
        assert!(UNIVERSAL_FIELDS.contains(&"related_to"));
        assert!(UNIVERSAL_FIELDS.contains(&"replaces"));
        assert!(UNIVERSAL_FIELDS.contains(&"conflicts"));
        assert!(UNIVERSAL_FIELDS.contains(&"see_also"));
        assert!(UNIVERSAL_FIELDS.contains(&"notes"));
        assert!(UNIVERSAL_FIELDS.contains(&"execution_status"));
        assert!(UNIVERSAL_FIELDS.contains(&"executed_by"));
        assert!(UNIVERSAL_FIELDS.contains(&"executed_at"));
        assert!(UNIVERSAL_FIELDS.contains(&"execution_log"));
        assert!(UNIVERSAL_FIELDS.contains(&"retry_count"));
        assert!(UNIVERSAL_FIELDS.contains(&"memory"));
    }
}
