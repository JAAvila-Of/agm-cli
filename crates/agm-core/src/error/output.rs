//! Error output formatting (spec section 21.4).

use std::fmt;

use serde::{Deserialize, Serialize};

use super::diagnostic::AgmError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorOutputFormat {
    Text,
    Json,
}

impl fmt::Display for ErrorOutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
        }
    }
}

impl std::str::FromStr for ErrorOutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unknown error output format: {other} (expected `text` or `json`)"
            )),
        }
    }
}

#[must_use]
pub fn format_error_text(error: &AgmError) -> String {
    let file = error.location.file.as_deref().unwrap_or("<unknown>");
    let location = match error.location.line {
        Some(line) => format!("{file}:{line}"),
        None => file.to_string(),
    };
    format!(
        "{location} [{code}] {severity}: {message}",
        code = error.code,
        severity = error.severity,
        message = error.message,
    )
}

#[must_use]
pub fn format_error_json(error: &AgmError) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "code".to_string(),
        serde_json::Value::String(error.code.to_string()),
    );
    map.insert(
        "severity".to_string(),
        serde_json::Value::String(error.severity.to_string()),
    );
    map.insert(
        "message".to_string(),
        serde_json::Value::String(error.message.clone()),
    );
    if let Some(ref file) = error.location.file {
        map.insert("file".to_string(), serde_json::Value::String(file.clone()));
    }
    if let Some(line) = error.location.line {
        map.insert(
            "line".to_string(),
            serde_json::Value::Number(serde_json::Number::from(line)),
        );
    }
    if let Some(ref node) = error.location.node {
        map.insert("node".to_string(), serde_json::Value::String(node.clone()));
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;
    use crate::error::diagnostic::{AgmError, ErrorLocation};

    #[test]
    fn test_format_error_text_full_location() {
        let err = AgmError::new(
            ErrorCode::V003,
            "Duplicate node ID: `auth.login`",
            ErrorLocation::full("file.agm", 42, "auth.login"),
        );
        let text = format_error_text(&err);
        assert_eq!(
            text,
            "file.agm:42 [AGM-V003] error: Duplicate node ID: `auth.login`"
        );
    }

    #[test]
    fn test_format_error_text_warning() {
        let err = AgmError::new(
            ErrorCode::V010,
            "Node type `workflow` typically includes field `steps` (missing)",
            ErrorLocation::full("file.agm", 87, "deploy.step3"),
        );
        let text = format_error_text(&err);
        assert_eq!(
            text,
            "file.agm:87 [AGM-V010] warning: Node type `workflow` typically includes field `steps` (missing)"
        );
    }

    #[test]
    fn test_format_error_text_no_file_uses_unknown() {
        let err = AgmError::new(
            ErrorCode::P008,
            "Empty file (no nodes)",
            ErrorLocation::default(),
        );
        let text = format_error_text(&err);
        assert_eq!(text, "<unknown> [AGM-P008] error: Empty file (no nodes)");
    }

    #[test]
    fn test_format_error_text_file_without_line() {
        let err = AgmError::new(
            ErrorCode::P008,
            "Empty file (no nodes)",
            ErrorLocation::new(Some("empty.agm".to_string()), None, None),
        );
        let text = format_error_text(&err);
        assert_eq!(text, "empty.agm [AGM-P008] error: Empty file (no nodes)");
    }

    #[test]
    fn test_format_error_json_full_location() {
        let err = AgmError::new(
            ErrorCode::V003,
            "Duplicate node ID: `auth.login`",
            ErrorLocation::full("file.agm", 42, "auth.login"),
        );
        let json = format_error_json(&err);
        assert_eq!(json["code"], "AGM-V003");
        assert_eq!(json["severity"], "error");
        assert_eq!(json["message"], "Duplicate node ID: `auth.login`");
        assert_eq!(json["file"], "file.agm");
        assert_eq!(json["line"], 42);
        assert_eq!(json["node"], "auth.login");
    }

    #[test]
    fn test_format_error_json_minimal_location() {
        let err = AgmError::new(
            ErrorCode::P008,
            "Empty file (no nodes)",
            ErrorLocation::default(),
        );
        let json = format_error_json(&err);
        assert_eq!(json["code"], "AGM-P008");
        assert_eq!(json["severity"], "error");
        assert!(json.get("file").is_none());
        assert!(json.get("line").is_none());
        assert!(json.get("node").is_none());
    }

    #[test]
    fn test_format_error_json_is_valid_json() {
        let err = AgmError::new(
            ErrorCode::V003,
            "test message",
            ErrorLocation::full("file.agm", 42, "auth.login"),
        );
        let json = format_error_json(&err);
        let json_string = serde_json::to_string(&json).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&json_string).unwrap();
        assert_eq!(json, reparsed);
    }

    #[test]
    fn test_format_error_json_warning_severity_value() {
        let err = AgmError::new(ErrorCode::V010, "test", ErrorLocation::default());
        let json = format_error_json(&err);
        assert_eq!(json["severity"], "warning");
    }

    #[test]
    fn test_format_error_json_info_severity_value() {
        let err = AgmError::new(ErrorCode::P010, "test", ErrorLocation::default());
        let json = format_error_json(&err);
        assert_eq!(json["severity"], "info");
    }

    #[test]
    fn test_error_output_format_from_str_text() {
        assert_eq!(
            "text".parse::<ErrorOutputFormat>().unwrap(),
            ErrorOutputFormat::Text
        );
        assert_eq!(
            "TEXT".parse::<ErrorOutputFormat>().unwrap(),
            ErrorOutputFormat::Text
        );
    }

    #[test]
    fn test_error_output_format_from_str_json() {
        assert_eq!(
            "json".parse::<ErrorOutputFormat>().unwrap(),
            ErrorOutputFormat::Json
        );
        assert_eq!(
            "JSON".parse::<ErrorOutputFormat>().unwrap(),
            ErrorOutputFormat::Json
        );
    }

    #[test]
    fn test_error_output_format_from_str_invalid() {
        assert!("xml".parse::<ErrorOutputFormat>().is_err());
    }

    #[test]
    fn test_error_output_format_display() {
        assert_eq!(ErrorOutputFormat::Text.to_string(), "text");
        assert_eq!(ErrorOutputFormat::Json.to_string(), "json");
    }
}
