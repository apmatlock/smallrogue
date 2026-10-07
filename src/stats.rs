//! Counts kept over a run, shown on the death screen: what was killed,
//! damage dealt and taken, how the player went down, what was used.

use std::collections::BTreeMap;

use crate::item::WeaponKind;

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// Kills by monster name.
    pub kills: BTreeMap<&'static str, u32>,
    /// The kill worth the most experience: its name and the depth.
    pub toughest_kill: Option<Kill>,
    pub hits: u32,
    pub misses: u32,
    pub damage_dealt: u32,
    pub damage_taken: u32,
    pub stairs_taken: u32,
    pub trapdoor_falls: u32,
    pub traps_sprung: u32,
    pub items_picked_up: u32,
    /// Artifacts picked up, each counted once even if lost and found
    /// again.
    pub artifacts_found: Vec<WeaponKind>,
    pub potions_drunk: u32,
    pub scrolls_read: u32,
    pub meals_eaten: u32,
    /// Real time spent playing, not counting long idle gaps. Kept by
    /// the interface, since the game itself has no clock.
    pub seconds_played: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kill {
    pub name: &'static str,
    pub xp: u32,
    pub depth: u32,
}

impl Stats {
    pub fn record_kill(&mut self, kill: Kill) {
        *self.kills.entry(kill.name).or_default() += 1;
        if self.toughest_kill.is_none_or(|t| kill.xp > t.xp) {
            self.toughest_kill = Some(kill);
        }
    }

    pub fn total_kills(&self) -> u32 {
        self.kills.values().sum()
    }

    /// The monster killed most often, and how many times. Ties go to
    /// the name first in alphabetical order, so the answer is stable.
    pub fn most_killed(&self) -> Option<(&'static str, u32)> {
        self.kills
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(&name, &count)| (name, count))
    }

    /// Time played as "M:SS", or "H:MM:SS" from an hour on.
    pub fn time_played(&self) -> String {
        format_duration(self.seconds_played)
    }

    /// Percent of the player's attacks that hit, if any were made.
    pub fn accuracy(&self) -> Option<u32> {
        let swings = self.hits + self.misses;
        (swings > 0).then(|| self.hits * 100 / swings)
    }
}

/// Seconds as "M:SS", or "H:MM:SS" from an hour on.
pub fn format_duration(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_a_clock() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(754), "12:34");
        assert_eq!(format_duration(3_723), "1:02:03");
    }

    fn kill(name: &'static str, xp: u32) -> Kill {
        Kill { name, xp, depth: 1 }
    }

    #[test]
    fn kills_are_counted_by_name() {
        let mut s = Stats::default();
        for k in [kill("rat", 1), kill("orc", 5), kill("rat", 1)] {
            s.record_kill(k);
        }
        assert_eq!(s.total_kills(), 3);
        assert_eq!(s.most_killed(), Some(("rat", 2)));
        assert_eq!(s.toughest_kill.unwrap().name, "orc");
    }

    #[test]
    fn most_killed_ties_go_to_the_first_name() {
        let mut s = Stats::default();
        s.record_kill(kill("rat", 1));
        s.record_kill(kill("bat", 1));
        assert_eq!(s.most_killed(), Some(("bat", 1)));
    }

    #[test]
    fn accuracy_needs_a_swing() {
        let mut s = Stats::default();
        assert_eq!(s.accuracy(), None);
        s.hits = 3;
        s.misses = 1;
        assert_eq!(s.accuracy(), Some(75));
    }
}
