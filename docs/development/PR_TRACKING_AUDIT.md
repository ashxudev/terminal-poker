# PR tracking audit - 2026-09-09

PR preparation authorized after this audit. Portable onboarding, four-platform CI,
ADR index updates and archive-reference labeling are now implemented. The audit
observations below describe the pre-commit state; final commits follow the explicit
file list. Native macOS CI results and real terminal/network smoke are distinct.

## Decision

Prepare one integration-baseline PR for the server-authoritative Sneaky Blinders
application, with code/assets/tests, maintainer documentation, and cross-platform
quality checks in separate cohesive commits. This is not a small lobby-only diff:
most implementation since the original offline game is still untracked.
Do not reconstruct historical sprint commits from today's files or remove exported
training/review modules simply to make the diff look smaller.

The exact proposed checkout is **186 files**, including 29 files already tracked.
[PR_FILE_LIST.txt](PR_FILE_LIST.txt) is the explicit inclusion list. It describes
files present after the proposed commits, not 186 newly added files. Nothing has
been staged or committed. Audit output and inventory remain local in output/pr-audit/.

## Repository and PR route

- Upstream: public `ashxudev/terminal-poker`, default branch `main`.
- Current checkout and observed upstream main both resolve to
  `2042aaf48833f17c853143a3f5db512aaca4e80e`.
- Authenticated account has READ access to upstream. Use a fork and a branch such
  as `feat/sneakyblinders-lan-baseline`, then a draft PR targeting upstream main.
- A local commit alone will not let Ash pull the branch. After an authorized push,
  he can check out the PR branch; main contains the new app only after merge.
- This audit performed read-only GitHub inspection. No fork, push, PR, tag,
  remote CI run, release, or message to Ash was created.

## Include in this PR

| Area | Decision and reason |
|---|---|
| Cargo.toml / Cargo.lock, all src/**/*.rs | The app, authority, protocol, lobby, TLS, tournaments, UI and fix form one build baseline. Retain exported training/review code and declared binaries; separating them would require another code change and validation. |
| tests/, examples/ | Existing correctness, privacy, process and TLS regressions, including delayed waiting-host reproduction. Examples remain production-renderer/acceptance tools. |
| assets/branding/wordmark.png and portrait.png | Required compile-time embedded menu resources. |
| assets/network/server-ca.der | Required public trust anchor for the deployed server. This is a public certificate, not its private signing key. |
| tests/fixtures/tls/ | All six artificial fixture files, including server.key and its README. That key is intentionally public test material; expiry/name/trust tests require it. |
| deploy/linux/ and scripts/package_linux_source.py | Reproducible Linux source manifest, binary bundle and service installation. The packager is necessary because build.sh expects source-manifest.json in an extracted archive. |
| scripts/run_rust_gate.ps1 | Maintainer Windows gate referenced by the toolchain guide; workstation defaults need clear labeling. |
| docs/adr/, docs/rules/, requirements | Authority, privacy, poker rules, compatibility and architecture context. |
| Current docs/agile top-level files and templates | Current backlog, status, decisions, release gates and delivery contract, without raw historical evidence. |
| docs/LINUX_SERVER.md and docs/development/ | Operator/developer handoff plus this audit and proposed PR description. Refresh stale entry points before opening the PR. |
| .github templates and quality workflow | Review and test infrastructure; add real macOS validation before calling the PR ready. |
| Existing license, changelog, demo and release configuration | Preserve upstream history; no unrelated deletions or release trigger changes. |
| .gitignore / .gitattributes | Exclude private/generated state; keep shell scripts and source portable across OS checkouts. |

The inclusion list retains the existing approximately 4.3 MiB demo GIF. The new
runtime branding images and public CA are tiny; they are not the bulk of the tree.
About 39,000 Rust lines are currently in untracked files (including tests/examples),
on top of the tracked-file edits. Review scope must reflect the complete app.

## Keep local and excluded

The ignore changes now cover tmp/, output/, the literal $build/ directory,
docs/agile/reports/, Python caches, local .env variants, known-host variants,
root runtime registry/hands files, tls/, and private *.key files. The artificial
TLS test key is explicitly allowlisted. Files remain on disk; nothing was deleted.

Generated artifacts were mixed with source: approximately 1.04 GiB of scratch,
332 MiB of output and 88 MiB of documentation/evidence. Historical review folders
include JSON checkpoints, not just screenshots. A blanket git add would be unsafe
and noisy. Keep logs, binary backups, archives, checkpoints, sessions, private
hands, keys, credentials and machine-specific render caches out of the PR.

A selected review PDF can later be shared as a separately inspected attachment.
Do not copy an entire historical evidence folder into the public repository.

## Defer from this player-sharing PR

- agents/ research/curriculum/arXiv material and unused assets/concepts/ or
  assets/references/ belong to a separate curated research/design change.
- Historical docs/agile/rituals/, private-beta operations guides and the older
  AGILE_DELIVERY_ASSESSMENT.md can be archived/reviewed separately.
- Historical sprint PDF builders, rendering scripts, remote capture/collection
  helpers and the old SSH tunnel helper are not required to build or play.
- Root AGENTS.md is already ignored by upstream policy. It is not required for
  the binary. Decide separately whether to adopt its Windows-focused delivery
  requirements as shared contributor policy; do not silently force-add it.

Deferred authoring material remains visible/unmodified unless it is inside a
specifically ignored generated directory. The explicit file list, not git add .,
is the staging boundary. No source-only exclusion is a deletion from disk.

## Public-content audit

Scanned 545 eligible text files outside tmp/, output/ and $build/ for the locally
stored secret values, recognizable provider-token formats, credential URLs and
private-key headers. No matches to stored SSH secrets or the checked provider-token
patterns were found. The sole private-key header was the documented artificial
TLS test fixture. This is a bounded pattern scan, not a guarantee against every
possible secret format. Generated evidence was excluded rather than certified safe.

The embedded DER parses as a public CA certificate. Fingerprint SHA-256:
`cf8610793a367cea811d6556befe0e82003f54253d0c84404ac03952b8f6edff`.
The real CA/server private keys stay on the host. No secrets file is in the list.

The source deliberately exposes the private LAN address and public trust anchor,
which Ash needs for automatic connection. These are not credentials. The public
PR audience still matters: generalize personal filesystem paths and operator
account references in onboarding documents; do not imply the LAN host is a public
service or publish the operator's password. Credentials belong to administration,
not player setup.

## Validation performed

Copied only the initial 176-file inclusion set to a new directory outside this
repository. The ten later additions are attributes, documentation and maintainer
packaging/gate scripts; they add no Rust code or runtime asset dependency.
All Rust source and all test fixtures were present. No case-insensitive path
collisions were found. Cargo used the existing dependency/build cache, but source,
fixtures and embedded assets came from the isolated copy.

- Locked, offline all-target/all-feature Windows cargo check: PASS.
- Locked, offline all-target/all-feature Windows tests: **321 passed, 0 failed,
  4 existing ignored**.
- Latest deployed-code Linux gate already records **319 passed, 0 failed,
  3 existing ignored**; it was not rerun for this tracking-only audit.
- Ignore probes: .env, known hosts, scratch, output and historical checkpoint
  ignored; public CA and artificial TLS fixture remain eligible.
- macOS compilation, terminal rendering and Ash's VLAN reach are still unverified.

Local proof: output/pr-audit/{inventory,snapshot,quality}.json and snapshot logs.

## Preparation before opening the draft PR

1. Make README lead with the checkout-based Sneaky Blinders launch command:
   cargo run --locked --release --bin sneakyblinders. Include Mac prerequisites,
   automatic Join, tested terminal sizes and the Windows-only focus limitation.
   Bare cargo run still launches the original game; published installers do not
   imply they contain this unmerged baseline.
2. Add macOS and Windows quality jobs alongside Linux. Use locked dependencies;
   run build/tests, not only a release plan. Existing release configuration lists
   Mac targets, but it does not prove this source has passed macOS quality checks.
3. Refresh the ADR index for 0018-0022. Label older beta/toolchain instructions and
   historical counts. Mark links into excluded local evidence as archive references;
   the public checkout should not pretend those artifacts are bundled.
4. Recheck the final staged diff and exact file list after those edits, including
   private-key exceptions and Cargo runtime assets. Validate the actual proposed
   commit checkout, then create the fork/branch/commits and draft PR when requested.
5. Keep ordinary PR validation separate from release tags: existing tag workflows
   can publish GitHub, Homebrew and crates.io releases. No release is part of this
   audit or the proposed initial Ash compatibility session.

## Suggested commit structure

1. feat(game): add server-authoritative multiplayer and Sneaky Blinders client
   (complete compileable code, lockfile, assets, tests and Linux deployment inputs).
2. docs: document LAN play, compatibility and server operations
   (current architecture/backlog, portable onboarding and maintainer utilities).
3. ci: validate the application across desktop platforms
   (quality matrix, repository hygiene and review templates).

Move required ignore/attribute rules into the first commit if creating commits
sequentially; never transiently commit credentials or generated state. Do not
stage Cargo targets without their matching modules, assets and fixtures.
