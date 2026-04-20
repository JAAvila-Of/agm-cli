//! Header parser: reads the AGM file header and produces a `Header` struct.

use std::sync::LazyLock;

use regex::Regex;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::file::Header;

use super::fields::{
    FieldTracker, parse_block, parse_imports, parse_indented_list, skip_field_body,
};
use super::lexer::{Line, LineKind};
use super::structured::parse_load_profiles;

// ---------------------------------------------------------------------------
// Validation regexes
// ---------------------------------------------------------------------------

static AGM_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+\.[0-9]+$").unwrap());

static PACKAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)*$").unwrap());

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_agm_format(value: &str, line_number: usize, errors: &mut Vec<AgmError>) {
    if !AGM_VERSION_RE.is_match(value) {
        errors.push(AgmError::new(
            ErrorCode::P001,
            format!("Invalid `agm` format: expected MAJOR.MINOR, got {value:?}"),
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }
}

fn validate_package_format(value: &str, line_number: usize, errors: &mut Vec<AgmError>) {
    if !PACKAGE_RE.is_match(value) {
        errors.push(AgmError::new(
            ErrorCode::P001,
            format!("Invalid `package` format: {value:?}"),
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }
}

fn validate_version_format(value: &str, line_number: usize, errors: &mut Vec<AgmError>) {
    if semver::Version::parse(value).is_err() {
        errors.push(AgmError::new(
            ErrorCode::P001,
            format!("Invalid `version` (expected semver): {value:?}"),
            ErrorLocation::new(None, Some(line_number), None),
        ));
    }
}

// ---------------------------------------------------------------------------
// parse_header
// ---------------------------------------------------------------------------

/// Parses the header section of an AGM file.
///
/// Advances `pos` to the first `NodeDeclaration` or end of `lines`.
/// Emits errors into `errors`.
pub fn parse_header(lines: &[Line], pos: &mut usize, errors: &mut Vec<AgmError>) -> Header {
    let mut tracker = FieldTracker::new();

    let mut agm: Option<String> = None;
    let mut package: Option<String> = None;
    let mut version: Option<String> = None;
    let mut title: Option<String> = None;
    let mut owner: Option<String> = None;
    let mut default_load: Option<String> = None;
    let mut description: Option<String> = None;
    let mut status: Option<String> = None;
    let mut target_runtime: Option<String> = None;
    let mut tags: Option<Vec<String>> = None;
    let mut imports_raw: Option<Vec<crate::model::imports::ImportEntry>> = None;
    let mut load_profiles_parsed: Option<
        std::collections::BTreeMap<String, crate::model::file::LoadProfile>,
    > = None;

    while *pos < lines.len() {
        match &lines[*pos].kind.clone() {
            LineKind::NodeDeclaration(_) => break,

            LineKind::Blank | LineKind::Comment | LineKind::TestExpectHeader(_) => {
                *pos += 1;
            }

            LineKind::ScalarField(key, value) => {
                let key = key.clone();
                let value = value.clone();
                let line_number = lines[*pos].number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in header"),
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    *pos += 1;
                    continue;
                }

                match key.as_str() {
                    "agm" => {
                        validate_agm_format(&value, line_number, errors);
                        agm = Some(value);
                    }
                    "package" => {
                        validate_package_format(&value, line_number, errors);
                        package = Some(value);
                    }
                    "version" => {
                        validate_version_format(&value, line_number, errors);
                        version = Some(value);
                    }
                    "title" => title = Some(value),
                    "owner" => owner = Some(value),
                    "default_load" => default_load = Some(value),
                    "description" => description = Some(value),
                    "status" => status = Some(value),
                    "target_runtime" => target_runtime = Some(value),
                    _ => {} // unknown scalar fields are ignored in header
                }
                *pos += 1;
            }

            LineKind::InlineListField(key, items) => {
                let key = key.clone();
                let items = items.clone();
                let line_number = lines[*pos].number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in header"),
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    *pos += 1;
                    continue;
                }

                match key.as_str() {
                    "tags" => tags = Some(items),
                    "imports" => {
                        imports_raw = Some(parse_imports(&items, line_number, errors));
                    }
                    _ => {} // unknown inline list fields ignored
                }
                *pos += 1;
            }

            LineKind::FieldStart(key) => {
                let key = key.clone();
                let line_number = lines[*pos].number;

                if tracker.track(&key) {
                    errors.push(AgmError::new(
                        ErrorCode::P006,
                        format!("Duplicate field `{key}` in header"),
                        ErrorLocation::new(None, Some(line_number), None),
                    ));
                    *pos += 1;
                    skip_field_body(lines, pos);
                    continue;
                }

                *pos += 1; // advance past the FieldStart line

                match key.as_str() {
                    "description" => {
                        description = Some(parse_block(lines, pos));
                    }
                    "tags" => {
                        tags = Some(parse_indented_list(lines, pos));
                    }
                    "imports" => {
                        let raw_items = parse_indented_list(lines, pos);
                        imports_raw = Some(parse_imports(&raw_items, line_number, errors));
                    }
                    "load_profiles" => {
                        load_profiles_parsed = Some(parse_load_profiles(lines, pos, errors));
                    }
                    _ => {
                        skip_field_body(lines, pos);
                    }
                }
            }

            LineKind::BodyMarker(_) => {
                // Unexpected in header — ignore.
                *pos += 1;
            }

            LineKind::ListItem(_) | LineKind::IndentedLine(_) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!(
                        "Unexpected indentation in header at line {}",
                        lines[*pos].number
                    ),
                    ErrorLocation::new(None, Some(lines[*pos].number), None),
                ));
                *pos += 1;
            }
        }
    }

    // Check required fields.
    for field in ["agm", "package", "version"] {
        let is_missing = match field {
            "agm" => agm.is_none(),
            "package" => package.is_none(),
            "version" => version.is_none(),
            _ => false,
        };
        if is_missing {
            errors.push(AgmError::new(
                ErrorCode::P001,
                format!("Missing required header field: '{field}'"),
                ErrorLocation::new(None, None, None),
            ));
        }
    }

    Header {
        agm: agm.unwrap_or_default(),
        package: package.unwrap_or_default(),
        version: version.unwrap_or_default(),
        title,
        owner,
        imports: imports_raw,
        default_load,
        description,
        tags,
        status,
        load_profiles: load_profiles_parsed,
        target_runtime,
    }
}
