#!/bin/bash
# Quick install script for terminal-poker
# Usage: curl -sSL https://raw.githubusercontent.com/terminal-poker/terminal-poker/main/scripts/install.sh | bash

set -e

REPO="terminal-poker/terminal-poker"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    darwin) OS="apple-darwin" ;;
    linux) OS="unknown-linux-gnu" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"
echo "Detected platform: $TARGET"

# Get latest version
VERSION=$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$VERSION" ]; then
    VERSION="v0.1.0"  # Fallback
fi
echo "Installing terminal-poker $VERSION"

# Download
URL="https://github.com/$REPO/releases/download/$VERSION/terminal-poker-$TARGET.tar.gz"
echo "Downloading from $URL"

TMPDIR=$(mktemp -d)
curl -sSL "$URL" | tar -xz -C "$TMPDIR"

# Install
mkdir -p "$INSTALL_DIR"
mv "$TMPDIR/terminal-poker" "$INSTALL_DIR/"
mv "$TMPDIR/poker" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/terminal-poker" "$INSTALL_DIR/poker"
rm -rf "$TMPDIR"

echo ""
echo "Installed to $INSTALL_DIR"
echo ""

# Check if in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    echo ""
fi

echo "Run 'poker' or 'terminal-poker' to start playing!"
