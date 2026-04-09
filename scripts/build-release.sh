#!/bin/bash
# Build release binaries for all supported targets.
# Requires cross-compilation toolchains or the `cross` tool.
#
# Usage:
#   ./scripts/build-release.sh [version]
#   ./scripts/build-release.sh 0.1.0
#
# Output: dist/ directory with archives for each target.

set -euo pipefail

VERSION="${1:-$(cargo metadata --no-deps --format-version 1 | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)}"
BIN_NAME="agm"
DIST_DIR="dist"

TARGETS=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
)

echo "Building agm v${VERSION} for ${#TARGETS[@]} targets..."
mkdir -p "$DIST_DIR"

for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "=== Building for ${TARGET} ==="

    # Use cross if available, otherwise cargo
    if command -v cross &>/dev/null; then
        cross build --release --target "$TARGET" -p agm-cli
    else
        cargo build --release --target "$TARGET" -p agm-cli
    fi

    STAGING_DIR="${DIST_DIR}/staging-${TARGET}"
    mkdir -p "$STAGING_DIR"

    # Copy binary
    case "$TARGET" in
        *windows*)
            cp "target/${TARGET}/release/${BIN_NAME}.exe" "$STAGING_DIR/"
            ;;
        *)
            cp "target/${TARGET}/release/${BIN_NAME}" "$STAGING_DIR/"
            ;;
    esac

    # Copy docs
    cp LICENSE README.md "$STAGING_DIR/"

    # Create archive
    ARCHIVE_NAME="agm-v${VERSION}-${TARGET}"
    case "$TARGET" in
        *windows*)
            (cd "$STAGING_DIR" && zip -r "../${ARCHIVE_NAME}.zip" .)
            echo "  -> ${DIST_DIR}/${ARCHIVE_NAME}.zip"
            ;;
        *)
            tar -czf "${DIST_DIR}/${ARCHIVE_NAME}.tar.gz" -C "$STAGING_DIR" .
            echo "  -> ${DIST_DIR}/${ARCHIVE_NAME}.tar.gz"
            ;;
    esac

    rm -rf "$STAGING_DIR"
done

echo ""
echo "Build complete. Archives in ${DIST_DIR}/:"
ls -la "$DIST_DIR"/*.{zip,tar.gz} 2>/dev/null || true
