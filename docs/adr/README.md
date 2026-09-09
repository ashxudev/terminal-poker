# Architecture Decision Records

Use ADRs for decisions that materially affect authority, privacy, protocol compatibility, storage, recovery, rules interpretation, dependency direction, or deployment topology.

## Index

| ADR | Title | State |
|---|---|---|
| [0000](0000-template.md) | ADR template | Template |
| [0001](0001-server-authority-and-table-actor.md) | Server authority and serialized table actor | Accepted |
| [0002](0002-module-and-crate-dependency-direction.md) | Module and crate dependency direction | Accepted |
| [0005](0005-authoritative-randomness-and-deterministic-review.md) | Authoritative randomness and deterministic review fixtures | Accepted |
| [0006](0006-validated-seat-command-boundary.md) | Validated seat-command boundary | Accepted |
| [0007](0007-multiway-betting-pots-and-showdown.md) | Multiway betting, pots, and showdown | Accepted |
| [0008](0008-versioned-envelopes-and-private-projections.md) | Versioned envelopes and private projections | Accepted |
| [0009](0009-bounded-idempotency-ledger-and-table-mailbox.md) | Bounded idempotency ledger and table mailbox | Accepted |
| [0010](0010-authorized-sessions-deterministic-deadlines-and-subscriptions.md) | Authorized sessions, deterministic deadlines, and subscriptions | Accepted |
| [0011](0011-bounded-loopback-tcp-json-framing.md) | Bounded loopback TCP and length-prefixed JSON | Accepted |
| [0012](0012-authoritative-rules-and-table-lifecycle-boundary.md) | Authoritative rules and safe table-lifecycle boundary | Accepted |
| [0013](0013-bounded-multi-table-registry-and-public-lobby.md) | Bounded multi-table registry and public lobby boundary | Accepted |
| [0014](0014-between-hand-checkpoint-and-restart-boundary.md) | Between-hand checkpoint and restart boundary | Accepted |
| [0015](0015-durable-private-beta-credentials-history-and-drain.md) | Durable private-beta credentials, history, and drain boundaries | Accepted |
| [0016](0016-training-only-deal-plans-and-policy-boundary.md) | Training-only deal plans and projection-native policy boundary | Accepted |
| [0017](0017-ordered-tournament-showdown.md) | Ordered tournament showdown and private mucks; supersedes reveal portion of 0007 | Accepted |
| [0018](0018-embedded-branded-home.md) | Embedded branded home and terminal fallback | Accepted |
| [0019](0019-dedicated-server-ownership.md) | Dedicated server and independent game ownership | Accepted |
| [0020](0020-listed-password-games.md) | Listed open and password-protected games | Accepted |
| [0021](0021-linux-dedicated-server-deployment.md) | Native Linux dedicated-server deployment | Accepted |
| [0022](0022-automatic-lan-tls.md) | Automatic direct LAN connection with verified TLS | Accepted |

## Workflow

1. Copy `0000-template.md` to the next sequential number.
2. Set the state to Proposed.
3. Describe the decision context and real alternatives.
4. Record validation evidence and consequences.
5. Obtain the appropriate product or technical decision.
6. Mark Accepted, Rejected, or Superseded.
7. Add it to this index and `docs/agile/DECISIONS.md`.

ADRs are immutable historical records after acceptance. Supersede them with a new ADR rather than rewriting the original decision.
