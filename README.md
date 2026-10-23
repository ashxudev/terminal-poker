# terminal-poker

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

## Stats

Statistics are saved between sessions to `~/.local/share/terminal-poker/stats.json` (Linux) or the platform equivalent.

Tracked stats include:

- **Preflop** — VPIP, PFR, 3-bet frequency
- **Postflop** — C-bet%, fold to c-bet%
- **Showdown** — WTSD (went to showdown), W$SD (won $ at showdown)
- **Overall** — Aggression factor, BB/100 win rate, hands played, biggest pots

Press `S` in-game to view your session and lifetime stats.
