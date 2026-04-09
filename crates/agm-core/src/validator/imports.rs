//! Import validation (spec S10).
//!
//! Pass 6 (cross-package): validates imports, resolves packages, checks cross-package
//! references. Only runs when an `ImportResolver` is provided.

use std::collections::HashSet;

use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::import::ImportResolver;
use crate::model::file::AgmFile;
use crate::model::node::Node;

/// Collects all node ID references from all relationship fields and agent_context.
fn collect_all_refs(node: &Node) -> Vec<String> {
    let mut refs = Vec::new();

    let push_from = |v: &Option<Vec<String>>, out: &mut Vec<String>| {
        if let Some(list) = v {
            out.extend(list.iter().cloned());
        }
    };

    push_from(&node.depends, &mut refs);
    push_from(&node.related_to, &mut refs);
    push_from(&node.replaces, &mut refs);
    push_from(&node.conflicts, &mut refs);
    push_from(&node.see_also, &mut refs);

    // Also check agent_context.load_nodes
    if let Some(ref ctx) = node.agent_context {
        push_from(&ctx.load_nodes, &mut refs);
    }

    refs
}

/// Validates imports: resolves packages, checks version constraints, detects
/// circular imports, validates cross-package references.
///
/// Rules: I001 (unresolved import), I002 (version constraint not satisfied),
/// I003 (circular import), I004 (cross-package ref to non-existent node),
/// I005 (deprecated import).
#[must_use]
pub fn validate_imports(
    file: &AgmFile,
    resolver: &dyn ImportResolver,
    file_name: &str,
) -> Vec<AgmError> {
    let mut errors = Vec::new();

    let import_entries = match &file.header.imports {
        Some(entries) => entries,
        None => return errors,
    };

    // Step 1: Validate all import entries (parse version constraints)
    let (validated, constraint_errors) = crate::import::validate_all_imports(import_entries);
    errors.extend(constraint_errors);

    // Step 2: Resolve each validated import
    let mut resolved_packages = Vec::new();
    for validated_import in &validated {
        match resolver.resolve(validated_import) {
            Ok(resolved) => {
                // I005 — check if the package is deprecated
                if let Some(warn) = crate::import::check_deprecated(&resolved) {
                    errors.push(warn);
                }
                resolved_packages.push(resolved);
            }
            Err(e) => errors.push(e), // I001 or I002
        }
    }

    // Step 3: Circular import detection (I003)
    if let Err(e) =
        crate::import::detect_circular_imports(&file.header.package, &validated, resolver)
    {
        errors.push(e);
    }

    // Step 4: Cross-package reference validation (I004)
    let local_ids: HashSet<String> = file.nodes.iter().map(|n| n.id.clone()).collect();

    for node in &file.nodes {
        let all_refs = collect_all_refs(node);
        for ref_id in all_refs {
            // Skip refs that resolve locally
            if local_ids.contains(&ref_id) {
                continue;
            }

            // Only attempt cross-package resolution for refs that look like
            // they belong to an imported package (start with a known package prefix)
            let is_cross_package = import_entries.iter().any(|imp| {
                ref_id.starts_with(&imp.package) && ref_id[imp.package.len()..].starts_with('.')
            });

            if !is_cross_package {
                continue; // unresolved local refs handled by references.rs
            }

            if let Err(mut e) =
                crate::import::resolve_cross_package_ref(&ref_id, &resolved_packages)
            {
                e.location = ErrorLocation::full(file_name, node.span.start_line, &node.id);
                errors.push(e);
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::error::codes::ErrorCode;
    use crate::import::{ImportResolver, ResolvedPackage, ValidatedImport};
    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::imports::ImportEntry;
    use crate::model::node::Node;

    // ---------------------------------------------------------------------------
    // Mock resolver helpers
    // ---------------------------------------------------------------------------

    struct AlwaysFailResolver;

    impl ImportResolver for AlwaysFailResolver {
        fn resolve(
            &self,
            import: &ValidatedImport,
        ) -> Result<ResolvedPackage, crate::error::diagnostic::AgmError> {
            Err(crate::error::diagnostic::AgmError::new(
                ErrorCode::I001,
                format!("Unresolved import: `{}`", import.package()),
                ErrorLocation::default(),
            ))
        }
    }

    struct AlwaysSucceedResolver {
        deprecated: bool,
    }

    impl ImportResolver for AlwaysSucceedResolver {
        fn resolve(
            &self,
            import: &ValidatedImport,
        ) -> Result<ResolvedPackage, crate::error::diagnostic::AgmError> {
            let header = Header {
                agm: "1.0".to_owned(),
                package: import.package().to_owned(),
                version: "1.0.0".to_owned(),
                title: None,
                owner: None,
                imports: None,
                default_load: None,
                description: None,
                tags: None,
                status: if self.deprecated {
                    Some("deprecated".to_owned())
                } else {
                    None
                },
                load_profiles: None,
                target_runtime: None,
            };
            Ok(ResolvedPackage {
                package: import.package().to_owned(),
                version: semver::Version::new(1, 0, 0),
                path: PathBuf::from("fake/path"),
                file: AgmFile {
                    header,
                    nodes: vec![],
                },
            })
        }
    }

    fn make_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: "a node".to_owned(),
            priority: None,
            stability: None,
            confidence: None,
            status: None,
            depends: None,
            related_to: None,
            replaces: None,
            conflicts: None,
            see_also: None,
            items: None,
            steps: None,
            fields: None,
            input: None,
            output: None,
            detail: None,
            rationale: None,
            tradeoffs: None,
            resolution: None,
            examples: None,
            notes: None,
            code: None,
            code_blocks: None,
            verify: None,
            agent_context: None,
            target: None,
            execution_status: None,
            executed_by: None,
            executed_at: None,
            execution_log: None,
            retry_count: None,
            parallel_groups: None,
            memory: None,
            scope: None,
            applies_when: None,
            valid_from: None,
            valid_until: None,
            tags: None,
            aliases: None,
            keywords: None,
            extra_fields: BTreeMap::new(),
            span: Span::new(5, 7),
        }
    }

    fn make_file_with_import(pkg: &str) -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: "myapp".to_owned(),
                version: "0.1.0".to_owned(),
                title: None,
                owner: None,
                imports: Some(vec![ImportEntry {
                    package: pkg.to_owned(),
                    version_constraint: None,
                }]),
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes: vec![make_node("local.node")],
        }
    }

    #[test]
    fn test_validate_imports_no_imports_returns_empty() {
        let file = AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: "myapp".to_owned(),
                version: "0.1.0".to_owned(),
                title: None,
                owner: None,
                imports: None,
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes: vec![make_node("local.node")],
        };
        let resolver = AlwaysFailResolver;
        let errors = validate_imports(&file, &resolver, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_imports_unresolved_returns_i001() {
        let file = make_file_with_import("shared.missing");
        let resolver = AlwaysFailResolver;
        let errors = validate_imports(&file, &resolver, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::I001));
    }

    #[test]
    fn test_validate_imports_resolved_returns_empty() {
        let file = make_file_with_import("shared.security");
        let resolver = AlwaysSucceedResolver { deprecated: false };
        let errors = validate_imports(&file, &resolver, "test.agm");
        assert!(!errors.iter().any(|e| e.code == ErrorCode::I001));
    }

    #[test]
    fn test_validate_imports_deprecated_returns_i005() {
        let file = make_file_with_import("shared.old");
        let resolver = AlwaysSucceedResolver { deprecated: true };
        let errors = validate_imports(&file, &resolver, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::I005));
    }

    #[test]
    fn test_validate_imports_version_mismatch_returns_i002() {
        // Use a resolver that returns I002 error
        struct VersionMismatchResolver;
        impl ImportResolver for VersionMismatchResolver {
            fn resolve(
                &self,
                import: &ValidatedImport,
            ) -> Result<ResolvedPackage, crate::error::diagnostic::AgmError> {
                Err(crate::error::diagnostic::AgmError::new(
                    ErrorCode::I002,
                    format!(
                        "Import version constraint not satisfied: `{}`",
                        import.package()
                    ),
                    ErrorLocation::default(),
                ))
            }
        }

        let file = make_file_with_import("shared.security");
        let resolver = VersionMismatchResolver;
        let errors = validate_imports(&file, &resolver, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::I002));
    }
}
