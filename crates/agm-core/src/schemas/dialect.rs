//! Dialect wrappers and `SchemaOptions`.
//!
//! Three dialects are supported:
//! - `JsonSchema` — raw Draft 2020-12, no wrapping.
//! - `AnthropicToolUse` — `{name, description, input_schema}`.
//! - `OpenAiTool` — `{type: "function", function: {name, description, parameters}}`.

use serde_json::{Value, json};

use crate::model::fields::NodeType;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Dialect wrappers around the base JSON Schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchemaDialect {
    /// Raw JSON Schema Draft 2020-12 — no wrapping.
    #[default]
    JsonSchema,
    /// Anthropic tool-use shape: `{name, description, input_schema}`.
    AnthropicToolUse,
    /// OpenAI function-calling shape:
    /// `{type: "function", function: {name, description, parameters}}`.
    OpenAiTool,
}

/// Options controlling schema generation.
#[derive(Debug, Clone)]
pub struct SchemaOptions {
    pub dialect: SchemaDialect,
    /// Emit enum values as JSON Schema `enum` clauses instead of plain `string`.
    /// Default: `true`.
    pub include_enums: bool,
    /// Tighten the schema to reject any field not in the allowed set.
    /// Sets `additionalProperties: false` and makes `type` a `const`.
    /// Default: `false`.
    pub strict: bool,
    /// Tool name for dialect wrapping. If `None`, defaults to the type's canonical tool name.
    pub tool_name: Option<String>,
    /// Tool description for dialect wrapping. If `None`, a default is generated.
    pub tool_description: Option<String>,
}

impl Default for SchemaOptions {
    fn default() -> Self {
        Self {
            dialect: SchemaDialect::JsonSchema,
            include_enums: true,
            strict: false,
            tool_name: None,
            tool_description: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Dialect wrapping
// ---------------------------------------------------------------------------

/// Wraps `base` in the appropriate dialect shape for `node_type` and `opts`.
///
/// For `SchemaDialect::JsonSchema`, this is a no-op.
#[must_use]
pub fn dialect_wrap(base: Value, node_type: &NodeType, opts: &SchemaOptions) -> Value {
    let defaults = type_defaults(node_type);

    let name = opts
        .tool_name
        .clone()
        .unwrap_or_else(|| defaults.tool_name.to_owned());

    let description = opts
        .tool_description
        .clone()
        .unwrap_or_else(|| defaults.tool_description.to_owned());

    match opts.dialect {
        SchemaDialect::JsonSchema => base,
        SchemaDialect::AnthropicToolUse => wrap_anthropic(base, &name, &description),
        SchemaDialect::OpenAiTool => wrap_openai(base, &name, &description),
    }
}

/// Produces Anthropic tool-use shape: `{name, description, input_schema}`.
#[must_use]
fn wrap_anthropic(base: Value, name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "input_schema": base,
    })
}

/// Produces OpenAI function-calling shape:
/// `{type: "function", function: {name, description, parameters}}`.
#[must_use]
fn wrap_openai(base: Value, name: &str, description: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": base,
        }
    })
}

// ---------------------------------------------------------------------------
// Per-type defaults
// ---------------------------------------------------------------------------

struct TypeDefaults {
    tool_name: &'static str,
    tool_description: &'static str,
}

fn type_defaults(node_type: &NodeType) -> TypeDefaults {
    match node_type {
        NodeType::Facts => TypeDefaults {
            tool_name: "emit_facts",
            tool_description: "Emit a facts node per AGM spec v1.2.0 §13.1",
        },
        NodeType::Rules => TypeDefaults {
            tool_name: "emit_rules",
            tool_description: "Emit a rules node per AGM spec v1.2.0 §13.2",
        },
        NodeType::Workflow => TypeDefaults {
            tool_name: "emit_workflow",
            tool_description: "Emit a workflow node per AGM spec v1.2.0 §13.3",
        },
        NodeType::Entity => TypeDefaults {
            tool_name: "emit_entity",
            tool_description: "Emit an entity node per AGM spec v1.2.0 §13.4",
        },
        NodeType::Decision => TypeDefaults {
            tool_name: "emit_decision",
            tool_description: "Emit a decision node per AGM spec v1.2.0 §13.5",
        },
        NodeType::Exception => TypeDefaults {
            tool_name: "emit_exception",
            tool_description: "Emit an exception node per AGM spec v1.2.0 §13.6",
        },
        NodeType::Example => TypeDefaults {
            tool_name: "emit_example",
            tool_description: "Emit an example node per AGM spec v1.2.0 §13.7",
        },
        NodeType::Glossary => TypeDefaults {
            tool_name: "emit_glossary",
            tool_description: "Emit a glossary node per AGM spec v1.2.0 §13.8",
        },
        NodeType::AntiPattern => TypeDefaults {
            tool_name: "emit_anti_pattern",
            tool_description: "Emit an anti_pattern node per AGM spec v1.2.0 §13.9",
        },
        NodeType::Orchestration => TypeDefaults {
            tool_name: "plan_execution",
            tool_description: "Emit an orchestration node per AGM spec v1.2.0 §13.10",
        },
        NodeType::Ticket => TypeDefaults {
            tool_name: "create_ticket",
            tool_description: "Emit a ticket node per AGM spec v1.2.0 §13.11",
        },
        NodeType::Custom(_) => TypeDefaults {
            tool_name: "emit_node",
            tool_description: "Emit a custom node",
        },
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_base() -> Value {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {},
            "required": ["type", "summary"],
        })
    }

    #[test]
    fn test_wrap_anthropic_shape() {
        let base = sample_base();
        let wrapped = wrap_anthropic(base.clone(), "create_ticket", "A ticket tool");
        assert_eq!(wrapped["name"].as_str(), Some("create_ticket"));
        assert_eq!(wrapped["description"].as_str(), Some("A ticket tool"));
        assert_eq!(wrapped["input_schema"], base);
        assert!(wrapped.get("function").is_none());
    }

    #[test]
    fn test_wrap_openai_shape() {
        let base = sample_base();
        let wrapped = wrap_openai(base.clone(), "create_ticket", "A ticket tool");
        assert_eq!(wrapped["type"].as_str(), Some("function"));
        let func = &wrapped["function"];
        assert_eq!(func["name"].as_str(), Some("create_ticket"));
        assert_eq!(func["description"].as_str(), Some("A ticket tool"));
        assert_eq!(func["parameters"], base);
    }

    #[test]
    fn test_wrap_vanilla_noop() {
        let base = sample_base();
        let opts = SchemaOptions {
            dialect: SchemaDialect::JsonSchema,
            ..Default::default()
        };
        let result = dialect_wrap(base.clone(), &NodeType::Facts, &opts);
        assert_eq!(result, base);
    }

    #[test]
    fn test_dialect_wrap_anthropic_uses_default_tool_name() {
        let base = sample_base();
        let opts = SchemaOptions {
            dialect: SchemaDialect::AnthropicToolUse,
            ..Default::default()
        };
        let result = dialect_wrap(base, &NodeType::Ticket, &opts);
        assert_eq!(result["name"].as_str(), Some("create_ticket"));
    }

    #[test]
    fn test_dialect_wrap_openai_uses_default_tool_name() {
        let base = sample_base();
        let opts = SchemaOptions {
            dialect: SchemaDialect::OpenAiTool,
            ..Default::default()
        };
        let result = dialect_wrap(base, &NodeType::Orchestration, &opts);
        assert_eq!(result["function"]["name"].as_str(), Some("plan_execution"));
    }

    #[test]
    fn test_dialect_wrap_custom_tool_name_overrides_default() {
        let base = sample_base();
        let opts = SchemaOptions {
            dialect: SchemaDialect::AnthropicToolUse,
            tool_name: Some("my_custom_ticket".to_owned()),
            ..Default::default()
        };
        let result = dialect_wrap(base, &NodeType::Ticket, &opts);
        assert_eq!(result["name"].as_str(), Some("my_custom_ticket"));
    }

    #[test]
    fn test_schema_options_default() {
        let opts = SchemaOptions::default();
        assert_eq!(opts.dialect, SchemaDialect::JsonSchema);
        assert!(opts.include_enums);
        assert!(!opts.strict);
        assert!(opts.tool_name.is_none());
        assert!(opts.tool_description.is_none());
    }
}
