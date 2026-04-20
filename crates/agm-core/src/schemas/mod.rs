//! JSON Schema emission for every built-in AGM node type.
//!
//! Schemas are generated from `schema::registry` so they stay synchronized
//! with the validator's definition of what is valid.

pub mod builder;
pub mod dialect;
pub mod field_type_map;

use serde_json::Value;

use crate::model::fields::NodeType;

use self::builder::build_base_schema;
use self::dialect::dialect_wrap;

// ---------------------------------------------------------------------------
// Public types (re-exported)
// ---------------------------------------------------------------------------

pub use self::dialect::{SchemaDialect, SchemaOptions};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during schema generation.
#[derive(Debug, thiserror::Error)]
pub enum SchemaGenError {
    /// Custom node types have no built-in schema definition.
    #[error("no schema defined for custom node type: `{0}`")]
    CustomType(String),
    /// Tool name must match `^[a-z][a-z0-9_]*$`.
    #[error("invalid tool name: {0}")]
    InvalidToolName(String),
}

// ---------------------------------------------------------------------------
// Primary API
// ---------------------------------------------------------------------------

/// Produces a JSON Schema (Draft 2020-12) for the given node type.
///
/// Returns `Err` for `NodeType::Custom` since there is no built-in schema.
///
/// # Errors
///
/// Returns [`SchemaGenError::CustomType`] if `node_type` is `NodeType::Custom`.
/// Returns [`SchemaGenError::InvalidToolName`] if `opts.tool_name` contains
/// characters outside `[a-z][a-z0-9_]*`.
pub fn schema_for(node_type: &NodeType, opts: &SchemaOptions) -> Result<Value, SchemaGenError> {
    // Validate tool_name if provided.
    if let Some(ref name) = opts.tool_name {
        let re = regex::Regex::new(r"^[a-z][a-z0-9_]*$").expect("static regex");
        if !re.is_match(name) {
            return Err(SchemaGenError::InvalidToolName(name.clone()));
        }
    }

    match node_type {
        NodeType::Custom(s) => Err(SchemaGenError::CustomType(s.clone())),
        other => {
            let type_schema = crate::schema::registry::get_schema(other)
                .expect("get_schema returns Some for all non-Custom variants");
            let base = build_base_schema(other, &type_schema, opts);
            Ok(dialect_wrap(base, other, opts))
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience functions — one per built-in node type
// ---------------------------------------------------------------------------

/// Returns a JSON Schema for an AGM `facts` node (vanilla, default options).
#[must_use]
pub fn facts_schema() -> Value {
    schema_for(&NodeType::Facts, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `rules` node (vanilla, default options).
#[must_use]
pub fn rules_schema() -> Value {
    schema_for(&NodeType::Rules, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `workflow` node (vanilla, default options).
#[must_use]
pub fn workflow_schema() -> Value {
    schema_for(&NodeType::Workflow, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `entity` node (vanilla, default options).
#[must_use]
pub fn entity_schema() -> Value {
    schema_for(&NodeType::Entity, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `decision` node (vanilla, default options).
#[must_use]
pub fn decision_schema() -> Value {
    schema_for(&NodeType::Decision, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `exception` node (vanilla, default options).
#[must_use]
pub fn exception_schema() -> Value {
    schema_for(&NodeType::Exception, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `example` node (vanilla, default options).
#[must_use]
pub fn example_schema() -> Value {
    schema_for(&NodeType::Example, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `glossary` node (vanilla, default options).
#[must_use]
pub fn glossary_schema() -> Value {
    schema_for(&NodeType::Glossary, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `anti_pattern` node (vanilla, default options).
#[must_use]
pub fn anti_pattern_schema() -> Value {
    schema_for(&NodeType::AntiPattern, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `orchestration` node (vanilla, default options).
#[must_use]
pub fn orchestration_schema() -> Value {
    schema_for(&NodeType::Orchestration, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema for an AGM `ticket` node (vanilla, default options).
#[must_use]
pub fn ticket_schema() -> Value {
    schema_for(&NodeType::Ticket, &SchemaOptions::default()).unwrap()
}

/// Returns a JSON Schema covering only the universal fields (spec §14.2).
///
/// These fields are allowed on every node type and are never flagged by the validator.
#[must_use]
pub fn universal_fields_schema() -> Value {
    use crate::schema::registry::UNIVERSAL_FIELDS;
    use serde_json::json;

    let mut properties = serde_json::Map::new();
    for field in UNIVERSAL_FIELDS {
        properties.insert(
            (*field).to_owned(),
            field_type_map::field_schema(field, true),
        );
    }
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "AGM Universal Fields",
        "description": "Fields allowed on every AGM node type (spec §14.2)",
        "type": "object",
        "properties": properties,
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_for_custom_returns_err() {
        let err = schema_for(
            &NodeType::Custom("widget".to_owned()),
            &SchemaOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, SchemaGenError::CustomType(_)));
        assert!(err.to_string().contains("widget"));
    }

    #[test]
    fn test_schema_for_invalid_tool_name_returns_err() {
        let opts = SchemaOptions {
            tool_name: Some("My-Tool".to_owned()),
            ..Default::default()
        };
        let err = schema_for(&NodeType::Ticket, &opts).unwrap_err();
        assert!(matches!(err, SchemaGenError::InvalidToolName(_)));
    }

    #[test]
    fn test_schema_for_valid_tool_name_returns_ok() {
        let opts = SchemaOptions {
            tool_name: Some("create_ticket".to_owned()),
            ..Default::default()
        };
        assert!(schema_for(&NodeType::Ticket, &opts).is_ok());
    }

    #[test]
    fn test_every_builtin_type_has_convenience_fn() {
        let schemas = [
            facts_schema(),
            rules_schema(),
            workflow_schema(),
            entity_schema(),
            decision_schema(),
            exception_schema(),
            example_schema(),
            glossary_schema(),
            anti_pattern_schema(),
            orchestration_schema(),
            ticket_schema(),
        ];
        for schema in schemas {
            assert!(schema.is_object(), "expected JSON object");
            assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));
        }
    }

    #[test]
    fn test_universal_fields_schema_is_object() {
        let s = universal_fields_schema();
        assert!(s.is_object());
        assert!(s.get("properties").is_some());
    }
}
