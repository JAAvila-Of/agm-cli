#!/bin/sh
# AGM CLI installer for macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh

set -e

REPO="JAAvila-Of/agm-cli"
BIN_NAME="agm"
INSTALL_DIR="${AGM_INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS — macOS only
OS="$(uname -s)"
case "$OS" in
    Darwin) ;;
    *)
        echo "Error: This installer is for macOS only. On Windows, use install.ps1 or download the MSI."
        exit 1
        ;;
esac

# Get latest release version
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
    echo "Error: Could not determine latest version."
    exit 1
fi

VERSION="${LATEST#v}"
ARCHIVE="agm-v${VERSION}-universal-macos.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"

echo "Downloading agm ${VERSION} for ${TARGET}..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"

echo "Extracting..."
tar -xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}"

echo ""
echo "agm ${VERSION} installed to ${INSTALL_DIR}/${BIN_NAME}"

# Check if install dir is in PATH
case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo ""
        echo "Note: ${INSTALL_DIR} is not in your PATH."
        echo "Add it with:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""
        echo "Or add that line to your shell profile (~/.bashrc, ~/.zshrc, etc.)"
        ;;
esac
