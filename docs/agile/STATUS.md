# Delivery status - 2026-09-09

Sprint 20 and the waiting-host correction are complete. No new sprint is active.
Delivery accounting: 759 forecast / 644 accepted / 115 remaining; no additional
points are awarded for source sharing or this PR.

The installed client connects Host/Join directly to the managed Linux server at
192.168.5.250:6969 with verified TLS. Fresh profiles use that endpoint; old tunnel
defaults migrate. Players need no operator SSH access. The server owns multiple
independent single-table tournaments, open/password lobby access and game state.

The waiting-host correction separates status polling from admission budgets and
sends clean TLS closure after final responses. Two installed Windows clients
waited more than 30 seconds, joined and completed a tournament successfully.

Current runtime baseline: Linux release ec2addc908811de5; server SHA-256
`e4fb1b794c0855b85fb684788659cc7adadc806bd95029cc440af0636ed41186`.
Installed Windows client SHA-256
`651b03135ab4ae38ad7d7fe91f6090c498cc0381d3c2aadf7f93862921f166a3`.
Prior complete tests: Windows 321 passed / 0 failed / 4 existing ignored;
Linux 319 / 0 / 3. Strict Clippy and optimized builds passed.

## Source handoff

The owner authorized a fork PR following the [tracking audit](../development/PR_TRACKING_AUDIT.md).
The [explicit file list](../development/PR_FILE_LIST.txt) excludes private/generated
state and historical research/design archives. An isolated source-only snapshot
passed all 321 Windows tests. Portable onboarding and a four-platform Quality
matrix are included; PR CI results will establish the native Mac build status.
A real Mac terminal session and Ash's network route still need validation.

Historical sprint reviews and screenshots are retained locally rather than
included in this public checkout. References to those artifacts in older planning
records are labeled local archive references. No release tag/publication is part
of the PR. Public deployment, active-hand crash durability, ARM Linux validation
and multi-table tournament movement remain outside accepted scope.

Next recommended work: complete cross-VLAN and Mac player smoke checks, then
recovery/rollout reliability, Custom Practice and client packaging in backlog order.
See [Linux operations](../LINUX_SERVER.md), [ADR 0022](../adr/0022-automatic-lan-tls.md)
and [backlog](BACKLOG.md).
