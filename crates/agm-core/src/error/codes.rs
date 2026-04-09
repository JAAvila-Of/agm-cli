//! Error code registry (spec Appendix D).

use std::fmt;

use super::diagnostic::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Parse,
    Validation,
    Import,
    Runtime,
    Lint,
}

impl ErrorCategory {
    #[must_use]
    pub fn prefix(self) -> char {
        match self {
            Self::Parse => 'P',
            Self::Validation => 'V',
            Self::Import => 'I',
            Self::Runtime => 'R',
            Self::Lint => 'L',
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Parse => "parse",
            Self::Validation => "validation",
            Self::Import => "import",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    P001,
    P002,
    P003,
    P004,
    P005,
    P006,
    P007,
    P008,
    P009,
    P010,
    V001,
    V002,
    V003,
    V004,
    V005,
    V006,
    V007,
    V008,
    V009,
    V010,
    V011,
    V012,
    V013,
    V014,
    V015,
    V016,
    V017,
    V018,
    V019,
    V020,
    V021,
    V022,
    V023,
    V024,
    V025,
    V026,
    I001,
    I002,
    I003,
    I004,
    I005,
    R001,
    R002,
    R003,
    R004,
    R005,
    R006,
    R007,
    R008,
    /// Lint: summary > 100 chars (quality hint)
    L001,
    /// Lint: duplicate summaries across nodes
    L002,
    /// Lint: node has too many fields (granularity hint)
    L003,
}

impl ErrorCode {
    #[must_use]
    pub fn category(self) -> ErrorCategory {
        match self {
            Self::P001
            | Self::P002
            | Self::P003
            | Self::P004
            | Self::P005
            | Self::P006
            | Self::P007
            | Self::P008
            | Self::P009
            | Self::P010 => ErrorCategory::Parse,
            Self::V001
            | Self::V002
            | Self::V003
            | Self::V004
            | Self::V005
            | Self::V006
            | Self::V007
            | Self::V008
            | Self::V009
            | Self::V010
            | Self::V011
            | Self::V012
            | Self::V013
            | Self::V014
            | Self::V015
            | Self::V016
            | Self::V017
            | Self::V018
            | Self::V019
            | Self::V020
            | Self::V021
            | Self::V022
            | Self::V023
            | Self::V024
            | Self::V025
            | Self::V026 => ErrorCategory::Validation,
            Self::I001 | Self::I002 | Self::I003 | Self::I004 | Self::I005 => ErrorCategory::Import,
            Self::R001
            | Self::R002
            | Self::R003
            | Self::R004
            | Self::R005
            | Self::R006
            | Self::R007
            | Self::R008 => ErrorCategory::Runtime,
            Self::L001 | Self::L002 | Self::L003 => ErrorCategory::Lint,
        }
    }

    #[must_use]
    pub fn number(self) -> u16 {
        match self {
            Self::P001 | Self::V001 | Self::I001 | Self::R001 | Self::L001 => 1,
            Self::P002 | Self::V002 | Self::I002 | Self::R002 | Self::L002 => 2,
            Self::P003 | Self::V003 | Self::I003 | Self::R003 | Self::L003 => 3,
            Self::P004 | Self::V004 | Self::I004 | Self::R004 => 4,
            Self::P005 | Self::V005 | Self::I005 | Self::R005 => 5,
            Self::P006 | Self::V006 | Self::R006 => 6,
            Self::P007 | Self::V007 | Self::R007 => 7,
            Self::P008 | Self::V008 | Self::R008 => 8,
            Self::P009 | Self::V009 => 9,
            Self::P010 | Self::V010 => 10,
            Self::V011 => 11,
            Self::V012 => 12,
            Self::V013 => 13,
            Self::V014 => 14,
            Self::V015 => 15,
            Self::V016 => 16,
            Self::V017 => 17,
            Self::V018 => 18,
            Self::V019 => 19,
            Self::V020 => 20,
            Self::V021 => 21,
            Self::V022 => 22,
            Self::V023 => 23,
            Self::V024 => 24,
            Self::V025 => 25,
            Self::V026 => 26,
        }
    }

    #[must_use]
    pub fn default_severity(self) -> Severity {
        match self {
            Self::P009 => Severity::Warning,
            Self::P010 => Severity::Info,
            Self::P001
            | Self::P002
            | Self::P003
            | Self::P004
            | Self::P005
            | Self::P006
            | Self::P007
            | Self::P008 => Severity::Error,
            Self::V010 | Self::V011 | Self::V012 | Self::V013 | Self::V014 | Self::V017 => {
                Severity::Warning
            }
            Self::V001
            | Self::V002
            | Self::V003
            | Self::V004
            | Self::V005
            | Self::V006
            | Self::V007
            | Self::V008
            | Self::V009
            | Self::V015
            | Self::V016
            | Self::V018
            | Self::V019
            | Self::V020
            | Self::V021
            | Self::V022
            | Self::V023
            | Self::V024
            | Self::V025 => Severity::Error,
            Self::V026 => Severity::Warning,
            Self::I005 => Severity::Warning,
            Self::I001 | Self::I002 | Self::I003 | Self::I004 => Severity::Error,
            Self::R007 => Severity::Warning,
            Self::R001
            | Self::R002
            | Self::R003
            | Self::R004
            | Self::R005
            | Self::R006
            | Self::R008 => Severity::Error,
            Self::L001 | Self::L002 | Self::L003 => Severity::Info,
        }
    }

    #[must_use]
    pub fn message_template(self) -> &'static str {
        match self {
            Self::P001 => "Missing required header field: `{field}`",
            Self::P002 => "Invalid node declaration syntax",
            Self::P003 => "Unexpected indentation",
            Self::P004 => "Tab character in indentation (spaces required)",
            Self::P005 => "Unterminated block field",
            Self::P006 => "Duplicate field `{field}` in node `{node}`",
            Self::P007 => "Invalid inline list syntax",
            Self::P008 => "Empty file (no nodes)",
            Self::P009 => "Unknown field `{field}` in node `{node}`",
            Self::P010 => {
                "File spec version `{file_version}` newer than parser version `{parser_version}`"
            }
            Self::V001 => "Node `{node}` missing required field: `type`",
            Self::V002 => "Node `{node}` missing required field: `summary`",
            Self::V003 => "Duplicate node ID: `{node}`",
            Self::V004 => "Unresolved reference `{ref}` in `{field}` of node `{node}`",
            Self::V005 => "Cycle detected in `depends`: `{cycle_path}`",
            Self::V006 => "Invalid `execution_status` value: `{value}`",
            Self::V007 => "`valid_from` is after `valid_until` in node `{node}`",
            Self::V008 => "Code block missing required field: `{field}`",
            Self::V009 => "Verify entry missing required field: `{field}`",
            Self::V010 => "Node type `{type}` typically includes field `{field}` (missing)",
            Self::V011 => "Summary is empty in node `{node}`",
            Self::V012 => "Summary exceeds 200 characters in node `{node}`",
            Self::V013 => "Conflicting active nodes co-loaded: `{node_a}` and `{node_b}`",
            Self::V014 => "Deprecated node `{node}` missing `replaces` or `superseded_by`",
            Self::V015 => "`target` path is absolute or contains traversal: `{path}`",
            Self::V016 => "Disallowed field `{field}` on node type `{type}` (strict mode)",
            Self::V017 => "Disallowed field `{field}` on node type `{type}` (standard mode)",
            Self::V018 => "Orchestration group `{group}` references non-existent node `{node}`",
            Self::V019 => "Cycle in orchestration `requires`: `{cycle_path}`",
            Self::V020 => "Invalid `execution_status` transition: `{from}` -> `{to}`",
            Self::V021 => "Node ID does not match required pattern: `{node}`",
            Self::V022 => "Memory key does not match required pattern: `{key}`",
            Self::V023 => "Invalid memory action: `{action}`",
            Self::V024 => "Node `{node}` of type `{type}` missing required schema field: `{field}`",
            Self::V025 => "Memory topic does not match required pattern: `{topic}`",
            Self::V026 => {
                "Unresolved memory topic `{topic}` in `agent_context.load_memory` of node `{node}`"
            }
            Self::I001 => "Unresolved import: `{package}`",
            Self::I002 => {
                "Import version constraint not satisfied: `{package}@{constraint}` (found `{actual}`)"
            }
            Self::I003 => "Circular import detected: `{cycle_path}`",
            Self::I004 => "Cross-package reference to non-existent node: `{ref}`",
            Self::I005 => "Import `{package}` is deprecated",
            Self::R001 => "Code block target file not found: `{path}`",
            Self::R002 => "Code block anchor/old pattern not found in target: `{pattern}`",
            Self::R003 => "Code block anchor/old matches multiple locations in target",
            Self::R004 => "Verification check failed: `{check_description}`",
            Self::R005 => "Agent context file not found: `{path}`",
            Self::R006 => "Memory operation failed: `{action}` on key `{key}`",
            Self::R007 => "Node `{node}` retry count exceeded threshold: `{count}`",
            Self::R008 => "Execution timeout for node `{node}`",
            Self::L001 => "Summary of node `{node}` exceeds 100 characters (quality hint)",
            Self::L002 => "Nodes `{node_a}` and `{node_b}` have identical summaries",
            Self::L003 => "Node `{node}` has too many populated fields; consider splitting",
        }
    }

    #[must_use]
    pub fn display_code(self) -> String {
        format!("AGM-{}{:03}", self.category().prefix(), self.number())
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AGM-{}{:03}", self.category().prefix(), self.number())
    }
}

impl serde::Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for ErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl std::str::FromStr for ErrorCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AGM-P001" => Ok(Self::P001),
            "AGM-P002" => Ok(Self::P002),
            "AGM-P003" => Ok(Self::P003),
            "AGM-P004" => Ok(Self::P004),
            "AGM-P005" => Ok(Self::P005),
            "AGM-P006" => Ok(Self::P006),
            "AGM-P007" => Ok(Self::P007),
            "AGM-P008" => Ok(Self::P008),
            "AGM-P009" => Ok(Self::P009),
            "AGM-P010" => Ok(Self::P010),
            "AGM-V001" => Ok(Self::V001),
            "AGM-V002" => Ok(Self::V002),
            "AGM-V003" => Ok(Self::V003),
            "AGM-V004" => Ok(Self::V004),
            "AGM-V005" => Ok(Self::V005),
            "AGM-V006" => Ok(Self::V006),
            "AGM-V007" => Ok(Self::V007),
            "AGM-V008" => Ok(Self::V008),
            "AGM-V009" => Ok(Self::V009),
            "AGM-V010" => Ok(Self::V010),
            "AGM-V011" => Ok(Self::V011),
            "AGM-V012" => Ok(Self::V012),
            "AGM-V013" => Ok(Self::V013),
            "AGM-V014" => Ok(Self::V014),
            "AGM-V015" => Ok(Self::V015),
            "AGM-V016" => Ok(Self::V016),
            "AGM-V017" => Ok(Self::V017),
            "AGM-V018" => Ok(Self::V018),
            "AGM-V019" => Ok(Self::V019),
            "AGM-V020" => Ok(Self::V020),
            "AGM-V021" => Ok(Self::V021),
            "AGM-V022" => Ok(Self::V022),
            "AGM-V023" => Ok(Self::V023),
            "AGM-V024" => Ok(Self::V024),
            "AGM-V025" => Ok(Self::V025),
            "AGM-V026" => Ok(Self::V026),
            "AGM-I001" => Ok(Self::I001),
            "AGM-I002" => Ok(Self::I002),
            "AGM-I003" => Ok(Self::I003),
            "AGM-I004" => Ok(Self::I004),
            "AGM-I005" => Ok(Self::I005),
            "AGM-R001" => Ok(Self::R001),
            "AGM-R002" => Ok(Self::R002),
            "AGM-R003" => Ok(Self::R003),
            "AGM-R004" => Ok(Self::R004),
            "AGM-R005" => Ok(Self::R005),
            "AGM-R006" => Ok(Self::R006),
            "AGM-R007" => Ok(Self::R007),
            "AGM-R008" => Ok(Self::R008),
            "AGM-L001" => Ok(Self::L001),
            "AGM-L002" => Ok(Self::L002),
            "AGM-L003" => Ok(Self::L003),
            _ => Err(format!("unknown error code: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_display_parse_formats_correctly() {
        assert_eq!(ErrorCode::P001.to_string(), "AGM-P001");
        assert_eq!(ErrorCode::P010.to_string(), "AGM-P010");
    }

    #[test]
    fn test_error_code_display_validation_formats_correctly() {
        assert_eq!(ErrorCode::V001.to_string(), "AGM-V001");
        assert_eq!(ErrorCode::V023.to_string(), "AGM-V023");
    }

    #[test]
    fn test_error_code_display_import_formats_correctly() {
        assert_eq!(ErrorCode::I001.to_string(), "AGM-I001");
        assert_eq!(ErrorCode::I005.to_string(), "AGM-I005");
    }

    #[test]
    fn test_error_code_display_runtime_formats_correctly() {
        assert_eq!(ErrorCode::R001.to_string(), "AGM-R001");
        assert_eq!(ErrorCode::R008.to_string(), "AGM-R008");
    }

    #[test]
    fn test_error_code_category_returns_correct_category() {
        assert_eq!(ErrorCode::P005.category(), ErrorCategory::Parse);
        assert_eq!(ErrorCode::V012.category(), ErrorCategory::Validation);
        assert_eq!(ErrorCode::I003.category(), ErrorCategory::Import);
        assert_eq!(ErrorCode::R006.category(), ErrorCategory::Runtime);
    }

    #[test]
    fn test_error_code_default_severity_error_codes() {
        assert_eq!(ErrorCode::P001.default_severity(), Severity::Error);
        assert_eq!(ErrorCode::V003.default_severity(), Severity::Error);
        assert_eq!(ErrorCode::I001.default_severity(), Severity::Error);
        assert_eq!(ErrorCode::R001.default_severity(), Severity::Error);
    }

    #[test]
    fn test_error_code_default_severity_warning_codes() {
        assert_eq!(ErrorCode::P009.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V010.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V011.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V012.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V013.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V014.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::V017.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::I005.default_severity(), Severity::Warning);
        assert_eq!(ErrorCode::R007.default_severity(), Severity::Warning);
    }

    #[test]
    fn test_error_code_default_severity_info_codes() {
        assert_eq!(ErrorCode::P010.default_severity(), Severity::Info);
    }

    #[test]
    fn test_error_code_message_template_not_empty() {
        assert!(ErrorCode::P001.message_template().contains("header"));
        assert!(ErrorCode::V003.message_template().contains("Duplicate"));
        assert!(ErrorCode::I002.message_template().contains("constraint"));
        assert!(ErrorCode::R004.message_template().contains("Verification"));
    }

    #[test]
    fn test_error_code_total_count_is_52() {
        let all_codes: Vec<ErrorCode> = vec![
            ErrorCode::P001,
            ErrorCode::P002,
            ErrorCode::P003,
            ErrorCode::P004,
            ErrorCode::P005,
            ErrorCode::P006,
            ErrorCode::P007,
            ErrorCode::P008,
            ErrorCode::P009,
            ErrorCode::P010,
            ErrorCode::V001,
            ErrorCode::V002,
            ErrorCode::V003,
            ErrorCode::V004,
            ErrorCode::V005,
            ErrorCode::V006,
            ErrorCode::V007,
            ErrorCode::V008,
            ErrorCode::V009,
            ErrorCode::V010,
            ErrorCode::V011,
            ErrorCode::V012,
            ErrorCode::V013,
            ErrorCode::V014,
            ErrorCode::V015,
            ErrorCode::V016,
            ErrorCode::V017,
            ErrorCode::V018,
            ErrorCode::V019,
            ErrorCode::V020,
            ErrorCode::V021,
            ErrorCode::V022,
            ErrorCode::V023,
            ErrorCode::V024,
            ErrorCode::V025,
            ErrorCode::V026,
            ErrorCode::I001,
            ErrorCode::I002,
            ErrorCode::I003,
            ErrorCode::I004,
            ErrorCode::I005,
            ErrorCode::R001,
            ErrorCode::R002,
            ErrorCode::R003,
            ErrorCode::R004,
            ErrorCode::R005,
            ErrorCode::R006,
            ErrorCode::R007,
            ErrorCode::R008,
            ErrorCode::L001,
            ErrorCode::L002,
            ErrorCode::L003,
        ];
        assert_eq!(all_codes.len(), 52);
    }

    #[test]
    fn test_error_code_from_str_roundtrip() {
        let code = ErrorCode::V003;
        let s = code.to_string();
        let parsed: ErrorCode = s.parse().unwrap();
        assert_eq!(parsed, code);
    }

    #[test]
    fn test_error_code_from_str_invalid_returns_err() {
        let result = "AGM-X999".parse::<ErrorCode>();
        assert!(result.is_err());
    }

    #[test]
    fn test_error_code_serde_roundtrip() {
        let code = ErrorCode::P003;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"AGM-P003\"");
        let deserialized: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, code);
    }

    #[test]
    fn test_error_category_prefix_chars() {
        assert_eq!(ErrorCategory::Parse.prefix(), 'P');
        assert_eq!(ErrorCategory::Validation.prefix(), 'V');
        assert_eq!(ErrorCategory::Import.prefix(), 'I');
        assert_eq!(ErrorCategory::Runtime.prefix(), 'R');
        assert_eq!(ErrorCategory::Lint.prefix(), 'L');
    }

    #[test]
    fn test_lint_codes_display_correctly() {
        assert_eq!(ErrorCode::L001.to_string(), "AGM-L001");
        assert_eq!(ErrorCode::L002.to_string(), "AGM-L002");
        assert_eq!(ErrorCode::L003.to_string(), "AGM-L003");
    }

    #[test]
    fn test_lint_codes_default_severity_is_info() {
        assert_eq!(ErrorCode::L001.default_severity(), Severity::Info);
        assert_eq!(ErrorCode::L002.default_severity(), Severity::Info);
        assert_eq!(ErrorCode::L003.default_severity(), Severity::Info);
    }

    #[test]
    fn test_lint_codes_from_str_roundtrip() {
        for code in [ErrorCode::L001, ErrorCode::L002, ErrorCode::L003] {
            let s = code.to_string();
            let parsed: ErrorCode = s.parse().unwrap();
            assert_eq!(parsed, code);
        }
    }
}
