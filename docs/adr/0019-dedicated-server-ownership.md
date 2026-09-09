# ADR 0019 - Dedicated server owns game lifetime

2026-09-09; accepted implementation direction from user discussion.
One separately running server process owns the registry and all table/tournament
actors. The game creator is a client and cannot stop that process by leaving.
Existing authorized LobbySession create/join and registry runtime are reused.
The installed Host route connects to an existing endpoint; it no longer embeds
an authority. Practice remains local. No new poker or wire authority is added.

SB2 invites carry a socket address, table ID and private access code. Existing
SB1 loopback invitations remain parseable. Malformed invitations are rejected
without echoing access material. Endpoints remain loopback-only for this local
milestone; protected LAN transport is a distinct next integration boundary.
Pi/Linux service packaging and actual VLAN validation follow when available.

Multiple independent tournaments are supported by the existing registry;
balancing players within one multi-table tournament remains D2 work. Existing
between-hand persistence is retained; active-hand crash durability is not claimed.
