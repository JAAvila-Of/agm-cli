# Releasing agm-cli

## Version Management

agm-cli uses **manual versioning** via `Cargo.toml`. The version lives in two places:

| File | Field |
|------|-------|
| `crates/agm-core/Cargo.toml` | `version = "X.Y.Z"` |
| `crates/agm-cli/Cargo.toml` | `version = "X.Y.Z"` |

Both must match. The release pipeline validates that the git tag matches the version in `Cargo.toml`.

### Using cargo-release (recommended)

[cargo-release](https://github.com/crate-ci/cargo-release) automates version bumps, commits, and tags:

```bash
# Install once
cargo install cargo-release

# Bump patch version (0.1.0 → 0.1.1), commit, and tag
cargo release patch --execute

# Bump minor version (0.1.0 → 0.2.0)
cargo release minor --execute

# Bump major version (0.1.0 → 1.0.0)
cargo release major --execute

# Dry run (preview what will happen)
cargo release patch
```

`cargo-release` will:
1. Update version in both `Cargo.toml` files
2. Create a git commit: `chore: release vX.Y.Z`
3. Create git tag: `vX.Y.Z`
4. Push commit + tag to origin

### Manual process

```bash
# 1. Edit versions
#    crates/agm-core/Cargo.toml → version = "0.2.0"
#    crates/agm-cli/Cargo.toml  → version = "0.2.0"

# 2. Commit
git add -A
git commit -m "chore: release v0.2.0"

# 3. Tag
git tag v0.2.0

# 4. Push
git push origin main --tags
```

## CI/CD Pipelines (Azure DevOps)

All pipelines clone from GitHub via the `github-connection` service connection.

### CI Build (`azure-pipelines.yml`)

**Trigger**: Push to `main` or `release/*`, PRs to `main`

| Stage | What |
|-------|------|
| Check | `cargo fmt --check` + `cargo clippy` (Ubuntu) |
| Test | `cargo test --workspace` on Windows + macOS |

### Release (`azure-pipelines-release.yml`)

**Trigger**: Push tag `v*`

| Stage | What |
|-------|------|
| Validate | Checks git tag matches `Cargo.toml` version |
| Test | Full test suite on Windows + macOS |
| BuildWindows | MSI installer + portable zip (x86_64) |
| BuildMacOS | Universal .pkg (Intel + Apple Silicon) + portable tar.gz |
| PublishCrate | `cargo publish` to crates.io (agm-core first, then agm-cli) |
| GitHubRelease | Creates GitHub release with all artifacts |

### Pipeline diagram

```
push to main/PR ──→ CI Build (lint → test)
                        │
                      merge
                        │
git tag v0.1.0 ────→ Release Pipeline
                        │
                    Validate tag
                        │
                   Test (Win+Mac)
                        │
              ┌─────────┴─────────┐
         Build Windows       Build macOS
         (MSI + zip)         (universal .pkg + tar.gz)
              └─────────┬─────────┘
              ┌─────────┴─────────┐
         Publish crates.io  GitHub Release
              └───────────────────┘
```

## Release Artifacts

Each release produces:

| Artifact | Platform | Type |
|----------|----------|------|
| `agm-vX.Y.Z-x86_64-windows.msi` | Windows x86_64 | Installer (adds to PATH, Add/Remove Programs) |
| `agm-vX.Y.Z-x86_64-windows.zip` | Windows x86_64 | Portable binary |
| `agm-vX.Y.Z-universal-macos.pkg` | macOS Intel + Apple Silicon | Installer (installs to `/usr/local/bin`) |
| `agm-vX.Y.Z-universal-macos.tar.gz` | macOS Intel + Apple Silicon | Portable binary |

## Installation Methods

### Windows

**MSI installer** (recommended):
1. Download `.msi` from GitHub Releases
2. Double-click to install
3. `agm` is added to PATH automatically
4. Uninstall from Settings → Apps

**Portable**:
1. Download `.zip` from GitHub Releases
2. Extract `agm.exe` anywhere
3. Add location to PATH manually

**PowerShell script**:
```powershell
irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
```

### macOS

**pkg installer** (recommended):
1. Download `.pkg` from GitHub Releases
2. Double-click to install (installs to `/usr/local/bin/agm`)
3. Works on both Intel and Apple Silicon Macs

**Portable**:
```bash
curl -L https://github.com/JAAvila-Of/agm-cli/releases/latest/download/agm-vX.Y.Z-universal-macos.tar.gz | tar xz
sudo mv agm /usr/local/bin/
```

**Shell script**:
```bash
curl -sSf https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
```

### From source (any platform)

```bash
cargo install agm-cli
```

## Required Azure DevOps Configuration

| Item | Type | Where | Purpose |
|------|------|-------|---------|
| `github-connection` | GitHub | Project Settings → Service Connections | Clone repo + create GitHub Releases |
| `cargo-connection` | Generic | Project Settings → Service Connections | Publish to crates.io (token stored securely) |

### Setting up `cargo-connection`

1. Go to **Project Settings → Service Connections → New service connection**
2. Select **Generic**
3. Fill in:
   - **Connection name**: `cargo-connection`
   - **Server URL**: `https://crates.io`
   - **Password/Token Key**: your crates.io API token (from https://crates.io/settings/tokens)
4. Grant access to all pipelines (or authorize per-pipeline)

The release pipeline fetches the token at runtime from this service connection using the Azure DevOps REST API. No secret variables needed in the pipeline itself.

## Semantic Versioning

Follow [SemVer 2.0.0](https://semver.org/):

| Change | Bump | Example |
|--------|------|---------|
| Breaking API change | **major** | `1.0.0 → 2.0.0` |
| New feature, backward compatible | **minor** | `0.1.0 → 0.2.0` |
| Bug fix | **patch** | `0.1.0 → 0.1.1` |
| Pre-release | pre-release suffix | `0.1.0-alpha.1` |
