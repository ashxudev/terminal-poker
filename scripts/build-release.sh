#!/bin/bash
# Build release binaries for multiple platforms
# Requires: cross (cargo install cross)

set -e

VERSION=$(grep '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
echo "Building terminal-poker v$VERSION"

TARGETS=(
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
)

mkdir -p dist

for TARGET in "${TARGETS[@]}"; do
    echo "Building for $TARGET..."

    if [[ "$TARGET" == *"darwin"* ]]; then
        # Native macOS build
        cargo build --release --target "$TARGET"
    else
        # Cross-compile for Linux
        cross build --release --target "$TARGET"
    fi

    # Create archive
    ARCHIVE="terminal-poker-$TARGET.tar.gz"
    mkdir -p "dist/$TARGET"
    cp "target/$TARGET/release/terminal-poker" "dist/$TARGET/"
    cp "target/$TARGET/release/poker" "dist/$TARGET/"
    tar -czvf "dist/$ARCHIVE" -C "dist/$TARGET" terminal-poker poker
    rm -rf "dist/$TARGET"

    echo "Created dist/$ARCHIVE"
done

echo "Done! Release artifacts in dist/"
ls -la dist/
