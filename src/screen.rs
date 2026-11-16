//! A small curses-style window layer drawn with Ratatui.
//!
//! The game was written for ncurses: windows that keep what was written to
//! them, attributes switched on and off, `box_()`, and a blocking `getch()`.
//! This module keeps that shape, with the same function names, so the rest
//! of the code reads as it did. Underneath, each window is a plain grid of
//! cells, and the visible windows are composed into one Ratatui frame,
//! oldest refresh first, whenever the screen is drawn.
//!
//! The game draws on a fixed 80x25 screen (`LINES()` x `COLS()`), which
//! sits in the middle of the terminal and follows it when it is resized.
//! `on_resize()` lets the game rearrange its windows for small terminals.
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier, Style};
use ratatui::{DefaultTerminal, Frame};
#[cfg(test)]
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

/// Attribute bits plus a color pair number, as in curses
pub type attr_t = u32;

/// A character, as in curses
pub type chtype = u32;

/// A handle to a window
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WINDOW(usize);

pub const A_NORMAL: attr_t = 0;
pub const A_BOLD: attr_t = 1;
pub const A_DIM: attr_t = 1 << 1;
pub const A_REVERSE: attr_t = 1 << 2;
const A_COLOR: attr_t = 0xff << 8;

pub const COLOR_BLACK: i16 = 0;
pub const COLOR_RED: i16 = 1;
pub const COLOR_GREEN: i16 = 2;
pub const COLOR_YELLOW: i16 = 3;
pub const COLOR_BLUE: i16 = 4;
pub const COLOR_MAGENTA: i16 = 5;
pub const COLOR_CYAN: i16 = 6;
pub const COLOR_WHITE: i16 = 7;

/// Key codes, as in curses
pub const KEY_DOWN: i32 = 0o402;
pub const KEY_UP: i32 = 0o403;
pub const KEY_LEFT: i32 = 0o404;
pub const KEY_RIGHT: i32 = 0o405;
pub const KEY_BACKSPACE: i32 = 0o407;
/// Any other special key (function keys, Home, ...)
pub const KEY_OTHER: i32 = 0o777;

/// The size of the screen the game draws on
const SCREEN_LINES: i32 = 25;
const SCREEN_COLS: i32 = 80;

/// The smallest terminal the game is playable in
const MIN_LINES: u16 = 24;
const MIN_COLS: u16 = 80;

/// Attribute bits for a color pair
pub fn COLOR_PAIR(n: i16) -> attr_t {
    ((n as attr_t) << 8) & A_COLOR
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CURSOR_VISIBILITY {
    CURSOR_INVISIBLE,
    CURSOR_VISIBLE,
}

/// A window. A derived window (`derwin()`) has no cells of its own: it
/// writes into its parent's, like a curses subwindow.
struct Win {
    lines: i32,
    cols: i32,
    /// Position on the screen, or in the parent for a derived window
    y: i32,
    x: i32,
    parent: Option<usize>,
    cells: Vec<(char, attr_t)>,
    cury: i32,
    curx: i32,
    attr: attr_t,
    scroll: bool,
    /// Shown on the screen since its last refresh
    shown: bool,
    /// Refresh order, the last refreshed window is on top
    z: u64,
}

const BLANK: (char, attr_t) = (' ', A_NORMAL);

/// All windows and the terminal they are drawn on
struct Screen {
    wins: Vec<Option<Win>>,
    pairs: [(i16, i16); 64],
    color: bool,
    cursor: bool,
    /// The window whose cursor is the screen cursor
    focus: Option<usize>,
    z: u64,
    term: Option<Term>,
}

/// Where the screen is drawn and the keys come from
enum Term {
    Real(DefaultTerminal),
    /// A terminal in memory, with keys from a function that sees the screen
    #[cfg(test)]
    Test(Terminal<TestBackend>, TestInput),
}

#[cfg(test)]
type TestInput = Box<dyn FnMut(&Buffer) -> Event>;

/// Called with the terminal's lines and columns when its size changes
type ResizeHook = Box<dyn Fn(i32, i32)>;

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
    static RESIZE_HOOK: RefCell<Option<ResizeHook>> = const { RefCell::new(None) };
}

/// Run a function on the screen
fn with<R>(f: impl FnOnce(&mut Screen) -> R) -> R {
    SCREEN.with(|s| f(s.borrow_mut().as_mut().expect("initscr() not called")))
}

impl Screen {
    fn win(&self, w: WINDOW) -> &Win {
        self.wins[w.0].as_ref().expect("window was deleted")
    }

    fn win_mut(&mut self, w: WINDOW) -> &mut Win {
        self.wins[w.0].as_mut().expect("window was deleted")
    }

    /// The window that holds the cells, and the offset into it
    fn root(&self, w: WINDOW) -> (usize, i32, i32) {
        let mut id = w.0;
        let (mut oy, mut ox) = (0, 0);

        while let Some(parent) = self.win(WINDOW(id)).parent {
            let win = self.win(WINDOW(id));
            oy += win.y;
            ox += win.x;
            id = parent;
        }

        (id, oy, ox)
    }

    /// Set a cell given window coordinates
    fn put(&mut self, w: WINDOW, y: i32, x: i32, cell: (char, attr_t)) {
        let win = self.win(w);
        if y < 0 || x < 0 || y >= win.lines || x >= win.cols {
            return;
        }

        let (root, oy, ox) = self.root(w);
        let rw = self.win_mut(WINDOW(root));
        let (ry, rx) = (y + oy, x + ox);

        if ry < rw.lines && rx < rw.cols {
            let i = (ry * rw.cols + rx) as usize;
            rw.cells[i] = cell;
        }
    }

    /// Get a cell given window coordinates
    fn get(&self, w: WINDOW, y: i32, x: i32) -> (char, attr_t) {
        let (root, oy, ox) = self.root(w);
        let rw = self.win(WINDOW(root));
        let (ry, rx) = (y + oy, x + ox);

        if ry < 0 || rx < 0 || ry >= rw.lines || rx >= rw.cols {
            return BLANK;
        }

        rw.cells[(ry * rw.cols + rx) as usize]
    }

    /// Scroll a window's contents up a line
    fn scroll_up(&mut self, w: WINDOW) {
        let (lines, cols) = (self.win(w).lines, self.win(w).cols);

        for y in 0..lines - 1 {
            for x in 0..cols {
                let c = self.get(w, y + 1, x);
                self.put(w, y, x, c);
            }
        }

        for x in 0..cols {
            self.put(w, lines - 1, x, BLANK);
        }
    }

    /// Move the cursor to the next line, scrolling if allowed
    fn newline(&mut self, w: WINDOW) {
        let win = self.win(w);

        if win.cury < win.lines - 1 {
            self.win_mut(w).cury += 1;
        } else if win.scroll {
            self.scroll_up(w);
        }
    }

    /// Add a character at the cursor, like curses `waddch()`
    fn addch(&mut self, w: WINDOW, c: char) {
        if c == '\n' {
            // Clear to the end of the line, then go to the next one
            let win = self.win(w);
            let (y, x0, cols) = (win.cury, win.curx, win.cols);
            for x in x0..cols {
                self.put(w, y, x, BLANK);
            }

            self.win_mut(w).curx = 0;
            self.newline(w);
            return;
        }

        let win = self.win(w);
        let (y, x, attr) = (win.cury, win.curx, win.attr);
        self.put(w, y, x, (c, attr));

        let win = self.win_mut(w);
        win.curx += 1;

        if win.curx >= win.cols {
            win.curx = 0;
            self.newline(w);
        }
    }

    /// Draw the shown windows onto a frame
    fn compose(&self, frame: &mut Frame) {
        let area = frame.area();

        // The game's screen sits in the middle of the terminal, or at the
        // top left when the terminal is smaller
        let oy = ((area.height as i32 - SCREEN_LINES) / 2).max(0);
        let ox = ((area.width as i32 - SCREEN_COLS) / 2).max(0);

        let mut order: Vec<&Win> = self.wins.iter().flatten().filter(|w| w.shown).collect();
        order.sort_by_key(|w| w.z);

        let buf = frame.buffer_mut();

        for win in order {
            for y in 0..win.lines {
                for x in 0..win.cols {
                    let (sy, sx) = (oy + win.y + y, ox + win.x + x);

                    if sy < 0 || sx < 0 || sy >= area.height as i32 || sx >= area.width as i32 {
                        continue;
                    }

                    let (ch, attr) = win.cells[(y * win.cols + x) as usize];

                    if let Some(cell) = buf.cell_mut(Position::new(sx as u16, sy as u16)) {
                        cell.reset();
                        cell.set_char(ch);
                        cell.set_style(self.style(attr));
                    }
                }
            }
        }

        if self.cursor
            && let Some(id) = self.focus
            && self.wins[id].is_some()
        {
            let (root, ry, rx) = self.root(WINDOW(id));
            let (r, w) = (self.win(WINDOW(root)), self.win(WINDOW(id)));
            let (sy, sx) = (oy + r.y + ry + w.cury, ox + r.x + rx + w.curx);

            if sy >= 0 && sx >= 0 && sy < area.height as i32 && sx < area.width as i32 {
                frame.set_cursor_position(Position::new(sx as u16, sy as u16));
            }
        }

        if area.width < MIN_COLS || area.height < MIN_LINES {
            let msg = format!(
                " Please make the terminal at least {}x{} ",
                MIN_COLS, MIN_LINES
            );
            let x = (area.width as i32 - msg.len() as i32).max(0) / 2;
            let y = area.height.saturating_sub(1);
            let style = Style::default().add_modifier(Modifier::REVERSED);
            frame
                .buffer_mut()
                .set_stringn(x as u16, y, &msg, area.width as usize, style);
        }
    }

    /// Turn attributes into a Ratatui style
    ///
    /// Bold colors are drawn in the bright variant of the color, as most
    /// terminals did with curses, so the map looks the same everywhere.
    /// Color pairs keep the terminal's own background.
    fn style(&self, attr: attr_t) -> Style {
        let mut style = Style::default();
        let pair = ((attr & A_COLOR) >> 8) as usize;
        let bold = attr & A_BOLD != 0;

        if self.color && pair != 0 {
            let (fg, _bg) = self.pairs[pair];
            style = style.fg(color(fg, bold));
        }

        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if attr & A_DIM != 0 {
            style = style.add_modifier(Modifier::DIM);
        }
        if attr & A_REVERSE != 0 {
            style = style.add_modifier(Modifier::REVERSED);
        }

        style
    }

    /// Draw the screen
    fn draw(&mut self) {
        if let Some(mut term) = self.term.take() {
            match &mut term {
                Term::Real(t) => {
                    let _ = t.draw(|frame| self.compose(frame));
                }
                #[cfg(test)]
                Term::Test(t, _) => {
                    let _ = t.draw(|frame| self.compose(frame));
                }
            }
            self.term = Some(term);
        }
    }
}

/// A curses color number as a Ratatui color
fn color(c: i16, bright: bool) -> Color {
    match (c, bright) {
        (COLOR_BLACK, false) => Color::Black,
        (COLOR_RED, false) => Color::Red,
        (COLOR_GREEN, false) => Color::Green,
        (COLOR_YELLOW, false) => Color::Yellow,
        (COLOR_BLUE, false) => Color::Blue,
        (COLOR_MAGENTA, false) => Color::Magenta,
        (COLOR_CYAN, false) => Color::Cyan,
        (COLOR_WHITE, false) => Color::Gray,
        (COLOR_BLACK, true) => Color::DarkGray,
        (COLOR_RED, true) => Color::LightRed,
        (COLOR_GREEN, true) => Color::LightGreen,
        (COLOR_YELLOW, true) => Color::LightYellow,
        (COLOR_BLUE, true) => Color::LightBlue,
        (COLOR_MAGENTA, true) => Color::LightMagenta,
        (COLOR_CYAN, true) => Color::LightCyan,
        _ => Color::White,
    }
}

/// Start up the screen: raw mode, the alternate screen, no cursor
pub fn initscr() {
    start(Term::Real(ratatui::init()));
}

/// Start up a screen in memory, `cols` x `lines`, for tests: `input` is
/// called for each event getch() waits for, with the screen as drawn
#[cfg(test)]
pub fn initscr_test(cols: u16, lines: u16, input: impl FnMut(&Buffer) -> Event + 'static) {
    let term = Terminal::new(TestBackend::new(cols, lines)).unwrap();
    start(Term::Test(term, Box::new(input)));
}

fn start(term: Term) {
    SCREEN.with(|s| {
        *s.borrow_mut() = Some(Screen {
            wins: Vec::new(),
            pairs: [(COLOR_WHITE, COLOR_BLACK); 64],
            color: false,
            cursor: false,
            focus: None,
            z: 0,
            term: Some(term),
        })
    });
}

/// Put the terminal back the way it was
pub fn endwin() {
    let screen = SCREEN.with(|s| s.borrow_mut().take());

    if let Some(Screen {
        term: Some(Term::Real(_)),
        ..
    }) = screen
    {
        ratatui::restore();
    }
}

/// True if colors can be shown; `NO_COLOR` turns them off
pub fn has_colors() -> bool {
    std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
}

pub fn start_color() {
    with(|s| s.color = true);
}

pub fn init_pair(pair: i16, fg: i16, bg: i16) {
    with(|s| s.pairs[pair as usize] = (fg, bg));
}

/// The screen height the game draws for
pub fn LINES() -> i32 {
    SCREEN_LINES
}

/// The screen width the game draws for
pub fn COLS() -> i32 {
    SCREEN_COLS
}

pub fn curs_set(v: CURSOR_VISIBILITY) {
    with(|s| s.cursor = v == CURSOR_VISIBILITY::CURSOR_VISIBLE);
}

/// Clear the whole screen on the next `refresh()`
pub fn erase() {}

/// Clear the whole screen on the next `refresh()`
pub fn clear() {}

/// Show the (always empty) background screen, covering every window
pub fn refresh() {
    with(|s| {
        for w in s.wins.iter_mut().flatten() {
            w.shown = false;
        }
        s.focus = None;
    });
}

/// Make a new window
pub fn newwin(lines: i32, cols: i32, y: i32, x: i32) -> WINDOW {
    let win = Win {
        lines,
        cols,
        y,
        x,
        parent: None,
        cells: vec![BLANK; (lines * cols).max(0) as usize],
        cury: 0,
        curx: 0,
        attr: A_NORMAL,
        scroll: false,
        shown: false,
        z: 0,
    };

    with(|s| {
        s.wins.push(Some(win));
        WINDOW(s.wins.len() - 1)
    })
}

/// Make a window inside another one, sharing its cells
pub fn derwin(orig: WINDOW, lines: i32, cols: i32, y: i32, x: i32) -> WINDOW {
    let win = Win {
        lines,
        cols,
        y,
        x,
        parent: Some(orig.0),
        cells: Vec::new(),
        cury: 0,
        curx: 0,
        attr: A_NORMAL,
        scroll: false,
        shown: false,
        z: 0,
    };

    with(|s| {
        s.wins.push(Some(win));
        WINDOW(s.wins.len() - 1)
    })
}

/// Delete a window; it disappears from the screen
pub fn delwin(w: WINDOW) {
    with(|s| {
        s.wins[w.0] = None;
        if s.focus == Some(w.0) {
            s.focus = None;
        }
    });
}

/// Move a window on the screen
pub fn mvwin(w: WINDOW, y: i32, x: i32) {
    with(|s| {
        let win = s.win_mut(w);
        win.y = y;
        win.x = x;
    });
}

pub fn scrollok(w: WINDOW, ok: bool) {
    with(|s| s.win_mut(w).scroll = ok);
}

/// Blank a window and home its cursor
pub fn werase(w: WINDOW) {
    with(|s| {
        let (lines, cols) = (s.win(w).lines, s.win(w).cols);
        for y in 0..lines {
            for x in 0..cols {
                s.put(w, y, x, BLANK);
            }
        }
        let win = s.win_mut(w);
        win.cury = 0;
        win.curx = 0;
    });
}

pub fn wmove(w: WINDOW, y: i32, x: i32) {
    with(|s| {
        let win = s.win_mut(w);
        win.cury = y.clamp(0, (win.lines - 1).max(0));
        win.curx = x.clamp(0, (win.cols - 1).max(0));
    });
}

pub fn waddch(w: WINDOW, c: chtype) -> i32 {
    let c = char::from_u32(c).unwrap_or('?');
    with(|s| s.addch(w, c));
    0
}

pub fn waddstr(w: WINDOW, st: &str) -> i32 {
    with(|s| {
        for c in st.chars() {
            s.addch(w, c);
        }
    });
    0
}

pub fn mvwaddstr(w: WINDOW, y: i32, x: i32, s: &str) -> i32 {
    wmove(w, y, x);
    waddstr(w, s)
}

pub fn wattr_on(w: WINDOW, attr: attr_t) {
    with(|s| {
        let win = s.win_mut(w);
        if attr & A_COLOR != 0 {
            win.attr &= !A_COLOR;
        }
        win.attr |= attr;
    });
}

pub fn wattr_off(w: WINDOW, attr: attr_t) {
    with(|s| {
        let win = s.win_mut(w);
        if attr & A_COLOR != 0 {
            win.attr &= !A_COLOR;
        }
        win.attr &= !(attr & !A_COLOR);
    });
}

pub fn wattron(w: WINDOW, attr: attr_t) {
    wattr_on(w, attr);
}

pub fn wattroff(w: WINDOW, attr: attr_t) {
    wattr_off(w, attr);
}

pub fn wattr_set(w: WINDOW, attr: attr_t, pair: i16) {
    with(|s| s.win_mut(w).attr = (attr & !A_COLOR) | COLOR_PAIR(pair));
}

/// Draw a line border round a window in its current attributes
pub fn box_(w: WINDOW, _v: chtype, _h: chtype) {
    with(|s| {
        let win = s.win(w);
        let (lines, cols, a) = (win.lines, win.cols, win.attr);

        for x in 1..cols - 1 {
            s.put(w, 0, x, ('─', a));
            s.put(w, lines - 1, x, ('─', a));
        }
        for y in 1..lines - 1 {
            s.put(w, y, 0, ('│', a));
            s.put(w, y, cols - 1, ('│', a));
        }
        s.put(w, 0, 0, ('┌', a));
        s.put(w, 0, cols - 1, ('┐', a));
        s.put(w, lines - 1, 0, ('└', a));
        s.put(w, lines - 1, cols - 1, ('┘', a));
    });
}

/// Show a window on top of the others
pub fn wrefresh(w: WINDOW) {
    with(|s| {
        let (root, _, _) = s.root(w);
        s.z += 1;
        let z = s.z;
        let win = s.win_mut(WINDOW(root));
        win.shown = true;
        win.z = z;
        s.focus = Some(w.0);
    });
}

/// Show a window on top of the others
pub fn redrawwin(w: WINDOW) {
    wrefresh(w);
}

pub fn getcury(w: WINDOW) -> i32 {
    with(|s| s.win(w).cury)
}

pub fn getmaxx(w: WINDOW) -> i32 {
    with(|s| s.win(w).cols)
}

/// Draw the screen now
pub fn doupdate() {
    with(|s| s.draw());
}

/// A key event as a curses key code, or None for keys to ignore
fn key_code(ev: &Event) -> Option<i32> {
    let Event::Key(key) = ev else {
        return None;
    };

    if key.kind == KeyEventKind::Release {
        return None;
    }

    let code = match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl-C quits at once, as it did under curses
            endwin();
            std::process::exit(0);
        }
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // A control character, as a terminal sends it
            (c.to_ascii_lowercase() as i32) & 0x1f
        }
        KeyCode::Char(c) => c as i32,
        KeyCode::Enter => '\n' as i32,
        KeyCode::Tab => '\t' as i32,
        KeyCode::Esc => 27,
        KeyCode::Backspace => KEY_BACKSPACE,
        KeyCode::Up => KEY_UP,
        KeyCode::Down => KEY_DOWN,
        KeyCode::Left => KEY_LEFT,
        KeyCode::Right => KEY_RIGHT,
        KeyCode::Modifier(_) | KeyCode::CapsLock | KeyCode::NumLock | KeyCode::ScrollLock => {
            return None;
        }
        _ => KEY_OTHER,
    };

    Some(code)
}

/// Set the function called when the terminal is resized, and call it now
pub fn on_resize(hook: impl Fn(i32, i32) + 'static) {
    RESIZE_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));

    let size = with(|s| match s.term.as_ref() {
        Some(Term::Real(t)) => t.size().ok(),
        #[cfg(test)]
        Some(Term::Test(t, _)) => t.size().ok(),
        None => None,
    });

    if let Some(size) = size {
        resized(size.height, size.width);
    }
}

/// Tell the game the terminal has a new size
fn resized(lines: u16, cols: u16) {
    RESIZE_HOOK.with(|h| {
        if let Some(hook) = h.borrow().as_ref() {
            hook(lines as i32, cols as i32);
        }
    });
}

/// Wait for the next terminal event
fn read_event() -> Option<Event> {
    #[cfg(test)]
    {
        let ev = with(|s| match s.term.as_mut() {
            Some(Term::Test(t, input)) => {
                let ev = input(t.backend().buffer());
                if let Event::Resize(cols, lines) = ev {
                    t.backend_mut().resize(cols, lines);
                }
                Some(ev)
            }
            _ => None,
        });

        if ev.is_some() {
            return ev;
        }
    }

    event::read().ok()
}

/// Draw the screen and wait for a key
///
/// A resize is not a key: the screen is laid out again and redrawn, and
/// getch() goes on waiting.
pub fn getch() -> i32 {
    loop {
        doupdate();

        let Some(ev) = read_event() else {
            continue;
        };

        if let Event::Resize(cols, lines) = ev {
            resized(lines, cols);
            continue;
        }

        if let Some(code) = key_code(&ev) {
            return code;
        }
    }
}

/// Read a line of up to `n` characters, echoed at the window's cursor
pub fn wgetnstr(w: WINDOW, s: &mut String, n: i32) -> i32 {
    s.clear();

    loop {
        with(|sc| sc.focus = Some(w.0));

        match getch() {
            10 | 13 => break,
            KEY_BACKSPACE | 8 | 127 => {
                if s.pop().is_some() {
                    with(|sc| {
                        let win = sc.win_mut(w);
                        win.curx -= 1;
                        let (y, x) = (win.cury, win.curx);
                        sc.put(w, y, x, BLANK);
                    });
                }
            }
            c if (32..127).contains(&c) && (s.len() as i32) < n => {
                s.push(c as u8 as char);
                waddch(w, c as chtype);
            }
            _ => (),
        }
    }

    0
}
