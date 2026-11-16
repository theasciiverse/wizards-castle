# The Ratatui port, for upstream

Your README says ncurses-rs is frozen and that the next step would be a
Ratatui port. This is one, offered as a contribution, MIT like the rest:
take it whole, in part, or as a starting point. It moves the game from
ncurses to Ratatui and crossterm without touching the game: the same
windows, text, prompts, colours and key handling, and the game library
(`wizardscastle` v0.9.8) is unchanged.

## What it changes

- **A curses-style window layer, `src/screen.rs`.** The game is written
  against curses: windows that keep their contents, attributes switched
  on and off, `box_()`, and a blocking `getch()` in many places. Turning
  that into Ratatui's draw-everything-each-frame style would mean
  rewriting every screen as state plus a draw function. Instead,
  `screen.rs` offers the calls the game makes, with the same names and
  arguments (`newwin`, `derwin`, `werase`, `wmove`, `waddstr`, `waddch`,
  `wattr_on`/`wattr_off`, `box_`, `wrefresh`, `getch`, `wgetnstr`, ...),
  and each window is a grid of cells. On every `getch()` the shown windows
  are composed into one Ratatui frame, last refreshed on top, and Ratatui
  sends only what changed. A derived window writes into its parent's
  cells, as a curses subwindow does (the log's `loginner`). About 800
  lines with comments; everything else changes only its `use` line.
- **Resize works.** The 80x25 screen sits in the middle of the terminal and
  follows it when resized. Nothing is lost, since every window keeps its
  cells, and a resize is no longer a key, so it can't close a "Press any
  key" box. On an 80x24 terminal (the classic size, one line short of what
  the layout needs) the log window moves up a line over the bottom borders
  of the map and status windows, through a small hook (`on_resize()` in
  `main()`). Below 80x24 the screen is clipped and a line asks for a
  bigger terminal.
- **Windows and macOS** work through crossterm. Key releases, which
  Windows reports, are ignored.
- **Colours**: bold colours draw in the bright variant (yellow becomes
  bright yellow, as in your screenshot), so the map looks the same on
  terminals that don't brighten bold text. Colour pairs keep the terminal's
  own background instead of black, which looks the same on dark terminals
  and better on light ones. `NO_COLOR` turns colours off (`has_colors()`).
- **Small things**: Ctrl-C quits at once with the terminal put back (and a
  panic puts it back too, through `ratatui::init()`); the flare prompt's
  `wgetnstr()` echoes and handles Backspace itself, so the
  `cbreak`/`echo` juggling around it is gone; a stray `println!()` after
  opening a chest is dropped.
- **A headless test**, `src/tests.rs`: `screen.rs` can draw into Ratatui's
  `TestBackend` with keys from a function that sees the screen. A bot
  reads the screen the way a player would and plays the real `G::run()`
  through every screen (character creation, flares, fights, spells,
  bribes, vendors, teleport, death or exit, play again), 400 commands for
  each of four seeds, resizing the terminal along the way. The bot's
  choices are seeded; the library's dice use `rand::rng()` and can't be,
  so each run is a different castle. A seedable RNG in the library would
  make runs repeatable.
- README, man page and copyright box updated (Ratatui; the man page's
  BUGS are replaced by notes on the terminal size and `NO_COLOR`).

## How to review

The port is seven commits on top of your `master` (`8fbe861`), each
building and running:

1. `d456ae7` Add Ratatui as a dependency
2. `30535fd` Add a curses-style window layer drawn with Ratatui
3. `8715378` Read keys with crossterm in the window layer
4. `d914f32` Switch from ncurses to the Ratatui window layer
5. `76f9dc1` Follow the terminal size
6. `3766bd3` Tidy up after the Ratatui port
7. `287a765` Add a headless test: a bot plays seeded games through every screen

`git diff 8fbe861 287a765 -- src ':!src/screen.rs' ':!src/tests.rs'` shows
how little of the game itself moves (26 lines in, 31 out, across 14
files). To try it: `cargo run`, then resize
the terminal mid-game, open the help, resize again; `cargo test` runs the
bot.

Left for you to decide: the crate is still `wizards_castle_ncurses`, the
man page still says "ncurses edition", and the version is still 1.0.11.

## Not part of the offer

The commits after `287a765` are for the Asciiverse catalog only: session
logging (`src/session_log.rs`), a release workflow, the preview recording
and store page drafts (`asciiverse/`), and these notes.
