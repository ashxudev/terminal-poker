# ADR 0017: Ordered tournament showdown and private mucks

## PokerStars alignment - 8 September 2026 (current)

The user's request to align with PokerStars supersedes the best-first checked
river and five-second human choice policy below. The final called river bettor
or raiser shows first. A checked river starts at the first live seat left of the
button, including heads-up. A later hand auto-mucks only if already tabled eligible
hands strictly beat it in every pot it contests. Winners and ties always table.
Humans and AI share this default; H during betting opts into always-show for this
hand. No showdown input is required and no optional choice window delays payout.
Each newly tabled hand and runout street retains its 1.5-second server interval;
skipped mucks add no extra interval. Winner highlighting retains its 1.5 seconds.
All-in exposure, uncontested no-runout, best-five highlights and chip rules stay.

Sources checked 2026-09-08: [PokerStars showdown order](https://www.pokerstars.com/help/articles/poker-rules-master/),
[show/muck settings](https://www.pokerstars.com/help/articles/show-hole-card/45158/),
[beaten-hand mucking](https://www.pokerstars.com/poker/learn/lesson/poker-showdown/).
This implements their automatic-muck style, not complete client parity or a
claim about their exact animation timing. Optional showdown buttons, uncontested
show buttons and account-persistent preferences are not added. Public history
privacy remains stricter: no access to other players' mucked cards.

Protocol/wire v4 removes optional decision fields and rejects v3 peers. The
existing pre-showdown preference command remains private, authorized, idempotent
and unable to extend a betting deadline. Lobby and checkpoint versions are unchanged.

## Superseded post-review product correction - 8 September 2026

The user's follow-up supersedes the checked-river and automatic human muck
procedure below. With no called river aggressor, the best hand tables first;
this is an explicit digital house policy, not a claim that TDA removes the
clockwise showing obligation. Remaining pot winners/ties table automatically.
A losing human may press H during a shared five-second server-owned window to
show both cards; silence mucks. AI chooses immediately, auto-mucking beaten
hands. Always-show selected during betting remains available. The first called
river aggressor and tournament all-in exposure obligations are retained.
Only members of the winner's evaluated best five receive green brackets,
including zero, one or two hole cards. Both hole cards remain visible when shown.
Uncontested awards stop board dealing immediately; no rabbit hunting. A called
main pot still runs out even if an additional side-pot bet receives no call.
Optional choice identities are public, private cards are not. Reconnect and
repeated requests cannot restart the window. Protocol/wire v3 rejects older peers.

## Original Sprint 17 decision (historical)


Status: Accepted for Sprint 17 implementation under the user's requested rules review.
Date: 2026-09-08. Supersedes the reveal portion of ADR 0007.

## Policy

Baseline: [Poker TDA 2024 rules 13, 16-19](https://www.pokertda.com/view-poker-tda-rules/),
verified 2026-09-08. Fold wins do not require exposure. Ordinary river showdowns
start with the final river bettor/raiser; a checked river starts left of the
button. Both cards are tabled to claim a contested pot or tie. Tournament all-ins
table every remaining hand once no further betting is possible, before runout.
Side-pot betting must finish before exposure. Folded cards remain private.

## Digital procedure

Automatic show/muck is the initial implementation default, subject to user
clarification: first hand shows; subsequent hands muck only if strictly beaten
by an already tabled eligible hand in every pot they contest. Always-show can
be selected before the showdown begins. This never mucks a winner or a tie and
uses no unseen opponent cards in deciding whether to muck. The called river
aggressor always tables, satisfying the caller's right to see that hand without
a separate request action. Discretionary requests to expose other mucks and
physical-card retrieval disputes are outside this digital procedure.

The serialized hand authority owns reveal order, the public reveal set, runout,
and settlement. Clients receive only cards already public plus their own cards.
Server-controlled intervals advance the pending flow, independent of terminal
rendering, with 1.5 seconds after each reveal/runout step. The terminal holds
winners for 1.5 seconds and the existing award step for one second. Uncontested
hands show only an award. Domain-only simulations can drain the same sequence
synchronously; installed/network runtimes advance it over time.

Protocol changes require a version bump and explicit old-version rejection.
Reconnect snapshots include current flow state but no future cards or awards.
Public histories contain only tabled cards; private mucks never enter them.
Between-hand checkpoints remain the persistence boundary; no active-hand crash
recovery is added. Tests must cover order, all-ins, side pots, ties, privacy,
duplicate/stale intent, disconnect/reconnect and exact-once settlement.
