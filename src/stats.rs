//! Counts kept over a run, shown on the death screen: what was killed,
//! damage dealt and taken, how the player went down, what was used.

use std::collections::BTreeMap;

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
    pub potions_drunk: u32,
    pub scrolls_read: u32,
    pub meals_eaten: u32,
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

    /// Percent of the player's attacks that hit, if any were made.
    pub fn accuracy(&self) -> Option<u32> {
        let swings = self.hits + self.misses;
        (swings > 0).then(|| self.hits * 100 / swings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
