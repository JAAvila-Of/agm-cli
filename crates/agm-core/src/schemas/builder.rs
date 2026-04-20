//! Builds the base JSON Schema (Draft 2020-12 object schema) from the registry's
//! `TypeSchema` and `UNIVERSAL_FIELDS`.

use serde_json::{Value, json};

use crate::model::fields::NodeType;
use crate::model::schema::TypeSchema;
use crate::schema::registry::UNIVERSAL_FIELDS;

use super::dialect::SchemaOptions;
use super::field_type_map;

/// Builds a Draft 2020-12 object schema for the given node type.
///
/// The resulting schema:
/// - `required` = `["type", "summary"] ∪ type_schema.required`
/// - `properties` = union of universal + required + recommended + allowed field schemas
/// - `additionalProperties` = `false` when `opts.strict`, `true` otherwise
/// - `type` property is always a `const` matching the node type name
/// - `$defs` contains schemas for structured fields (code_block, etc.)
#[must_use]
pub fn build_base_schema(
    node_type: &NodeType,
    type_schema: &TypeSchema,
    opts: &SchemaOptions,
) -> Value {
    let type_name = node_type.to_string();
    let spec_section = spec_section_for(node_type);

    // --- Collect all field names to include in properties ---
    let mut all_fields: Vec<String> = UNIVERSAL_FIELDS.iter().map(|s| (*s).to_owned()).collect();
    for f in type_schema
        .required
        .iter()
        .chain(type_schema.recommended.iter())
        .chain(type_schema.allowed.iter())
    {
        if !all_fields.contains(f) {
            all_fields.push(f.clone());
        }
    }

    // --- Build properties map ---
    let mut properties = serde_json::Map::new();
    for field_name in &all_fields {
        let schema = if field_name == "type" {
            // type is always a const equal to the node type name
            json!({ "const": type_name })
        } else {
            field_type_map::field_schema(field_name, opts.include_enums)
        };
        properties.insert(field_name.clone(), schema);
    }

    // --- Build required list ---
    // Always include "type" and "summary", then add type-specific required fields.
    let mut required: Vec<String> = vec!["type".to_owned(), "summary".to_owned()];
    for f in &type_schema.required {
        if f != "summary" && !required.contains(f) {
            required.push(f.clone());
        }
    }

    // --- additionalProperties ---
    let additional_properties: Value = if opts.strict {
        Value::Bool(false)
    } else {
        Value::Bool(true)
    };

    // --- $defs ---
    let defs_map = field_type_map::defs(opts.include_enums);
    let defs_value = Value::Object(defs_map);

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "description": format!("AGM {type_name} node per spec v1.2.0 {spec_section}"),
        "required": required,
        "properties": properties,
        "additionalProperties": additional_properties,
        "$defs": defs_value,
    })
}

/// Returns the spec section reference for the given node type.
fn spec_section_for(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Facts => "§13.1",
        NodeType::Rules => "§13.2",
        NodeType::Workflow => "§13.3",
        NodeType::Entity => "§13.4",
        NodeType::Decision => "§13.5",
        NodeType::Exception => "§13.6",
        NodeType::Example => "§13.7",
        NodeType::Glossary => "§13.8",
        NodeType::AntiPattern => "§13.9",
        NodeType::Orchestration => "§13.10",
        NodeType::Ticket => "§13.11",
        NodeType::Custom(_) => "§13.x",
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::registry::get_schema;

    fn opts_default() -> SchemaOptions {
        SchemaOptions::default()
    }

    fn opts_strict() -> SchemaOptions {
        SchemaOptions {
            strict: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_ticket_required_includes_title_description_priority() {
        let ts = get_schema(&NodeType::Ticket).unwrap();
        let schema = build_base_schema(&NodeType::Ticket, &ts, &opts_default());
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            required.contains(&"title"),
            "required should contain 'title'"
        );
        assert!(
            required.contains(&"description"),
            "required should contain 'description'"
        );
        assert!(
            required.contains(&"priority"),
            "required should contain 'priority'"
        );
        assert!(required.contains(&"type"));
        assert!(required.contains(&"summary"));
    }

    #[test]
    fn test_strict_mode_sets_additional_properties_false() {
        let ts = get_schema(&NodeType::Ticket).unwrap();
        let schema = build_base_schema(&NodeType::Ticket, &ts, &opts_strict());
        assert_eq!(
            schema["additionalProperties"].as_bool(),
            Some(false),
            "strict mode must set additionalProperties=false"
        );
    }

    #[test]
    fn test_standard_mode_sets_additional_properties_true() {
        let ts = get_schema(&NodeType::Facts).unwrap();
        let schema = build_base_schema(&NodeType::Facts, &ts, &opts_default());
        assert_eq!(
            schema["additionalProperties"].as_bool(),
            Some(true),
            "default mode must set additionalProperties=true"
        );
    }

    #[test]
    fn test_type_const_matches_node_type_name() {
        for (nt, expected) in [
            (NodeType::Ticket, "ticket"),
            (NodeType::Orchestration, "orchestration"),
            (NodeType::Workflow, "workflow"),
            (NodeType::Facts, "facts"),
        ] {
            let ts = get_schema(&nt).unwrap();
            let schema = build_base_schema(&nt, &ts, &opts_default());
            let type_const = schema["properties"]["type"]["const"]
                .as_str()
                .expect("type.const should be a string");
            assert_eq!(
                type_const, expected,
                "type const for {nt} should be {expected}"
            );
        }
    }

    #[test]
    fn test_universal_fields_always_in_properties() {
        // Every universal field must appear in the properties of any type schema.
        for nt in [NodeType::Facts, NodeType::Ticket, NodeType::Orchestration] {
            let ts = get_schema(&nt).unwrap();
            let schema = build_base_schema(&nt, &ts, &opts_default());
            let props = schema["properties"].as_object().unwrap();
            for field in UNIVERSAL_FIELDS {
                assert!(
                    props.contains_key(*field),
                    "universal field '{field}' missing from properties of {nt} schema"
                );
            }
        }
    }

    #[test]
    fn test_schema_has_defs() {
        let ts = get_schema(&NodeType::Workflow).unwrap();
        let schema = build_base_schema(&NodeType::Workflow, &ts, &opts_default());
        assert!(schema.get("$defs").is_some(), "schema must have $defs");
        let defs = schema["$defs"].as_object().unwrap();
        assert!(defs.contains_key("code_block"));
    }

    #[test]
    fn test_schema_has_json_schema_version() {
        let ts = get_schema(&NodeType::Rules).unwrap();
        let schema = build_base_schema(&NodeType::Rules, &ts, &opts_default());
        assert_eq!(
            schema["$schema"].as_str(),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
    }
}
