//! Session logging for bug reports (Asciiverse)
//!
//! `RUST_LOG=debug wizards-castle 2>wizards-castle.log` records every key
//! as the terminal delivers it, the game's state changes and messages, and
//! a snapshot of the game once a second while it waits for a key. Without
//! `RUST_LOG`, only errors are logged, and the game logs none.

use std::cell::RefCell;

use log::Level;

use wizardscastle::player::Stat;

use crate::G;

thread_local! {
    static SNAPSHOT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Start the logger, writing to stderr
pub fn init() {
    let _ = env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .try_init();
}

/// True if snapshots are wanted, so getch() should wake up for them
pub fn snapshots_enabled() -> bool {
    log::log_enabled!(Level::Debug)
}

/// Log the latest snapshot; called once a second while waiting for a key
pub fn tick() {
    SNAPSHOT.with(|s| {
        let s = s.borrow();
        if s.is_empty() {
            log::debug!("snapshot: no game yet");
        } else {
            log::debug!("snapshot {}", s);
        }
    });
}

impl G {
    /// Remember the state of the game for the next snapshot
    pub fn update_snapshot(&self) {
        if !snapshots_enabled() {
            return;
        }

        let g = &self.game;

        let s = format!(
            "state={:?} statmode={:?} pos=({},{}) level={} room=\"{}\" race={} \
             ST={} IQ={} DX={} GP={} FL={} T={} armor={} weapon={} lamp={} \
             blind={} runestaff={} orb={} treasures={}",
            g.state(),
            self.statmode,
            g.player_x() + 1,
            g.player_y() + 1,
            g.player_z() + 1,
            G::room_name(g.room_at_player().room_type()),
            self.race_name(),
            g.player_stat(Stat::Strength),
            g.player_stat(Stat::Intelligence),
            g.player_stat(Stat::Dexterity),
            g.player_gp(),
            g.player_flares(),
            g.turn(),
            G::armor_name(g.player_armor_type()),
            G::weapon_name(g.player_weapon_type()),
            g.player_has_lamp(),
            g.player_is_blind(),
            g.player_has_runestaff(),
            g.player_has_orb_of_zot(),
            g.player_get_treasures().len(),
        );

        SNAPSHOT.with(|snap| *snap.borrow_mut() = s);
    }
}
