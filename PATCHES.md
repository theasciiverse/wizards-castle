# Changes from upstream

This is the Asciiverse packaging of [Wizard's Castle, curses
version](https://github.com/beejjorgensen/Wizards-Castle-Ncurses) by Brian
"Beej Jorgensen" Hall (MIT, `LICENSE.md`), a full-screen front end on his
[Wizard's Castle library](https://github.com/beejjorgensen/Wizards-Castle-Rust)
(tag v0.9.8, unchanged). Upstream is the `upstream` remote; this build is
upstream's `master` at `8fbe861` (2026-04-07), version 1.0.11. The
`asciiverse` branch carries the changes below. Our changes are MIT too.

## The Ratatui port (offered upstream, see UPSTREAM.md)

Seven commits, `d456ae7` to `287a765`, each building:

- **ncurses replaced by Ratatui and crossterm.** `src/screen.rs` is a small
  curses-style window layer (the same calls: `newwin`, `derwin`, `waddstr`,
  `wattr_on`, `box_`, `wrefresh`, `getch`, ...) drawn with Ratatui, so the
  rest of the code is unchanged apart from its `use` line and the dropped
  `cbreak`/`noecho`/`keypad`/`set_escdelay` calls. Runs on Linux, macOS and
  Windows.
- **Look kept**: the same windows, text, prompts and colours. Bold colours
  draw in their bright variant (as most terminals showed them under
  curses), colour pairs keep the terminal's own background instead of
  forcing black, and `NO_COLOR` turns colours off.
- **Resize fixed**: the 80x25 screen sits in the middle of the terminal and
  follows it when resized, keeping everything drawn; a resize no longer
  closes "Press any key" boxes. On an 80x24 terminal the log window moves
  up a line over the bottom borders of the map and status windows, so all
  of it shows. Smaller than 80x24, a line asks for a bigger terminal.
- **Ctrl-C** quits at once with the terminal put back; the terminal is
  also put back after a panic.
- A stray `println!()` after opening a chest (a newline behind curses'
  back) is gone.
- README, man page (BUGS replaced by notes on size and `NO_COLOR`) and the
  copyright box say Ratatui.
- **Headless test** (`src/tests.rs`): seeded bots play the real game through
  every screen in an in-memory terminal, 400 commands each, resizing along
  the way.

## Asciiverse only

- **Session logging** (`src/session_log.rs`, small calls elsewhere) with
  `log` and `env_logger`: `RUST_LOG=debug wizards_castle_ncurses 2>wc.log`
  gives every terminal event as it arrives, every message the game prints,
  combat starts and ends, game start and end, status-panel modes, and a
  snapshot of the game once a second while it waits for a key. Without
  `RUST_LOG`, nothing.
- The game saves nothing (no scores or settings), so there is no state
  folder.

## Packaging (new files)

- `.github/workflows/asciiverse-release.yml`: av archives for Linux (static
  musl, x86-64 and ARM64, built in Alpine), macOS (Apple silicon on
  macos-15, Intel on macos-15-intel) and Windows (x86-64 and ARM64), plus
  SHA256SUMS, with Rust pinned to 1.93.1. The binary is named
  `wizards-castle` in the archives.
- `asciiverse/record-preview.sh`, `asciiverse/drive.py`: the catalog
  preview, played by a script in a tmux pane.
- `asciiverse/wizards-castle.toml`, `asciiverse/wizards-castle.md`,
  `asciiverse/wizards-castle.cast`: the catalog manifest, store page and
  preview, drafts for hub.
- `UPSTREAM.md`: the port, as offered to upstream.
