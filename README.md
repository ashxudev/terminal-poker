# Sneaky Blinders

Terminal poker with local practice and server-authoritative multiplayer. One
dedicated server hosts multiple independent games; use **Host Game** to create an
open or password-protected tournament, or **Join Game** to browse the lobby.

## Run this checkout

This branch contains the new application. Existing published installers and
`cargo install terminal-poker` refer to the original release documented below.
Build the checkout to use Sneaky Blinders:

```bash
cargo run --locked --release --bin sneakyblinders
```

Run that command inside the repository after checking out this PR branch or
pulling the merged changes. Bare `cargo run` still launches the original offline
game. Optionally install the new command from this checkout:

```bash
cargo install --locked --path . --bin sneakyblinders
sneakyblinders
```

Install current stable [Rust](https://rust-lang.org/tools/install/) and your
platform's C build tools. On macOS, install Apple's command-line tools if needed:

```bash
xcode-select --install
```

Both Apple Silicon and Intel Mac builds are covered by the Quality workflow,
alongside Linux and Windows. CI checks compilation and tests; a real terminal
and network check is still needed for each player setup.

## Join the LAN game

Choose **Join Game**. The client automatically connects to `192.168.5.250:6969`
using verified TLS, then lists available games. Choose a game and enter its game
password when requested. Players need no SSH account or separate server terminal.
The configured private LAN address must be reachable from your network or VPN;
it is not a public internet service. S in the lobby changes the server address.

**Host Game** creates a game on the same dedicated server. Keep the waiting screen
open until the other players join. Leaving the client does not stop the server.
After updating a running installation, close and reopen the application.

Start with a **120-column by 40-row** terminal window. Menu/lobby setup has a
40x20 fallback; gameplay should use at least 80x24. Graphics-capable terminals
can show the branded menu; others use the text fallback. Colour and suit glyph
appearance depend on the terminal and font. Automatic focus/on-top on your turn
is implemented on Windows only; Mac users select their terminal normally.

The server owns cards, actions, timers, pots and results. This is play-money,
single-table tournament play with multiple independent games, not multi-table
tournament movement. Active-hand crash recovery and public internet release are
not supported milestones. See [development](docs/development/TOOLCHAIN.md),
[Linux operations](docs/LINUX_SERVER.md) and [current status](docs/agile/STATUS.md).
Historical review binaries, private state and generated screenshots are held in
local archives, not bundled in this source checkout.

---

# Original terminal-poker release

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/ashxudev/terminal-poker)

![terminal-poker gameplay](assets/demo.gif)

Heads-up No-Limit Texas Hold'em for the terminal, built with Rust and ratatui.

Practice your poker strategy against a rule-based AI bot with configurable aggression, track your stats over time, and sharpen your game — all without leaving the terminal.

## Features

- **Heads-up NLHE** — Full No-Limit Texas Hold'em with proper blind structure, button rotation, and all standard actions (fold, check, call, bet, raise, all-in)
- **Bot AI** — Rule-based opponent with preflop hand ranges, postflop board texture analysis, draw detection, and street-specific strategy
- **Configurable difficulty** — Adjust the bot's aggression level from passive (0.0) to aggressive (1.0)
- **Persistent stats** — Tracks VPIP, PFR, 3-bet%, c-bet%, aggression factor, BB/100 win rate, and more across sessions
- **TUI** — Colored card rendering, animated deals and reveals, action log, and interactive raise input

## Installation

### Cargo (requires [Rust](https://www.rust-lang.org/tools/install))

```bash
cargo install terminal-poker
```

### Quick install (macOS / Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ashxudev/terminal-poker/releases/latest/download/terminal-poker-installer.sh | sh
```

### Quick install (Windows PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ashxudev/terminal-poker/releases/latest/download/terminal-poker-installer.ps1 | iex"
```

### Homebrew (macOS / Linux)

```bash
brew tap ashxudev/terminal-poker
brew install terminal-poker
```

### Build from source

```bash
git clone https://github.com/ashxudev/terminal-poker.git
cd terminal-poker
cargo build --release
```

All methods install both `poker` and `terminal-poker` binaries.

## Usage

The current installed player entry point is:

```text
sneakyblinders
```

It opens Home without flags. Quick Practice runs repeatable nine-handed
play-money hands against eight local projection-bound bots using the production
multiplayer table. Hands continue automatically after the award. Host and Join
support open and password-protected single-table freezeout tournaments; Study remains future work.
Press Esc or Q to return Home.

Home now uses the reviewed skull portrait and SNEAKY BLINDERS bitmap wordmark
with a working six-entry menu: Quick Practice, Host Game, Join Game, Study,
Settings and Quit. Use arrows and Enter; S and ? still open Settings and Help.
Study remains unavailable. Artwork is embedded in the executable.

The bitmap layout uses a detected graphics-capable terminal at 100x36 or larger,
with the Ash true-color theme. It preserves the reviewed ivory/red palette.
Compact Home now fits down to 40x20 with a centered text wordmark, tagline,
all six entries and a clear selection highlight. The portrait is omitted.
Settings and Help retain an 80x24 requirement with Esc back to Home; gameplay
retains its own viewport requirements. Below 40x20, resize or quit safely.
Compact terminals, NO_COLOR and terminals
without graphics use the functional text menu. No terminal settings are changed.
Home images are cleared before gameplay, settings and entry screens.

Showdown keeps uncontested awards private and never deals an uncalled shove's
remaining board. A called river aggressor tables first. If the river checks
through, the first active seat left of the button tables first (big blind heads-up).
Winners and ties show both cards; only cards used in the best five get green
brackets. Tournament all-ins reveal all live hands only after betting is finished.

Beaten hands auto-muck for humans and AI. Press **H during betting** to toggle
always-show for the current hand. No showdown decision or five-second choice
window remains. Actual reveals and runout steps pause 1.5 seconds; skipping
mucked hands adds no delay. Winner highlights remain for 1.5 seconds before
the award. Public histories contain only shown cards; participant access to other
players' mucked cards is not implemented. This follows PokerStars-style automatic
mucking, without claiming complete client parity or identical animation timing.
All peers need the updated **table protocol 4 / wire version 4** build.
Restart running games to use it.

On Windows, your terminal requests focus and stays on top while it is your turn.
Automatic showdown does not request focus. It restores a minimized window and restores
its previous topmost state when your decision ends or you leave the table.
Keyboard activation uses a dedicated worker and temporary input-queue attachment
to restore the terminal input control, verified separately from window stacking.
Windows may still refuse activation across desktop/security boundaries; in that
case the taskbar flashes. This requires an identifiable visible local console
host (classic console or a Windows Terminal host exposing its owner window);
headless/remote terminals are unaffected. The game does not switch terminal tabs.

CMD, PowerShell, Git Bash, and other ConPTY-backed terminals are supported; the
shell ignores key-release events so the Enter used to launch the command cannot
also activate Quick Practice.

```bash
# Default: 100BB stacks, 0.5 aggression
poker # or terminal-poker

# Custom stack size (in big blinds) and bot aggression
poker --stack 200 --aggression 0.7
```

| Flag | Description | Default |
|------|-------------|---------|
| `--stack <BB>` | Starting stack size in big blinds | 100 |
| `--aggression <0.0-1.0>` | Bot aggression level | 0.5 |

### Play the current local build

The optimized Windows binaries are under `target\x86_64-pc-windows-gnu\release` after the project release gate runs.

For immediate solo play against the built-in AI:

```powershell
.\target\x86_64-pc-windows-gnu\release\terminal-poker.exe --stack 100 --aggression 0.5
```

For the authoritative network path, start a two-seat server and one client per seat in three terminals:

```powershell
# Terminal 1
.\target\x86_64-pc-windows-gnu\release\poker-server.exe --bind 127.0.0.1:17878 --seats 2 --stack 100

# Terminal 2
.\target\x86_64-pc-windows-gnu\release\poker-client.exe --connect 127.0.0.1:17878 --session player-s0

# Terminal 3
.\target\x86_64-pc-windows-gnu\release\poker-client.exe --connect 127.0.0.1:17878 --session player-s1
```

Network play currently requires one human client per seat. The `--headless` client is a short acceptance-test driver rather than a persistent bot opponent.

### Run the training arena

The initial policy-training tool runs one deterministic authoritative heads-up
hand with projection-native check/call policies and prints only the terminal
privacy-safe public history:

```bash
cargo run --bin poker-train -- --seed 73 --stack 100
```

The reusable Rust API is under `terminal_poker::training`. It includes validated
complete deal plans, exact private-card/public-runout fixtures, weighted range
sampling with card removal, versioned player-authorized observations, discrete
legal-action mapping, random and check/call policies, and a synchronous one-hand
arena. Mathematical oracles, tabular solvers, neural distillation, PPO, and
network bot deployment are later phases.

Measure optimized environment throughput with:

```bash
cargo run --release --bin poker-benchmark -- \
  --hands-per-case 100000 --workers auto --policy both --recording both
```

The JSON report includes hands/second, decisions/second, trajectory bytes,
failure counts, and environment-only projections through one billion decisions.
It explicitly lists oracle, CFR, and neural timings as unmeasured.

## Stats

Statistics are saved between sessions to `~/.local/share/terminal-poker/stats.json` (Linux) or the platform equivalent.

Tracked stats include:

- **Preflop** — VPIP, PFR, 3-bet frequency
- **Postflop** — C-bet%, fold to c-bet%
- **Showdown** — WTSD (went to showdown), W$SD (won $ at showdown)
- **Overall** — Aggression factor, BB/100 win rate, hands played, biggest pots

Press `S` in-game to view your session and lifetime stats.

## Multiplayer development

The project is being expanded toward server-authoritative, two-to-nine-player tables, concurrent ring games, and multi-table tournaments.

### Local network alpha

The current alpha runs one authoritative table on loopback. Start the server in one shell, then launch one terminal client per advertised session in separate shells:

```bash
# Shell 1: the printed port is selected by the OS
cargo run --bin poker-server -- --seats 3 --stack 100

# Shells 2-4: replace PORT with the LISTENING port
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session player-s0
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session player-s1
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session player-s2
```

Network controls are `F` fold, `C` call/check, `R` minimum bet/raise, `A` all-in, and `Q` quit. The server accepts loopback connections only, bounds each JSON frame to 64 KiB, and supports one active connection for each pre-bound `player-sN` alpha session. These are local development identities, not durable accounts or internet-safe credentials. The original `poker` and `terminal-poker` commands remain the offline heads-up game.

Run the independent-process acceptance journey with:

```powershell
& scripts/run_network_process_acceptance.ps1 -Seats 9
```

The multi-table local candidate can create public tables and retain their latest safe between-hand state across a server restart:

```powershell
# Start or restore a bounded registry. The checkpoint is created after a hand settles.
cargo run --bin poker-server -- --multi-table --max-tables 16 --checkpoint .\poker-registry.json

# Use the printed port to create, list, and join tables.
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session creator --create-table Alpha --table-seats 6 --table-stack 100
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session browser --lobby-list
cargo run --bin poker-client -- --connect 127.0.0.1:PORT --session alice --join-table 1 --seat 0
```

The version-1 checkpoint is bounded, checksummed, and atomically replaced. It contains private guest routing plus reconciled rosters/stacks, so protect it as server data. It deliberately excludes active-hand cards, decks, pots, awards, commands, deadlines, sockets, and runtime objects; a crash during a hand resumes from the prior between-hand boundary rather than replaying that hand.

- [Multiplayer requirements](NETWORKED_MULTIPLAYER_REQUIREMENTS.md)
- [Unified `sneakyblinders` player experience requirements](SNEAKYBLINDERS_PLAYER_EXPERIENCE_REQUIREMENTS.md)
- [Ratatui and TachyonFX UI map](docs/development/RATATUI_TACHYONFX_UI_MAP.md)
- UI reference catalogue (local archive: `assets/references/README.md`)
- [Agile delivery hub](docs/agile/README.md)
- [Canonical delivery loop](docs/agile/ITERATION_LOOP.md)
- Agent operating method (local archive: `AGENTS.md`)
- Delivery assessment (local archive: `AGILE_DELIVERY_ASSESSMENT.md`)
- [Development toolchain](docs/development/TOOLCHAIN.md)
- [Neutral table domain model](docs/development/DOMAIN_MODEL.md)

Appearance uses one unified presentation. Settings contains display name and
Quick Practice stack only; legacy saved theme/motion choices no longer alter the UI.

## Dedicated server (local milestone)

Start a server once in a separate terminal:

```powershell
poker-server --multi-table --bind 127.0.0.1:7777
```

Then run `sneakyblinders`, choose Host Game and accept `127.0.0.1:7777`.
Host asks for a game name and optional password, then tournament settings. Leave
the password blank for an open game. Join Game lists both open and password-
protected games on the remembered server; [LOCK] prompts for a masked password.
Use Up/Down and Enter to join, R to refresh, S to change server, or Esc to return.
The first connection asks for the server address once. Passwords are never saved
in the client profile. Esc withdraws registration while the game is waiting;
registration locks when the last player joins. The game and server survive a
creator leaving. The lobby supports 40x20 terminals; play uses the table minimum.
Both client and server must be updated together (wire 5 / lobby 2). Legacy hidden
server games remain hidden; their old invite parser is retained for compatibility.
The server owns all games and continues when a creator or player leaves.
Multiple independent tournaments share the server; tournament table-balancing is
not yet implemented. Escape cancels setup normally; a missing server produces a
recoverable connection error. Start the server before creating a game.

For durable between-hand checkpoints and safe histories, pass explicit
`--checkpoint <path>` and `--history <path>` to the server. Use distinct paths for
different server instances. Ctrl+C requests the existing safe drain path.
Active-hand crash recovery is not provided. This milestone keeps loopback-only
transport: Raspberry Pi/Linux service packaging and protected LAN connections
are subsequent work, so do not expose this server with a raw TCP proxy.

### Dedicated LAN service (Sprint 20)

The updated installed `sneakyblinders` connects Host Game and Join Game directly
to the managed Fedora server at `192.168.5.250:6969` with verified TLS. No SSH
or separate terminal is needed for players. Join opens the game directory; Host
creates an open or password-protected game. Earlier loopback examples remain
local developer modes. See [Linux operations](docs/LINUX_SERVER.md) for service,
certificate renewal and maintenance details.
