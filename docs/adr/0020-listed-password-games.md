# ADR 0020 - Listed password-protected games

2026-09-09, user-approved lobby policy: open and password-protected games are
listed. Visibility is distinct from access. Add PasswordProtected visibility;
retain Private/Unlisted semantics for legacy tables so existing secret tables
are not silently exposed. New tournaments use Public for an empty password and
PasswordProtected otherwise. Public summaries carry no access material.
Passwords are 4-96 printable ASCII bytes (spaces supported), case-sensitive;
blank host password means open. Password entry is masked and never persisted
in the player profile. Server stores a salted Argon2id verifier for the new
password visibility; legacy random-invite SHA-256 verifiers remain readable.
Argon2 API source: https://docs.rs/argon2/0.5.3/argon2/struct.Argon2.html

Wire v5/lobby v2 reject older clients that cannot parse the new visibility.
Checkpoint v4 is written; v3 is accepted with its existing visibility intact.
Profile v2 stores remembered server address; v0/v1 migrate without overwriting
until an explicit save. Old clients must not downgrade-write this new profile.

Installed lobby lists tournament games, retains unavailable rows with a reason,
refreshes periodically, preserves selection by stable table ID, and supports
server change. Existing ring games are shown as unsupported by this tournament
client rather than disappearing. Server remains authoritative at admission.
This is local loopback UI/access work; protected network transport is separate.

CancelWait now also withdraws an already seated entrant while tournament status
is Registering and no runtime exists. Lifecycle, durable roster, seat route and
tournament entrants are updated together. A socket-departure guard performs the
same bounded cleanup; it cannot withdraw from a started hand. Concurrent start
wins over late cancellation. Directory request failure disables admission until
a successful retry. Network requests retain the existing five-second bound;
fully asynchronous connection cancellation belongs to the next LAN/UX slice.
