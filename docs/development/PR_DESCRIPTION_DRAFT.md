# Proposed title

Add Sneaky Blinders multiplayer client and dedicated LAN server

# Draft body

The repository currently contains the original offline heads-up game. This change
adds the Sneaky Blinders terminal app and a separate server that owns multiple
independent poker games. Players use Host Game or Join Game to create or browse
open/password-protected single-table tournaments; creators can leave without
terminating the dedicated server.

The client connects automatically to the configured private LAN server over
verified TLS. The server remains authoritative for cards, legal actions, pots,
showdown and tournament progression. Private projections keep opponents' cards
hidden until legitimate reveal. Waiting-status polling has a separate bounded
allowance so normal two-terminal setup does not disconnect the Host.

## Scope

- Multiplayer domain, protocol/session boundaries, lobby and tournament client.
- Unified terminal UI, embedded branding and compact fallback.
- Verified TLS, reconnect credentials, private projections and admission limits.
- Native Linux package/service inputs, compatible state handling and runbook.
- Rules, process, privacy, TLS and delayed-join regression coverage.

This is an integration baseline accumulated since the original game, not only the
last lobby change. Existing training/review modules are retained because they are
part of the exported library and declared Cargo targets. Generated research,
private runtime state and historical screenshot/checkpoint bundles are excluded.

## Validation

- Isolated source-file checkout: Windows all-target/all-feature check and tests;
  321 passed, zero failures, four existing ignored.
- Existing matching Linux source validation: 319 passed, zero failures, three
  existing ignored; locked release build and strict Clippy passed.
- Live direct TLS Host/Join, waiting more than 30 seconds before a second terminal
  joins, reconnect and completed tournament verified on Windows against Linux.
- Quality CI now includes Apple Silicon macOS, Intel macOS, Windows and Linux.
  Remote results are reported on the PR; real Mac terminal/network smoke remains
  pending regardless of compilation results.

## Compatibility and limits

Wire version 5 / lobby version 2 require compatible peers. Persisted state follows
the documented checkpoint/profile migrations. Players need no operator SSH access.
Public CA material and artificial test keys are included; deployment private keys,
passwords, reconnect tokens and private game state are excluded.

Target is private-LAN play-money single-table tournaments, with independent games
on one server. Public internet release, multi-table tournament movement and
active-hand crash recovery are not accepted by this change. Window focus raising
is Windows-only; graphics and colours depend on terminal capabilities.

## Review readiness

README onboarding, ADR/evidence links and desktop CI matrix are prepared.
The owner authorized creation of a fork PR. No release tag or package publication
is requested. Exact proposed files: PR_FILE_LIST.txt.
