# Development toolchain

Use current stable Rust, including rustfmt and Clippy, with the committed
Cargo.lock. The Quality workflow runs on Linux, Windows, Apple Silicon macOS and
Intel macOS. Install Rust using https://rust-lang.org/tools/install/.

## Platform prerequisites

- macOS: Apple's Command Line Tools provide the C compiler/linker needed by native
  dependencies. Run `xcode-select --install` if they are not already installed.
  Rustup chooses the native Apple Silicon or Intel toolchain.
- Windows: the normal MSVC toolchain needs the Visual Studio C++ Build Tools and
  Windows SDK. The development workstation also validated the GNU toolchain.
- Linux: install your distribution's C compiler/linker toolchain. The dedicated
  deployment was validated natively on Fedora x86_64.

## Build and test from the checkout

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin sneakyblinders --bin poker-server
cargo run --locked --release --bin sneakyblinders
```

Run from the repository root. The explicit binary name is required: bare
`cargo run` starts the original offline game. The lockfile is required for the
same dependency selection on collaborators' computers and CI.

The existing `scripts/run_rust_gate.ps1` is an optional Windows-maintainer runner.
Its default toolchain location describes the original workstation setup; supply
its parameters for an equivalent setup or use the portable Cargo commands above.
It is not a Mac installation dependency.

## Verification boundaries

Normal tests include rules, authority/privacy, process lifecycle and TLS. Existing
ignored tests include explicit desktop attention checks and release/stress checks;
they are not claimed by a normal test pass. Invoke them individually only with
the required runtime/desktop setup. Do not indiscriminately run all ignored tests
on a headless CI runner.

Native macOS compilation/tests do not prove terminal graphics, keyboard handling
or the player's network route. Smoke-test Host/Join, a complete hand, resizing and
clean exit in the actual terminal. Start at 120x40, with at least 80x24 for gameplay.
Windows-only automatic turn focus does not run on macOS.

The Cargo warning that src/main.rs belongs to both poker and terminal-poker is
intentional compatibility with the original command names. Historical local
review logs and generated evidence are excluded from the repository.
