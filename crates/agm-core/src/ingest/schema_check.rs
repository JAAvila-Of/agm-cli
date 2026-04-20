//! JSON Schema pre-validation bridge.
//!
//! Compiles the schema for the given node type and checks the raw JSON value
//! against it. Returns the first validation error as `IngestError::SchemaCheck`.

use serde_json::Value;

use crate::model::fields::NodeType;
use crate::schemas::{SchemaOptions, schema_for};

use super::IngestError;

/// Validates `value` against the JSON Schema for `node_type`.
///
/// Uses `SchemaOptions::default()` (Draft 2020-12, `strict: false`,
/// `include_enums: true`). The `jsonschema` crate compiles the schema at
/// call-time; for hot paths the caller should disable schema_check or cache.
///
/// Returns `Ok(())` on success or `Err(IngestError::SchemaCheck(...))` with the
/// first validation error's JSON pointer and message.
pub fn run_schema_check(node_type: &NodeType, value: &Value) -> Result<(), IngestError> {
    // Custom types have no built-in schema.
    if matches!(node_type, NodeType::Custom(_)) {
        return Err(IngestError::SchemaCheck(
            "custom types have no schema".to_owned(),
        ));
    }

    let schema_value = match schema_for(node_type, &SchemaOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            return Err(IngestError::SchemaCheck(format!(
                "failed to generate schema: {e}"
            )));
        }
    };

    let validator = jsonschema::validator_for(&schema_value)
        .map_err(|e| IngestError::SchemaCheck(format!("failed to compile schema: {e}")))?;

    if !validator.is_valid(value) {
        // Collect the first error from iter_errors
        let mut errs = validator.iter_errors(value);
        if let Some(first) = errs.next() {
            let pointer = first.instance_path.to_string();
            let msg = first.to_string();
            let full = if pointer.is_empty() {
                msg
            } else {
                format!("{pointer}: {msg}")
            };
            return Err(IngestError::SchemaCheck(full));
        }
        // Fallback if iter_errors was somehow empty
        return Err(IngestError::SchemaCheck(
            "schema validation failed".to_owned(),
        ));
    }

    Ok(())
}
