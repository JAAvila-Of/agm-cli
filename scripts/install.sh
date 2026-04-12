#!/bin/sh
# AGM CLI installer for Linux and macOS
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
#   AGM_VERSION=v1.2.3 curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
#   AGM_INSTALL_DIR=/opt/agm/bin curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh

set -e

REPO="JAAvila-Of/agm-cli"
BIN_NAME="agm"
INSTALL_DIR="${AGM_INSTALL_DIR:-$HOME/.local/bin}"

# --- Detect OS + arch -----------------------------------------------------
uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
    Linux)
        case "$uname_m" in
            x86_64 | amd64)  TARGET="x86_64-unknown-linux-gnu"  ;;
            aarch64 | arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "error: unsupported Linux arch: $uname_m" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        case "$uname_m" in
            x86_64) TARGET="x86_64-apple-darwin"  ;;
            arm64)  TARGET="aarch64-apple-darwin" ;;
            *) echo "error: unsupported macOS arch: $uname_m" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "error: unsupported OS: $uname_s" >&2
        echo "       supported: Linux, Darwin (macOS)" >&2
        echo "       on Windows, use install.ps1" >&2
        exit 1
        ;;
esac

# --- Resolve version ------------------------------------------------------
if [ -n "${AGM_VERSION:-}" ]; then
    VERSION_TAG="$AGM_VERSION"
else
    echo "Fetching latest release..."
    VERSION_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
fi

if [ -z "$VERSION_TAG" ]; then
    echo "error: could not determine version to install" >&2
    exit 1
fi

# --- Download + verify ----------------------------------------------------
ARCHIVE="agm-${VERSION_TAG}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/${ARCHIVE}"
SHA_URL="${URL}.sha256"

echo "Downloading ${ARCHIVE}..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL"     -o "${TMPDIR}/${ARCHIVE}"
curl -fsSL "$SHA_URL" -o "${TMPDIR}/${ARCHIVE}.sha256"

echo "Verifying checksum..."
cd "$TMPDIR"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${ARCHIVE}.sha256" || {
        echo "error: checksum verification failed" >&2
        exit 1
    }
elif command -v shasum >/dev/null 2>&1; then
    # macOS default
    EXPECTED=$(awk '{print $1}' "${ARCHIVE}.sha256")
    ACTUAL=$(shasum -a 256 "${ARCHIVE}" | awk '{print $1}')
    [ "$EXPECTED" = "$ACTUAL" ] || {
        echo "error: checksum mismatch" >&2
        exit 1
    }
else
    echo "warning: no sha256sum/shasum available, skipping checksum verify" >&2
fi
cd - >/dev/null

# --- Extract + install ----------------------------------------------------
echo "Extracting..."
tar -xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
STAGE_DIR="${TMPDIR}/agm-${VERSION_TAG}-${TARGET}"
mv "${STAGE_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
chmod 755 "${INSTALL_DIR}/${BIN_NAME}"

echo ""
echo "Installed agm ${VERSION_TAG} to ${INSTALL_DIR}/${BIN_NAME}"

# --- PATH hint ------------------------------------------------------------
case ":$PATH:" in
    *":${INSTALL_DIR}:"*)
        echo "Run: agm --version"
        ;;
    *)
        echo ""
        echo "Note: ${INSTALL_DIR} is not in your PATH."
        echo "Add it by appending the following line to your shell profile"
        echo "(~/.bashrc, ~/.zshrc, ~/.profile, etc.):"
        echo ""
        echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
