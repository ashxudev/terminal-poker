# ADR 0022 - Automatic direct LAN connection

Accepted for the owner-requested Sprint 20 extension, 2026-09-09. Network admin
allocated TCP 6969. Players use the TUI directly; SSH remains administration only.

Default endpoint is 192.168.5.250:6969. The distributed client embeds only the
public deployment CA certificate and verifies certificate validity and IP/SAN
with rustls. No insecure verifier, trust-on-first-use prompt or plaintext fallback
for a remote endpoint. Server and CA private keys remain private on the host.
Certificate renewal under the same CA preserves client compatibility; CA rotation
requires a coordinated client update. Leaf certificate expires after 825 days.

The existing framing/authority stack operates over a stream abstraction with
plain loopback and verified TLS variants. Non-loopback server binding requires
TLS configuration. Server admission bounds cover handshake workers and connection
attempts, plus lobby request throttling. No TLS-to-plaintext network proxy.

The TUI starts connection work off the input/render loop and allows Escape to
cancel or retry/change endpoint after failure. Existing reconnect uses the same
verified transport. Known former default/tunnel endpoints migrate to the direct
default; other explicit local overrides remain usable. No new appearance options.

This extends Sprint 20's operator-only milestone into direct private LAN reach.
It does not supply public account identity, internet discovery, active-hand crash
recovery or ARM portability. Port 6969 host/network reach is tested from this
Windows client; all other VLAN routes remain the network administrator's scope.

Waiting-host correction (2026-09-09): JoinStatus now polls every 500 ms and uses
a separate per-connection allowance of 30 requests per ten seconds. It does not
consume the shared per-IP 60-request admission budget for other lobby operations.
This accommodates ordinary waiting and multiple clients on one machine while
retaining abuse limits. Final server responses now flush a bounded TLS close_notify
so a deliberate rejection is not obscured by an unexpected-EOF error.
