#!/usr/bin/env bash
# Run from the extracted source root. Produces a target-specific release bundle.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --locked --release --bin poker-server
bundle="$(mktemp -d target/poker-server-linux-x86_64.XXXXXXXX)"
install -m 755 target/release/poker-server "$bundle/poker-server"
install -m 644 deploy/linux/poker-server.service "$bundle/poker-server.service"
install -m 755 deploy/linux/install.sh "$bundle/install.sh"
install -m 644 source-manifest.json "$bundle/source-manifest.json"
rustc -Vv > "$bundle/toolchain.txt"
ldd target/release/poker-server > "$bundle/runtime-libraries.txt"
(cd "$bundle" && sha256sum poker-server poker-server.service install.sh source-manifest.json toolchain.txt runtime-libraries.txt > SHA256SUMS)
release="$(sha256sum "$bundle/SHA256SUMS" | cut -c1-16)"
archive="target/poker-server-linux-x86_64-$release.tar.gz"
tar -C "$bundle" -czf "$archive" .
printf 'BUNDLE %s\n' "$archive"
