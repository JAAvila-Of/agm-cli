#!/bin/bash
# Build release binaries for Windows and macOS.
# Requires cross-compilation toolchains or the `cross` tool.
#
# Usage:
#   ./scripts/build-release.sh [version]
#   ./scripts/build-release.sh 0.1.0
#
# Output: dist/ directory with archives and installers.

set -euo pipefail

VERSION="${1:-$(cargo metadata --no-deps --format-version 1 | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)}"
BIN_NAME="agm"
DIST_DIR="dist"

echo "Building agm v${VERSION}..."
mkdir -p "$DIST_DIR"

# ── Windows x86_64 ───────────────────────────────────────
echo ""
echo "=== Building for x86_64-pc-windows-msvc ==="
TARGET="x86_64-pc-windows-msvc"

if command -v cross &>/dev/null; then
    cross build --release --target "$TARGET" -p agm-cli
else
    cargo build --release --target "$TARGET" -p agm-cli
fi

STAGING_DIR="${DIST_DIR}/staging-${TARGET}"
mkdir -p "$STAGING_DIR"
cp "target/${TARGET}/release/${BIN_NAME}.exe" "$STAGING_DIR/"
cp LICENSE README.md "$STAGING_DIR/"
(cd "$STAGING_DIR" && zip -r "../agm-v${VERSION}-x86_64-windows.zip" .)
rm -rf "$STAGING_DIR"
echo "  -> ${DIST_DIR}/agm-v${VERSION}-x86_64-windows.zip"

# ── macOS Universal (Intel + Apple Silicon) ──────────────
echo ""
echo "=== Building for macOS Universal ==="

for MACOS_TARGET in x86_64-apple-darwin aarch64-apple-darwin; do
    echo "  Building ${MACOS_TARGET}..."
    if command -v cross &>/dev/null; then
        cross build --release --target "$MACOS_TARGET" -p agm-cli
    else
        cargo build --release --target "$MACOS_TARGET" -p agm-cli
    fi
done

echo "  Creating universal binary..."
mkdir -p "${DIST_DIR}/universal"
lipo -create \
    "target/x86_64-apple-darwin/release/${BIN_NAME}" \
    "target/aarch64-apple-darwin/release/${BIN_NAME}" \
    -output "${DIST_DIR}/universal/${BIN_NAME}"

# Portable tar.gz
(cd "${DIST_DIR}/universal" && tar czf "../agm-v${VERSION}-universal-macos.tar.gz" "${BIN_NAME}")
echo "  -> ${DIST_DIR}/agm-v${VERSION}-universal-macos.tar.gz"

# .pkg installer (macOS only)
if command -v pkgbuild &>/dev/null; then
    PKG_ROOT="${DIST_DIR}/pkg-root"
    mkdir -p "${PKG_ROOT}/usr/local/bin"
    cp "${DIST_DIR}/universal/${BIN_NAME}" "${PKG_ROOT}/usr/local/bin/${BIN_NAME}"
    chmod 755 "${PKG_ROOT}/usr/local/bin/${BIN_NAME}"
    pkgbuild \
        --root "$PKG_ROOT" \
        --identifier com.jaavila.agm \
        --version "$VERSION" \
        --install-location / \
        "${DIST_DIR}/agm-v${VERSION}-universal-macos.pkg"
    rm -rf "$PKG_ROOT"
    echo "  -> ${DIST_DIR}/agm-v${VERSION}-universal-macos.pkg"
else
    echo "  (pkgbuild not found — skipping .pkg, only tar.gz created)"
fi

rm -rf "${DIST_DIR}/universal"

echo ""
echo "Build complete. Artifacts in ${DIST_DIR}/:"
ls -la "$DIST_DIR"/agm-* 2>/dev/null || true
