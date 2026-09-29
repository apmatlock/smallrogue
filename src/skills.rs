//! Skills that improve by use, and character levels from experience.
//!
//! Skills: swinging a weapon trains Melee, being attacked trains Dodge,
//! being hit while armored trains Armor, and staying unnoticed by
//! sleeping monsters trains Stealth. Early levels come quickly so a
//! skill grows noticeably within one short run.
//!
//! Levels: kills give experience. Each new level raises maximum health
//! and one attribute.

/// Skills stop improving at this level.
pub const MAX_SKILL: u32 = 10;

/// Maximum health gained on each new character level.
pub const HEALTH_PER_LEVEL: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Melee,
    Dodge,
    Armor,
    Stealth,
}

impl Skill {
    pub const ALL: [Skill; 4] = [Skill::Melee, Skill::Dodge, Skill::Armor, Skill::Stealth];

    pub fn name(self) -> &'static str {
        match self {
            Skill::Melee => "melee",
            Skill::Dodge => "dodge",
            Skill::Armor => "armor",
            Skill::Stealth => "stealth",
        }
    }

    /// What one level of the skill does, for the character sheet.
    pub fn about(self) -> &'static str {
        match self {
            Skill::Melee => "+1 accuracy per level, +1 damage per 2 levels",
            Skill::Dodge => "+1 dodge per level",
            Skill::Armor => "+1 armor per 2 levels, less heavy-armor penalty",
            Skill::Stealth => "sleeping monsters are less likely to wake",
        }
    }

    fn index(self) -> usize {
        Skill::ALL
            .iter()
            .position(|&s| s == self)
            .expect("every skill is in ALL")
    }
}

/// Training points needed to go from `level` to `level + 1`: 10, then
/// 20, 30 and so on, so each level takes a little longer.
pub fn points_for_next(level: u32) -> u32 {
    10 * (level + 1)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    level: [u32; 4],
    /// Progress toward each skill's next level.
    points: [u32; 4],
}

impl Skills {
    pub fn level(&self, skill: Skill) -> u32 {
        self.level[skill.index()]
    }

    /// Progress toward the next level, as (points so far, needed).
    pub fn progress(&self, skill: Skill) -> (u32, u32) {
        let i = skill.index();
        (self.points[i], points_for_next(self.level[i]))
    }

    /// Adds training. Returns the new level if the skill went up.
    pub fn train(&mut self, skill: Skill, amount: u32) -> Option<u32> {
        let i = skill.index();
        if self.level[i] >= MAX_SKILL {
            return None;
        }
        self.points[i] += amount;
        let mut raised = None;
        while self.level[i] < MAX_SKILL && self.points[i] >= points_for_next(self.level[i]) {
            self.points[i] -= points_for_next(self.level[i]);
            self.level[i] += 1;
            raised = Some(self.level[i]);
        }
        if self.level[i] == MAX_SKILL {
            self.points[i] = 0;
        }
        raised
    }
}

/// Total experience needed to reach character `level`: 0 for level 1,
/// then 10, 30, 60, 100, ... (each level needs 10 more than the last).
pub fn xp_for_level(level: u32) -> u32 {
    let n = level.saturating_sub(1);
    10 * n * (n + 1) / 2
}

/// Which attribute a new level raises, in a repeating pattern that
/// suits a fighter: strength, agility, strength, agility, intellect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    Strength,
    Agility,
    Intellect,
}

pub fn attribute_for_level(level: u32) -> Attribute {
    match level % 5 {
        2 | 4 => Attribute::Strength,
        3 | 0 => Attribute::Agility,
        _ => Attribute::Intellect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_level_up_with_rising_costs() {
        let mut s = Skills::default();
        assert_eq!(s.train(Skill::Melee, 9), None);
        assert_eq!(s.train(Skill::Melee, 1), Some(1));
        assert_eq!(s.progress(Skill::Melee), (0, 20));
        // One big lump of training can raise several levels at once.
        assert_eq!(s.train(Skill::Melee, 20 + 30), Some(3));
        assert_eq!(s.level(Skill::Dodge), 0, "skills are separate");
    }

    #[test]
    fn skills_stop_at_the_maximum() {
        let mut s = Skills::default();
        s.train(Skill::Stealth, 10_000);
        assert_eq!(s.level(Skill::Stealth), MAX_SKILL);
        assert_eq!(s.train(Skill::Stealth, 100), None);
    }

    #[test]
    fn experience_curve() {
        let needed: Vec<u32> = (1..=6).map(xp_for_level).collect();
        assert_eq!(needed, [0, 10, 30, 60, 100, 150]);
    }

    #[test]
    fn attribute_pattern_repeats() {
        use Attribute::*;
        let pattern: Vec<Attribute> = (2..=11).map(attribute_for_level).collect();
        assert_eq!(
            pattern,
            [
                Strength, Agility, Strength, Agility, Intellect, Strength, Agility, Strength,
                Agility, Intellect
            ]
        );
    }
}
