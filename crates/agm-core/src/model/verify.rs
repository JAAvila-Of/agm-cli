//! Verification contract types (spec S24).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerifyCheck {
    Command {
        run: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        expect: Option<String>,
    },
    FileExists {
        file: String,
    },
    FileContains {
        file: String,
        pattern: String,
    },
    FileNotContains {
        file: String,
        pattern: String,
    },
    NodeStatus {
        node: String,
        status: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_command_serde_roundtrip() {
        let v = VerifyCheck::Command {
            run: "cargo check".to_owned(),
            expect: Some("exit_code_0".to_owned()),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_command_without_expect() {
        let v = VerifyCheck::Command {
            run: "echo hello".to_owned(),
            expect: None,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("expect"));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_file_exists_serde_roundtrip() {
        let v = VerifyCheck::FileExists {
            file: "src/main.rs".to_owned(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"type\":\"file_exists\""));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_file_contains_serde_roundtrip() {
        let v = VerifyCheck::FileContains {
            file: "src/lib.rs".to_owned(),
            pattern: "fn main".to_owned(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"type\":\"file_contains\""));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_file_not_contains_serde_roundtrip() {
        let v = VerifyCheck::FileNotContains {
            file: "src/lib.rs".to_owned(),
            pattern: "unsafe".to_owned(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"type\":\"file_not_contains\""));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_node_status_serde_roundtrip() {
        let v = VerifyCheck::NodeStatus {
            node: "auth.login".to_owned(),
            status: "completed".to_owned(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"type\":\"node_status\""));
        let back: VerifyCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_verify_deserialize_from_spec_json() {
        let json = r#"{"type":"command","run":"cargo check","expect":"exit_code_0"}"#;
        let v: VerifyCheck = serde_json::from_str(json).unwrap();
        assert!(matches!(v, VerifyCheck::Command { .. }));
    }
}
