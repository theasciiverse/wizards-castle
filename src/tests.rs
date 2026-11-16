//! Headless play: a bot plays the whole game, every screen of it, in an
//! in-memory terminal, reading the screen the way a player would.
//!
//! The bot's own choices come from a seeded RNG. The game's dice are the
//! library's own, which can't be seeded, so each run is a different
//! castle; the bot copes with whatever comes up.

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use std::collections::VecDeque;

use crate::screen::*;
use crate::{G, Opts};

/// Give up if a run takes more keys than this
const MAX_KEYS: u32 = 40_000;

/// The screen as lines of text
fn screen_text(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (0..area.height)
        .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// The first number after `after` on the screen
fn number_after(text: &[String], after: &str) -> Option<u32> {
    text.iter().find_map(|line| {
        let i = line.find(after)? + after.len();
        let digits: String = line[i..]
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits.parse().ok()
    })
}

struct Bot {
    rng: StdRng,
    /// Commands given on the map so far, and how many to give
    moves: u32,
    max_moves: u32,
    /// Games finished (died, left or quit)
    games: u32,
    /// Keys given in combat
    combat: u32,
    keys: u32,
    queue: VecDeque<Event>,
    sizes: [(u16, u16); 5],
}

fn key(c: char) -> Event {
    let code = match c {
        '\n' => KeyCode::Enter,
        '\x1b' => KeyCode::Esc,
        c => KeyCode::Char(c),
    };
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

impl Bot {
    fn new(seed: u64, max_moves: u32) -> Bot {
        Bot {
            rng: StdRng::seed_from_u64(seed),
            moves: 0,
            max_moves,
            games: 0,
            combat: 0,
            keys: 0,
            queue: VecDeque::new(),
            sizes: [(80, 25), (80, 24), (100, 30), (132, 43), (70, 20)],
        }
    }

    fn pick(&mut self, choices: &str) -> char {
        let v: Vec<char> = choices.chars().collect();
        v[self.rng.random_range(0..v.len())]
    }

    fn quitting(&self) -> bool {
        self.moves >= self.max_moves
    }

    /// The next event, given the screen as drawn
    fn next(&mut self, buf: &Buffer) -> Event {
        self.keys += 1;

        let text = screen_text(buf);
        let has = |s: &str| text.iter().any(|l| l.contains(s));

        assert!(
            self.keys < MAX_KEYS,
            "bot stuck after {} keys, {} moves:\n{}",
            self.keys,
            self.moves,
            text.join("\n")
        );

        if let Some(ev) = self.queue.pop_front() {
            return ev;
        }

        // A player makes a terminal that is too small bigger again
        if has("Please make the terminal at least") {
            return Event::Resize(80, 24);
        }

        // Now and then the terminal changes size
        if self.rng.random_range(0..200) == 0 {
            let (cols, lines) = self.sizes[self.rng.random_range(0..self.sizes.len())];
            return Event::Resize(cols, lines);
        }

        let c = if has("Press any key") {
            ' '
        } else if has("You may be an:") {
            self.pick("EDMH")
        } else if has("Which sex do you prefer?") {
            self.pick("FM")
        } else if has("How many points do you add") {
            let max = number_after(&text, "to [").unwrap_or(0);
            char::from_digit(self.rng.random_range(0..=max), 10).unwrap()
        } else if has("list of armor you can buy") {
            self.pick("PCLN")
        } else if has("list of weapons you can buy") {
            self.pick("SMDN")
        } else if has("Want to buy a lamp") {
            self.pick("YN")
        } else if has("How many do you want?") {
            let gps = number_after(&text, "you have").unwrap_or(0);
            // Sometimes too many, to see the error box
            let n = self.rng.random_range(0..=gps.min(30) + 2);
            for d in n.to_string().chars() {
                self.queue.push_back(key(d));
            }
            '\n'
        } else if has("Do you really want to exit") {
            if self.quitting() || self.rng.random_range(0..4) == 0 {
                'Y'
            } else {
                'N'
            }
        } else if has("Do you really want to quit") {
            if self.quitting() { 'Y' } else { 'N' }
        } else if has("[P]lay again or [Q]uit") {
            self.games += 1;
            if self.quitting() { 'Q' } else { 'P' }
        } else if has("[C]ontinue") {
            'C'
        } else if has("I want The") {
            self.pick("YN")
        } else if has("What do you want to sell?") {
            self.pick("12345678NN\x1b")
        } else if has("What equipment would you like") {
            self.pick("LCPDMSANNN\x1b")
        } else if has("You already have") {
            self.pick("YN")
        } else if has("buy some potions") {
            self.pick("SIDNN\x1b")
        } else if has("Teleport to where?") {
            self.pick("12345678N")
        } else if has("Shine lamp which way?") {
            self.pick("NSWEX")
        } else if has("[F]ireball") {
            self.pick("WFDN")
        } else if has("to retreat") {
            self.moves += 1;
            self.combat += 1;
            self.pick("AAAAAABCNSWE")
        } else if has("level") || has("** BLIND **") {
            // On the map
            self.moves += 1;
            if self.quitting() {
                'Q'
            } else {
                self.pick("NNNSSSWWWEEEUDDOGFLITPRHCAQ?")
            }
        } else {
            // Something the bot doesn't know: any key
            self.pick("NY C")
        };

        key(c)
    }
}

/// Play `max_moves` commands with a bot seeded with `seed`, at 80x25 to
/// start with, then quit the way a player would
fn play(seed: u64, max_moves: u32) -> (u32, u32, u32) {
    use std::cell::RefCell;
    use std::rc::Rc;

    let bot = Rc::new(RefCell::new(Bot::new(seed, max_moves)));
    let b = Rc::clone(&bot);

    initscr_test(80, 25, move |buf| b.borrow_mut().next(buf));
    start_color();

    let mut g = G::new(Opts {
        discover_all: false,
        force_bw: false,
        give_orb_of_zot: false,
        give_runestaff: false,
        locations: false,
    });

    let logwin = g.logwin;
    on_resize(move |lines, _cols| {
        let y = if lines < LINES() { LINES() - 9 } else { 17 };
        mvwin(logwin, y, 0);
    });

    g.run();

    endwin();

    let bot = bot.borrow();
    (bot.moves, bot.games, bot.combat)
}

#[test]
fn bot_plays_seeded_games() {
    let mut combat = 0;

    for seed in [1, 2, 3, 1980] {
        let (moves, games, c) = play(seed, 400);
        assert!(moves >= 400, "seed {}: only {} moves", seed, moves);
        assert!(games >= 1, "seed {}: no game finished", seed);
        combat += c;
    }

    assert!(combat > 0, "no fights in 1600 moves");
}
