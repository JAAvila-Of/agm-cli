//! Markdown-to-AGM compiler (spec S35).
//!
//! Converts structured Markdown documents into valid AGM files using
//! heuristic section extraction, type classification, and field mapping.

pub mod classifier;
pub mod id_generator;
pub mod mapper;
pub mod relation;
pub mod section;

use crate::model::file::{AgmFile, Header};
use crate::model::node::Node;

use self::classifier::NodeTypeClassifier;
use self::id_generator::IdGenerator;
use self::mapper::FieldMapper;
use self::relation::RelationInferrer;
use self::section::extract_sections;

// ---------------------------------------------------------------------------
// CompileOptions
// ---------------------------------------------------------------------------

/// Options controlling compilation behavior.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Package name for the generated AGM file header.
    pub package: String,
    /// Semantic version for the generated AGM file header.
    pub version: String,
    /// Minimum confidence threshold (0.0..=1.0) for including inferred nodes.
    /// Sections below this threshold produce a `LowConfidence` warning and
    /// are excluded from the output. Default: 0.5.
    pub min_confidence: f32,
    /// If true, consecutive sections classified as the same node type are
    /// merged into a single node. Default: false.
    pub merge_same_type: bool,
    /// Optional prefix prepended to all generated node IDs.
    /// Example: `Some("auth")` produces IDs like `auth.login_flow`.
    pub id_prefix: Option<String>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            package: "unnamed.package".to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.5,
            merge_same_type: false,
            id_prefix: None,
        }
    }
}

// ---------------------------------------------------------------------------
// CompileWarning
// ---------------------------------------------------------------------------

/// A warning produced during compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileWarning {
    pub kind: CompileWarningKind,
    pub message: String,
    /// Source line number in the input Markdown (1-indexed), if available.
    pub source_line: Option<usize>,
}

/// Classification of compiler warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileWarningKind {
    /// Section classified with confidence below the threshold.
    LowConfidence,
    /// Multiple node types scored similarly; highest was chosen.
    AmbiguousType,
    /// Section was skipped (e.g., empty body, no heading).
    SkippedSection,
    /// Section heading was present but body was empty.
    EmptySection,
    /// Generated node ID collided with an existing one; suffix appended.
    IdCollision,
}

// ---------------------------------------------------------------------------
// CompileResult
// ---------------------------------------------------------------------------

/// Result of a compilation pass.
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// The generated AGM file.
    pub file: AgmFile,
    /// Warnings about low-confidence inferences, skipped sections, etc.
    pub warnings: Vec<CompileWarning>,
}

// ---------------------------------------------------------------------------
// compile()
// ---------------------------------------------------------------------------

/// Compiles a Markdown document into an AGM file.
///
/// This is the primary entry point for the compiler. It:
/// 1. Extracts heading-delimited sections from the Markdown text.
/// 2. Classifies each section into an AGM node type with a confidence score.
/// 3. Generates unique, valid node IDs from heading text.
/// 4. Maps section content (paragraphs, lists, code blocks) to node fields.
/// 5. Infers cross-references and dependencies between nodes.
/// 6. Assembles the final `AgmFile` with header and nodes.
///
/// Sections with confidence below `options.min_confidence` are excluded
/// and reported as warnings.
#[must_use]
pub fn compile(markdown: &str, options: &CompileOptions) -> CompileResult {
    let mut warnings = Vec::new();

    // Stage 1: Extract sections
    let sections = extract_sections(markdown);

    if sections.is_empty() {
        warnings.push(CompileWarning {
            kind: CompileWarningKind::SkippedSection,
            message: "No headings found in input; entire body treated as a single section"
                .to_owned(),
            source_line: Some(1),
        });
        // Fallback: treat entire document as one section
        // (handled inside extract_sections when no headings are found)
    }

    // Stage 2 & 3: Classify and generate IDs
    let classifier = NodeTypeClassifier::new();
    let mut id_gen = IdGenerator::new(options.id_prefix.as_deref());
    let field_mapper = FieldMapper::new();

    let mut nodes: Vec<Node> = Vec::new();

    for section in &sections {
        // Skip empty sections
        if section.body_text.trim().is_empty()
            && section.list_items.is_empty()
            && section.code_blocks.is_empty()
        {
            warnings.push(CompileWarning {
                kind: CompileWarningKind::EmptySection,
                message: format!("Empty section: '{}'", section.heading),
                source_line: Some(section.source_line_start),
            });
            continue;
        }

        // Classify
        let (node_type, confidence, alt_type) = classifier.classify(section);

        if confidence < options.min_confidence {
            warnings.push(CompileWarning {
                kind: CompileWarningKind::LowConfidence,
                message: format!(
                    "Section '{}' classified as {} with confidence {:.2} (below threshold {:.2})",
                    section.heading, node_type, confidence, options.min_confidence
                ),
                source_line: Some(section.source_line_start),
            });
            continue;
        }

        if let Some(ref alt) = alt_type {
            warnings.push(CompileWarning {
                kind: CompileWarningKind::AmbiguousType,
                message: format!(
                    "Section '{}' could be {} or {}; chose {} (confidence {:.2})",
                    section.heading, node_type, alt, node_type, confidence
                ),
                source_line: Some(section.source_line_start),
            });
        }

        // Generate ID
        let (id, collision) = id_gen.generate(&section.heading);
        if collision {
            warnings.push(CompileWarning {
                kind: CompileWarningKind::IdCollision,
                message: format!(
                    "ID collision for heading '{}'; using '{}'",
                    section.heading, id
                ),
                source_line: Some(section.source_line_start),
            });
        }

        // Map fields
        let node = field_mapper.map_to_node(section, node_type, &id);
        nodes.push(node);
    }

    // Merge consecutive same-type nodes if requested
    if options.merge_same_type {
        nodes = merge_consecutive_same_type(nodes);
    }

    // Stage 5: Infer relations
    RelationInferrer::infer(&mut nodes);

    // Stage 6: Assemble AgmFile
    let header = Header {
        agm: "1.0".to_owned(),
        package: options.package.clone(),
        version: options.version.clone(),
        title: None,
        owner: None,
        imports: None,
        default_load: None,
        description: None,
        tags: None,
        status: None,
        load_profiles: None,
        target_runtime: None,
    };

    let file = AgmFile { header, nodes };

    CompileResult { file, warnings }
}

/// Merges consecutive nodes of the same type into a single node.
/// The first node's ID and summary are kept; items/steps/detail from
/// subsequent nodes are appended.
fn merge_consecutive_same_type(nodes: Vec<Node>) -> Vec<Node> {
    if nodes.is_empty() {
        return nodes;
    }

    let mut merged: Vec<Node> = Vec::new();
    let mut iter = nodes.into_iter();
    let mut current = iter.next().unwrap();

    for next in iter {
        if next.node_type == current.node_type {
            // Merge items
            if let Some(next_items) = next.items {
                current
                    .items
                    .get_or_insert_with(Vec::new)
                    .extend(next_items);
            }
            // Merge steps
            if let Some(next_steps) = next.steps {
                current
                    .steps
                    .get_or_insert_with(Vec::new)
                    .extend(next_steps);
            }
            // Merge detail
            if let Some(next_detail) = next.detail {
                let existing = current.detail.get_or_insert_with(String::new);
                if !existing.is_empty() {
                    existing.push_str("\n\n");
                }
                existing.push_str(&next_detail);
            }
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);

    merged
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_empty_input_returns_warning() {
        let result = compile("", &CompileOptions::default());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_compile_single_heading_with_body_returns_one_node() {
        let md = "## Login Flow\n\n1. Resolve tenant.\n2. Redirect.\n";
        let opts = CompileOptions {
            package: "auth.platform".to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0, // accept everything
            ..Default::default()
        };
        let result = compile(md, &opts);
        assert_eq!(result.file.nodes.len(), 1);
        assert_eq!(result.file.header.package, "auth.platform");
        assert_eq!(result.file.header.agm, "1.0");
    }

    #[test]
    fn test_compile_spec_s35_4_example_produces_two_nodes() {
        let md = "\
## Login Constraints

- Access tokens must never be exposed to the browser.
- Sensitive calls must originate from the server.

## Login Flow

1. Resolve tenant by host.
2. Redirect to the provider.
3. Validate callback.
4. Create a server-side session.
";
        let opts = CompileOptions {
            package: "auth.platform".to_owned(),
            version: "0.1.0".to_owned(),
            min_confidence: 0.0,
            ..Default::default()
        };
        let result = compile(md, &opts);
        assert_eq!(result.file.nodes.len(), 2);
    }

    #[test]
    fn test_compile_low_confidence_section_excluded_with_warning() {
        let md = "## Some Random Heading\n\nJust some text.\n";
        let opts = CompileOptions {
            min_confidence: 0.99, // very high threshold
            ..Default::default()
        };
        let result = compile(md, &opts);
        assert!(result.file.nodes.is_empty());
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.kind == CompileWarningKind::LowConfidence)
        );
    }

    #[test]
    fn test_compile_header_uses_options() {
        let md = "## Test\n\nSome content.\n";
        let opts = CompileOptions {
            package: "my.pkg".to_owned(),
            version: "2.0.0".to_owned(),
            min_confidence: 0.0,
            ..Default::default()
        };
        let result = compile(md, &opts);
        assert_eq!(result.file.header.package, "my.pkg");
        assert_eq!(result.file.header.version, "2.0.0");
    }
}
