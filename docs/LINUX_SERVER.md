# Linux dedicated server

Target: supplied Fedora 44 x86_64 dedicated host. This runbook covers an
operator-managed server with administrative SSH access. Players connect directly
to the verified TLS listener at `192.168.5.250:6969`.

## Build and install

Build with Rust stable, a C linker and the repository's Cargo.lock. The native
toolchain was installed under the dedicated service account using the
[official Rust installer](https://rust-lang.org/tools/install/), without editing
shell startup files or installing machine-wide packages.

On Windows, `python scripts/package_linux_source.py` creates an allowlisted source
archive and SHA-256 manifest under `output/sprint20`. It includes current working
source, not only committed files; .env and credentials are excluded. Transfer the
archive through SSH/SFTP, extract into a fresh build directory, then on Linux:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
bash deploy/linux/build.sh
```

Extract the reported release archive into a fresh directory, then:

```bash
bash install.sh
systemctl --user enable --now poker-server
systemctl --user status poker-server --no-pager
journalctl --user -u poker-server -n 30 --no-pager
```

The installer verifies package checksums and refuses to install while the service
is active. Runtime files are kept separately at
`~/.local/share/sneakyblinders/state/` with a private umask. Releases live under
`~/.local/share/sneakyblinders/releases/`; `current` selects one release atomically.
The unit resides at `~/.config/systemd/user/poker-server.service`.

## Connect from Windows

Run the updated installed `sneakyblinders`, then choose **Host Game** or
**Join Game**. Both connect automatically to `192.168.5.250:6969`. Join shows the
server's open and password-protected games. Players need neither SSH access nor
a separate terminal. Game passwords are independent of operator credentials.

Fresh profiles use this endpoint. Saved `127.0.0.1:7777` and
`127.0.0.1:17777` defaults migrate automatically after a successful connection;
custom addresses remain explicit overrides. In the lobby, S changes the server.
Connecting supports Esc cancellation; failure presents Retry/Back.

The client embeds the deployment's public CA certificate, verifies the server IP
and certificate lifetime, and sends game credentials only after TLS succeeds.
It never accepts arbitrary certificates or falls back to plaintext for remote
addresses. Legacy loopback development ports other than 6969 still use plaintext.
Do not configure a remote server through a legacy loopback forward.

## Listener, firewall and certificate operations

The installed service binds only `192.168.5.250:6969`. The host firewall permits
TCP 6969 addressed to that IP in the FedoraWorkstation zone, runtime and persistent.
Network routing/ACLs across VLANs remain administered by the network team. A
successful check from the development Windows PC does not prove every VLAN.
SSH remains operator-only. The historical scoped SSH-forwarding policy for 7777
is retained, but the game no longer depends on it. SELinux remains enforcing.

TLS files live under `~/.local/share/sneakyblinders/tls/` (directory 0700,
private files 0600). `server.pem` and `server.key` are loaded at service startup.
The deployment CA signing key stays on the host; only `ca.der` is copied into
`assets/network/server-ca.der` and embedded in client builds. Never distribute
`ca.key`, `server.key`, `.env`, checkpoints or reconnect credentials. The keys
in `tests/fixtures/tls/` are deliberately artificial public test material.

The current server certificate expires **2028-12-12 05:42:23 UTC**. Before expiry,
issue a new server certificate from the same deployment CA, with SAN IP
192.168.5.250 (and loopback 127.0.0.1 for local checks), serverAuth usage, and
appropriate dates. Validate the certificate/key pair with OpenSSL; replace the
host files privately during a maintenance window and restart the service. Existing
clients continue trusting a renewed leaf from the same CA. A CA replacement
requires distributing updated clients before switching the server chain. Never
bypass verification to resolve a certificate error.

TLS handshakes have a five-second budget. Pending workers are bounded by server
connection capacity. Per-source-IP admission permits 30 new TLS connections and
60 lobby requests per ten seconds, with bounded tracking storage. These are
private-LAN guardrails; shared NAT clients share a budget. Internet-facing identity,
capacity, trust distribution and operations remain separate release work.

## Stop, restart and boot

```bash
systemctl --user stop poker-server
systemctl --user start poker-server
systemctl --user restart poker-server
loginctl show-user "$USER" -p Linger
```

The unit sends SIGINT, allowing the existing drain and between-hand checkpoint
path to run. Stop/restart in a maintenance window after games finish. Active-hand
crash recovery is not implemented; automatic restart cannot restore an interrupted
hand. Never delete the state directory to resolve an error without investigating.

Service enablement alone is not boot-without-login support. Lingering must be
enabled for the account (`loginctl enable-linger "$USER"`, subject to host policy).
Actual reboot validation is separate; do not reboot a shared host for a smoke test.

## Upgrade, rollback and remove

Stop after games finish, preserve a private backup of the state directory, extract
the new release and run its installer. Start and check the journal. Previous
release paths are recorded in `~/.local/share/sneakyblinders/previous-release`.
For rollback, stop first, choose a known release under the same releases directory,
and run that release's `install.sh`; it restores both binary selection and unit.
Only roll back with compatible saved-state formats. Checkpoints contain private
material; do not attach them to ordinary bug reports.

To disable: `systemctl --user disable --now poker-server`. This preserves saved
state and releases. Removal of data is a separate explicit operator action.

## Support boundary

This artifact is validated on Fedora 44 x86_64 with the runtime libraries listed
inside the bundle. It is not an ARM/Pi or all-distribution Linux binary. Public release, client distribution, ARM validation and active-hand durability
remain backlog work. Existing web,
desktop, SSH and file-sharing services are outside this installation's scope.

Waiting-host correction (2026-09-09): JoinStatus now polls every 500 ms and uses
a separate per-connection allowance of 30 requests per ten seconds. It does not
consume the shared per-IP 60-request admission budget for other lobby operations.
This accommodates ordinary waiting and multiple clients on one machine while
retaining abuse limits. Final server responses now flush a bounded TLS close_notify
so a deliberate rejection is not obscured by an unexpected-EOF error.
