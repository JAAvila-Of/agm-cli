//! Import resolver trait and filesystem implementation (spec S10.3).

use std::path::PathBuf;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::file::AgmFile;
use crate::parser;

use super::constraint::ValidatedImport;

// ---------------------------------------------------------------------------
// ResolvedPackage
// ---------------------------------------------------------------------------

/// A successfully resolved import: the package name, its version, its source
/// path, and the parsed AGM file contents.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPackage {
    /// Package name (matches the `package` field in the header).
    pub package: String,
    /// The resolved version of the package.
    pub version: semver::Version,
    /// Path to the `.agm` file that was resolved.
    pub path: PathBuf,
    /// The parsed AGM file.
    pub file: AgmFile,
}

// ---------------------------------------------------------------------------
// ImportResolver trait
// ---------------------------------------------------------------------------

/// Trait for resolving AGM package imports to their on-disk (or registry) representations.
pub trait ImportResolver {
    /// Resolve a validated import to a package.
    ///
    /// Returns `Err` with I001 if the package cannot be found, or I002 if the
    /// package is found but no version satisfies the constraint.
    fn resolve(&self, import: &ValidatedImport) -> Result<ResolvedPackage, AgmError>;
}

// ---------------------------------------------------------------------------
// FileSystemResolver
// ---------------------------------------------------------------------------

/// Resolves imports by searching directories on the local filesystem.
///
/// Search paths are checked in order. Within each search path, a directory
/// matching the package name is expected. Inside that directory, `.agm` files
/// are parsed to extract their version. The highest version satisfying the
/// constraint is selected.
#[derive(Debug, Clone)]
pub struct FileSystemResolver {
    /// Ordered list of directories to search (e.g., [".agm/packages/"]).
    search_paths: Vec<PathBuf>,
}

impl FileSystemResolver {
    /// Creates a new FileSystemResolver with the given search paths.
    #[must_use]
    pub fn new(search_paths: Vec<PathBuf>) -> Self {
        Self { search_paths }
    }

    /// Convenience constructor for a single search path.
    #[must_use]
    pub fn single(path: impl Into<PathBuf>) -> Self {
        Self {
            search_paths: vec![path.into()],
        }
    }

    /// Finds all candidate `.agm` files in the package directory across
    /// all search paths. Returns (path, parsed_file) pairs.
    ///
    /// This is an internal helper; it does NOT filter by version.
    fn find_candidates(&self, package_name: &str) -> Result<Vec<(PathBuf, AgmFile)>, AgmError> {
        let mut candidates = Vec::new();

        for search_path in &self.search_paths {
            let package_dir = search_path.join(package_name);

            if !package_dir.is_dir() {
                continue;
            }

            let dir_entries = std::fs::read_dir(&package_dir).map_err(|e| {
                AgmError::new(
                    ErrorCode::I001,
                    format!(
                        "Failed to read package directory `{}`: {e}",
                        package_dir.display()
                    ),
                    ErrorLocation::default(),
                )
            })?;

            for dir_entry in dir_entries {
                let entry = dir_entry.map_err(|e| {
                    AgmError::new(
                        ErrorCode::I001,
                        format!("Failed to read directory entry: {e}"),
                        ErrorLocation::default(),
                    )
                })?;
                let path = entry.path();

                if path.extension().is_some_and(|ext| ext == "agm") {
                    let source = std::fs::read_to_string(&path).map_err(|e| {
                        AgmError::new(
                            ErrorCode::I001,
                            format!("Failed to read package file `{}`: {e}", path.display()),
                            ErrorLocation::default(),
                        )
                    })?;

                    // Skip files with parse errors (they are invalid packages)
                    let file = match parser::parse(&source) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };

                    // Verify the package field matches
                    if file.header.package == package_name {
                        candidates.push((path, file));
                    }
                }
            }
        }

        Ok(candidates)
    }

    /// From a list of candidates, selects the one whose version best
    /// matches the constraint. Returns the highest matching version.
    fn select_best_match(
        candidates: &[(PathBuf, AgmFile)],
        import: &ValidatedImport,
    ) -> Option<(PathBuf, AgmFile, semver::Version)> {
        let mut best: Option<(PathBuf, AgmFile, semver::Version)> = None;

        for (path, file) in candidates {
            // Parse the version from the file header
            let version = match semver::Version::parse(&file.header.version) {
                Ok(v) => v,
                Err(_) => continue, // skip files with invalid versions
            };

            // Check if version satisfies the constraint
            if !import.matches_version(&version) {
                continue;
            }

            // Keep the highest matching version
            match &best {
                None => best = Some((path.clone(), file.clone(), version)),
                Some((_, _, best_version)) => {
                    if version > *best_version {
                        best = Some((path.clone(), file.clone(), version));
                    }
                }
            }
        }

        best
    }
}

impl ImportResolver for FileSystemResolver {
    fn resolve(&self, import: &ValidatedImport) -> Result<ResolvedPackage, AgmError> {
        let package_name = import.package();

        let candidates = self.find_candidates(package_name)?;

        if candidates.is_empty() {
            return Err(AgmError::new(
                ErrorCode::I001,
                format!("Unresolved import: `{package_name}`"),
                ErrorLocation::default(),
            ));
        }

        match Self::select_best_match(&candidates, import) {
            Some((path, file, version)) => Ok(ResolvedPackage {
                package: package_name.to_owned(),
                version,
                path,
                file,
            }),
            None => {
                // Candidates exist but none satisfy the version constraint
                let found_versions: Vec<String> = candidates
                    .iter()
                    .filter_map(|(_, f)| semver::Version::parse(&f.header.version).ok())
                    .map(|v| v.to_string())
                    .collect();

                Err(AgmError::new(
                    ErrorCode::I002,
                    format!(
                        "Import version constraint not satisfied: `{pkg}@{constraint}` (found {found})",
                        pkg = package_name,
                        constraint = import.entry.version_constraint.as_deref().unwrap_or("*"),
                        found = if found_versions.is_empty() {
                            "no valid versions".to_owned()
                        } else {
                            found_versions.join(", ")
                        },
                    ),
                    ErrorLocation::default(),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::model::imports::ImportEntry;

    use constraint::validate_import;

    use super::super::constraint;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn write_agm_file(dir: &Path, filename: &str, package: &str, version: &str) {
        let content = format!(
            "agm: 1.0\npackage: {package}\nversion: {version}\n\nnode {package}.example\ntype: facts\nsummary: Example node\n"
        );
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn setup_package_dir(base: &Path, package_name: &str, versions: &[&str]) -> PathBuf {
        let pkg_dir = base.join(package_name);
        std::fs::create_dir_all(&pkg_dir).unwrap();
        for (i, version) in versions.iter().enumerate() {
            write_agm_file(&pkg_dir, &format!("v{i}.agm"), package_name, version);
        }
        pkg_dir
    }

    // -----------------------------------------------------------------------
    // Category G: FileSystemResolver
    // -----------------------------------------------------------------------

    #[test]
    fn test_fs_resolver_finds_package_in_search_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        setup_package_dir(tmp.path(), "shared.security", &["1.0.0"]);

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("shared.security".to_owned(), None);
        let import = validate_import(&entry).unwrap();

        let result = resolver.resolve(&import).unwrap();
        assert_eq!(result.package, "shared.security");
        assert_eq!(result.version, semver::Version::parse("1.0.0").unwrap());
    }

    #[test]
    fn test_fs_resolver_missing_package_returns_i001() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No package directory created

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("shared.security".to_owned(), None);
        let import = validate_import(&entry).unwrap();

        let err = resolver.resolve(&import).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::I001);
    }

    #[test]
    fn test_fs_resolver_version_mismatch_returns_i002() {
        let tmp = tempfile::TempDir::new().unwrap();
        setup_package_dir(tmp.path(), "shared.security", &["3.0.0"]);

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned()));
        let import = validate_import(&entry).unwrap();

        let err = resolver.resolve(&import).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::I002);
    }

    #[test]
    fn test_fs_resolver_selects_highest_matching_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        setup_package_dir(tmp.path(), "shared.security", &["1.0.0", "1.2.0"]);

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned()));
        let import = validate_import(&entry).unwrap();

        let result = resolver.resolve(&import).unwrap();
        assert_eq!(result.version, semver::Version::parse("1.2.0").unwrap());
    }

    #[test]
    fn test_fs_resolver_skips_files_with_parse_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pkg_dir = tmp.path().join("shared.http");
        std::fs::create_dir_all(&pkg_dir).unwrap();

        // Write a valid file
        write_agm_file(&pkg_dir, "valid.agm", "shared.http", "1.0.0");

        // Write a malformed file
        std::fs::write(pkg_dir.join("bad.agm"), "this is not valid agm content\n").unwrap();

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("shared.http".to_owned(), None);
        let import = validate_import(&entry).unwrap();

        let result = resolver.resolve(&import).unwrap();
        assert_eq!(result.package, "shared.http");
        assert_eq!(result.version, semver::Version::parse("1.0.0").unwrap());
    }

    #[test]
    fn test_fs_resolver_no_constraint_picks_highest() {
        let tmp = tempfile::TempDir::new().unwrap();
        setup_package_dir(tmp.path(), "core.utils", &["1.0.0", "2.5.0"]);

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());
        let entry = ImportEntry::new("core.utils".to_owned(), None);
        let import = validate_import(&entry).unwrap();

        let result = resolver.resolve(&import).unwrap();
        assert_eq!(result.version, semver::Version::parse("2.5.0").unwrap());
    }

    // -----------------------------------------------------------------------
    // Category H: Integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_import_pipeline_validate_resolve_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        setup_package_dir(tmp.path(), "shared.security", &["1.2.0"]);

        let resolver = FileSystemResolver::single(tmp.path().to_path_buf());

        // Validate the import entry
        let entry = ImportEntry::new("shared.security".to_owned(), Some("^1.0.0".to_owned()));
        let validated = validate_import(&entry).unwrap();
        assert_eq!(validated.package(), "shared.security");

        // Resolve it
        let resolved = resolver.resolve(&validated).unwrap();
        assert_eq!(resolved.package, "shared.security");
        assert_eq!(resolved.version, semver::Version::parse("1.2.0").unwrap());
        assert_eq!(resolved.file.header.package, "shared.security");
    }

    #[test]
    fn test_full_import_pipeline_circular_detection_rejects() {
        use crate::import::{ImportResolver, ValidatedImport, detect_circular_imports};
        use crate::model::fields::{NodeType, Span};
        use crate::model::file::{AgmFile, Header};
        use crate::model::imports::ImportEntry;
        use crate::model::node::Node;
        use std::collections::HashMap;
        use std::path::PathBuf;

        // Build a mock circular scenario: pkg-a imports pkg-b, pkg-b imports pkg-a
        struct CircularMock {
            packages: HashMap<String, ResolvedPackage>,
        }

        impl ImportResolver for CircularMock {
            fn resolve(
                &self,
                import: &ValidatedImport,
            ) -> Result<ResolvedPackage, crate::error::AgmError> {
                self.packages.get(import.package()).cloned().ok_or_else(|| {
                    crate::error::AgmError::new(
                        crate::error::ErrorCode::I001,
                        format!("not found: {}", import.package()),
                        crate::error::ErrorLocation::default(),
                    )
                })
            }
        }

        fn make_pkg(name: &str, imports: Vec<ImportEntry>) -> ResolvedPackage {
            ResolvedPackage {
                package: name.to_owned(),
                version: semver::Version::parse("1.0.0").unwrap(),
                path: PathBuf::from(format!("{name}.agm")),
                file: AgmFile {
                    header: Header {
                        agm: "1.0".to_owned(),
                        package: name.to_owned(),
                        version: "1.0.0".to_owned(),
                        title: None,
                        owner: None,
                        imports: if imports.is_empty() {
                            None
                        } else {
                            Some(imports)
                        },
                        default_load: None,
                        description: None,
                        tags: None,
                        status: None,
                        load_profiles: None,
                        target_runtime: None,
                    },
                    nodes: vec![Node {
                        id: format!("{name}.node"),
                        node_type: NodeType::Facts,
                        summary: "node".to_owned(),
                        span: Span::new(1, 1),
                        ..Default::default()
                    }],
                },
            }
        }

        let entry_a = ImportEntry::new("pkg-a".to_owned(), None);
        let entry_b = ImportEntry::new("pkg-b".to_owned(), None);

        let pkg_b = make_pkg("pkg-b", vec![entry_a.clone()]);
        let pkg_a_as_dep = make_pkg("pkg-a", vec![entry_b.clone()]);

        let mut packages = HashMap::new();
        packages.insert("pkg-b".to_owned(), pkg_b);
        packages.insert("pkg-a".to_owned(), pkg_a_as_dep);

        let mock = CircularMock { packages };

        let root_imports = vec![validate_import(&entry_b).unwrap()];
        let err = detect_circular_imports("pkg-a", &root_imports, &mock).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::I003);
    }
}
