//! Import resolution for AGM packages (spec S10).
//!
//! Provides version constraint validation, filesystem-based package resolution,
//! cross-package reference resolution, and circular import detection.

pub mod constraint;
pub mod resolver;

pub use constraint::{
    ValidatedImport, parse_version_constraint, validate_all_imports, validate_import,
};
pub use resolver::{FileSystemResolver, ImportResolver, ResolvedPackage};

use std::collections::HashSet;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::node::Node;

// ---------------------------------------------------------------------------
// qualify_node_id
// ---------------------------------------------------------------------------

/// Produces a fully qualified node ID: `"{package}.{node_id}"`.
///
/// Example: `qualify_node_id("shared.security", "auth.rules")` -> `"shared.security.auth.rules"`
#[must_use]
pub fn qualify_node_id(package: &str, node_id: &str) -> String {
    format!("{package}.{node_id}")
}

// ---------------------------------------------------------------------------
// resolve_cross_package_ref
// ---------------------------------------------------------------------------

/// Resolves a cross-package reference to the target node.
///
/// Given a fully qualified reference like `"shared.security.auth.rules"` and a
/// list of resolved packages, this function:
/// 1. Identifies which imported package the reference belongs to (longest prefix match).
/// 2. Strips the package prefix to get the local node ID.
/// 3. Searches the imported package's nodes for that ID.
///
/// Returns `Err(I004)` if the reference cannot be resolved.
///
/// `imported_packages` is the list of all successfully resolved packages.
pub fn resolve_cross_package_ref<'a>(
    ref_id: &str,
    imported_packages: &'a [ResolvedPackage],
) -> Result<&'a Node, AgmError> {
    // Step 1: Collect known package names, sorted by length descending
    //         (longest prefix match wins)
    let mut package_names: Vec<&str> = imported_packages
        .iter()
        .map(|p| p.package.as_str())
        .collect();
    package_names.sort_by_key(|b| std::cmp::Reverse(b.len()));

    // Step 2: Find which package the ref_id belongs to
    for pkg_name in &package_names {
        // The ref must start with "{pkg_name}." to be a cross-package ref
        if ref_id.starts_with(pkg_name) && ref_id[pkg_name.len()..].starts_with('.') {
            let local_node_id = &ref_id[pkg_name.len() + 1..]; // strip "pkg_name."

            // Step 3: Find the ResolvedPackage
            let resolved = imported_packages
                .iter()
                .find(|p| p.package == *pkg_name)
                .unwrap(); // safe: pkg_name came from this list

            // Step 4: Find the node in the resolved package
            for node in &resolved.file.nodes {
                if node.id == local_node_id {
                    return Ok(node);
                }
            }

            // Package found but node not found
            return Err(AgmError::new(
                ErrorCode::I004,
                format!("Cross-package reference to non-existent node: `{ref_id}`"),
                ErrorLocation::default(),
            ));
        }
    }

    // No imported package prefix matched
    Err(AgmError::new(
        ErrorCode::I004,
        format!("Cross-package reference to non-existent node: `{ref_id}`"),
        ErrorLocation::default(),
    ))
}

// ---------------------------------------------------------------------------
// detect_circular_imports
// ---------------------------------------------------------------------------

/// Detects circular imports starting from the root package.
///
/// Performs a DFS traversal of the import graph. For each package encountered,
/// its header is parsed to discover its own imports, which are then recursively
/// checked.
///
/// Returns `Err(I003)` with the cycle path if a cycle is found.
/// Returns `Ok(())` if no cycles exist.
///
/// `root_package` is the name of the top-level package being validated.
/// `root_imports` is the list of that package's validated imports.
/// `resolver` is used to resolve each package and inspect its imports.
pub fn detect_circular_imports(
    root_package: &str,
    root_imports: &[ValidatedImport],
    resolver: &dyn ImportResolver,
) -> Result<(), AgmError> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut in_stack: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = Vec::new();

    dfs(
        root_package,
        root_imports,
        resolver,
        &mut visited,
        &mut in_stack,
        &mut path,
    )
}

fn dfs(
    package: &str,
    imports: &[ValidatedImport],
    resolver: &dyn ImportResolver,
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Result<(), AgmError> {
    if in_stack.contains(package) {
        // Found a cycle — build the cycle path string
        let cycle_start = path.iter().position(|p| p == package).unwrap();
        let cycle_path = path[cycle_start..]
            .iter()
            .chain(std::iter::once(&package.to_owned()))
            .cloned()
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(AgmError::new(
            ErrorCode::I003,
            format!("Circular import detected: `{cycle_path}`"),
            ErrorLocation::default(),
        ));
    }

    if visited.contains(package) {
        return Ok(()); // already fully explored, no cycle through here
    }

    visited.insert(package.to_owned());
    in_stack.insert(package.to_owned());
    path.push(package.to_owned());

    for import in imports {
        let dep_package = import.package().to_owned();

        // Try to resolve the dependency to get its imports
        match resolver.resolve(import) {
            Ok(resolved) => {
                // Extract the resolved package's own imports
                let dep_imports = match &resolved.file.header.imports {
                    Some(entries) => {
                        let (validated, _errors) = validate_all_imports(entries);
                        validated
                    }
                    None => vec![],
                };

                // Recurse into the dependency
                dfs(
                    &dep_package,
                    &dep_imports,
                    resolver,
                    visited,
                    in_stack,
                    path,
                )?;
            }
            Err(_) => {
                // If resolution fails (I001/I002), skip this edge.
                // The resolver error will be reported separately.
                continue;
            }
        }
    }

    path.pop();
    in_stack.remove(package);
    Ok(())
}

// ---------------------------------------------------------------------------
// check_deprecated
// ---------------------------------------------------------------------------

/// Checks if a resolved package is deprecated and returns an I005 warning if so.
pub fn check_deprecated(resolved: &ResolvedPackage) -> Option<AgmError> {
    if resolved.file.header.status.as_deref() == Some("deprecated") {
        Some(AgmError::new(
            ErrorCode::I005,
            format!("Import `{}` is deprecated", resolved.package),
            ErrorLocation::default(),
        ))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::error::ErrorCode;
    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::imports::ImportEntry;
    use crate::model::node::Node;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_agm_file(package: &str, version: &str, nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: package.to_owned(),
                version: version.to_owned(),
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
            nodes,
        }
    }

    fn make_agm_file_with_imports(
        package: &str,
        version: &str,
        nodes: Vec<Node>,
        imports: Vec<ImportEntry>,
    ) -> AgmFile {
        AgmFile {
            header: Header {
                agm: "1.0".to_owned(),
                package: package.to_owned(),
                version: version.to_owned(),
                title: None,
                owner: None,
                imports: Some(imports),
                default_load: None,
                description: None,
                tags: None,
                status: None,
                load_profiles: None,
                target_runtime: None,
            },
            nodes,
        }
    }

    fn make_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: format!("Test node {id}"),
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
            span: Span::new(1, 1),
        }
    }

    fn make_resolved(package: &str, version: &str, nodes: Vec<Node>) -> ResolvedPackage {
        ResolvedPackage {
            package: package.to_owned(),
            version: semver::Version::parse(version).unwrap(),
            path: PathBuf::from(format!(".agm/packages/{package}/pkg.agm")),
            file: make_agm_file(package, version, nodes),
        }
    }

    fn make_resolved_with_imports(
        package: &str,
        version: &str,
        nodes: Vec<Node>,
        imports: Vec<ImportEntry>,
    ) -> ResolvedPackage {
        ResolvedPackage {
            package: package.to_owned(),
            version: semver::Version::parse(version).unwrap(),
            path: PathBuf::from(format!(".agm/packages/{package}/pkg.agm")),
            file: make_agm_file_with_imports(package, version, nodes, imports),
        }
    }

    // -----------------------------------------------------------------------
    // MockResolver for circular import tests
    // -----------------------------------------------------------------------

    struct MockResolver {
        packages: HashMap<String, ResolvedPackage>,
    }

    impl MockResolver {
        fn new() -> Self {
            Self {
                packages: HashMap::new(),
            }
        }

        fn add(&mut self, package: ResolvedPackage) {
            self.packages.insert(package.package.clone(), package);
        }
    }

    impl ImportResolver for MockResolver {
        fn resolve(&self, import: &ValidatedImport) -> Result<ResolvedPackage, AgmError> {
            self.packages.get(import.package()).cloned().ok_or_else(|| {
                AgmError::new(
                    ErrorCode::I001,
                    format!("Unresolved import: `{}`", import.package()),
                    ErrorLocation::default(),
                )
            })
        }
    }

    // -----------------------------------------------------------------------
    // Category D: Namespace qualification
    // -----------------------------------------------------------------------

    #[test]
    fn test_qualify_node_id_produces_dotted_id() {
        assert_eq!(
            qualify_node_id("shared.security", "auth.rules"),
            "shared.security.auth.rules"
        );
    }

    #[test]
    fn test_qualify_node_id_simple_names() {
        assert_eq!(qualify_node_id("core", "setup"), "core.setup");
    }

    // -----------------------------------------------------------------------
    // Category E: Cross-package reference resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_cross_package_ref_finds_node() {
        let node = make_node("auth.rules");
        let packages = vec![make_resolved("shared.security", "1.0.0", vec![node])];
        let result = resolve_cross_package_ref("shared.security.auth.rules", &packages).unwrap();
        assert_eq!(result.id, "auth.rules");
    }

    #[test]
    fn test_resolve_cross_package_ref_nonexistent_node_returns_i004() {
        let node = make_node("auth.rules");
        let packages = vec![make_resolved("shared.security", "1.0.0", vec![node])];
        let err = resolve_cross_package_ref("shared.security.nonexistent", &packages).unwrap_err();
        assert_eq!(err.code, ErrorCode::I004);
    }

    #[test]
    fn test_resolve_cross_package_ref_no_matching_package_returns_i004() {
        let node = make_node("auth.rules");
        let packages = vec![make_resolved("shared.security", "1.0.0", vec![node])];
        let err = resolve_cross_package_ref("unknown.pkg.node", &packages).unwrap_err();
        assert_eq!(err.code, ErrorCode::I004);
    }

    #[test]
    fn test_resolve_cross_package_ref_longest_prefix_wins() {
        // Packages: "shared" and "shared.security"
        // Reference: "shared.security.auth.rules" should resolve in "shared.security"
        let shared_node = make_node("security.auth.rules"); // would match if "shared" pkg was chosen
        let security_node = make_node("auth.rules"); // correct node in "shared.security"
        let packages = vec![
            make_resolved("shared", "1.0.0", vec![shared_node]),
            make_resolved("shared.security", "1.0.0", vec![security_node]),
        ];
        let result = resolve_cross_package_ref("shared.security.auth.rules", &packages).unwrap();
        assert_eq!(result.id, "auth.rules");
    }

    // -----------------------------------------------------------------------
    // Category F: Circular import detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_circular_imports_no_cycle_returns_ok() {
        // A -> B, B -> C, no cycle
        let entry_b = ImportEntry::new("B".to_owned(), None);
        let entry_c = ImportEntry::new("C".to_owned(), None);

        let pkg_b = make_resolved_with_imports("B", "1.0.0", vec![], vec![entry_c]);
        let pkg_c = make_resolved("C", "1.0.0", vec![]);

        let mut mock = MockResolver::new();
        mock.add(pkg_b);
        mock.add(pkg_c);

        let root_imports = vec![validate_import(&entry_b).unwrap()];
        let result = detect_circular_imports("A", &root_imports, &mock);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_circular_imports_direct_cycle_returns_i003() {
        // A -> B, B -> A
        let entry_a = ImportEntry::new("A".to_owned(), None);
        let entry_b = ImportEntry::new("B".to_owned(), None);

        // B imports A; A (as a resolved package) imports B (to complete the cycle)
        let pkg_b = make_resolved_with_imports("B", "1.0.0", vec![], vec![entry_a]);
        let pkg_a = make_resolved_with_imports("A", "1.0.0", vec![], vec![entry_b.clone()]);

        let mut mock = MockResolver::new();
        mock.add(pkg_b);
        mock.add(pkg_a);

        let root_imports = vec![validate_import(&entry_b).unwrap()];
        let err = detect_circular_imports("A", &root_imports, &mock).unwrap_err();
        assert_eq!(err.code, ErrorCode::I003);
        assert!(
            err.message.contains("A -> B -> A"),
            "message: {}",
            err.message
        );
    }

    #[test]
    fn test_detect_circular_imports_transitive_cycle_returns_i003() {
        // A -> B -> C -> A
        let entry_a = ImportEntry::new("A".to_owned(), None);
        let entry_b = ImportEntry::new("B".to_owned(), None);
        let entry_c = ImportEntry::new("C".to_owned(), None);

        // B imports C; C imports A; A (as a resolved package) imports B (to complete the cycle)
        let pkg_b = make_resolved_with_imports("B", "1.0.0", vec![], vec![entry_c]);
        let pkg_c = make_resolved_with_imports("C", "1.0.0", vec![], vec![entry_a]);
        let pkg_a = make_resolved_with_imports("A", "1.0.0", vec![], vec![entry_b.clone()]);

        let mut mock = MockResolver::new();
        mock.add(pkg_b);
        mock.add(pkg_c);
        mock.add(pkg_a);

        let root_imports = vec![validate_import(&entry_b).unwrap()];
        let err = detect_circular_imports("A", &root_imports, &mock).unwrap_err();
        assert_eq!(err.code, ErrorCode::I003);
        assert!(
            err.message.contains("A -> B -> C -> A"),
            "message: {}",
            err.message
        );
    }

    #[test]
    fn test_detect_circular_imports_diamond_no_cycle_returns_ok() {
        // A -> B, A -> C, B -> D, C -> D (diamond, not a cycle)
        let entry_b = ImportEntry::new("B".to_owned(), None);
        let entry_c = ImportEntry::new("C".to_owned(), None);
        let entry_d = ImportEntry::new("D".to_owned(), None);

        let pkg_b = make_resolved_with_imports("B", "1.0.0", vec![], vec![entry_d.clone()]);
        let pkg_c = make_resolved_with_imports("C", "1.0.0", vec![], vec![entry_d]);
        let pkg_d = make_resolved("D", "1.0.0", vec![]);

        let mut mock = MockResolver::new();
        mock.add(pkg_b);
        mock.add(pkg_c);
        mock.add(pkg_d);

        let root_imports = vec![
            validate_import(&entry_b).unwrap(),
            validate_import(&entry_c).unwrap(),
        ];
        let result = detect_circular_imports("A", &root_imports, &mock);
        assert!(result.is_ok());
    }
}
