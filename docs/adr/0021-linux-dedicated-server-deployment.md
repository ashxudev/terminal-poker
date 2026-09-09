# ADR 0021 - Native Fedora dedicated-server package

Accepted 2026-09-09 for Sprint 20.

The owner supplied a Fedora 44 x86_64 host and requested a Linux server sprint.
Build the same authoritative Rust server and locked dependencies on the target.
Record a source-file SHA-256 manifest because the accepted local baseline is
uncommitted. Upload only build inputs, never .env, profiles, checkpoints or secrets.

Install versioned releases under the user's local data directory with an atomic
current symlink. Keep private runtime state outside releases. A systemd user unit
owns lifetime independently of game clients and uses SIGINT for the existing
graceful drain handler. Reboot availability requires lingering; report that state
separately from enablement. No machine-wide package or firewall changes needed.

Retain the loopback transport restriction. Validate remote Windows clients over
authenticated SSH forwarding, retaining host-key trust locally after first use.
This is operator access, not the eventual player-facing LAN solution. Direct
VLAN reach, TLS/identity/rate-limit UX and public deployment remain separate.
No protocol/checkpoint version change. Fedora x86_64 artifact support is specific
to the tested runtime, not all Linux distributions or ARM/Raspberry Pi.
