//! The player character: position, health and attributes.

use crate::combat::{Attack, Defense};
use crate::geom::Point;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Point,
    pub hp: i32,
    pub max_hp: i32,
    /// Adds melee damage. Will also set carrying capacity.
    pub strength: i32,
    /// Sets accuracy and dodge.
    pub agility: i32,
    /// Will speed up identifying items, and later power magic.
    pub intellect: i32,
}

impl Player {
    /// The one starting background for now: a sturdy melee fighter.
    pub fn fighter(pos: Point) -> Self {
        Self {
            pos,
            hp: 25,
            max_hp: 25,
            strength: 4,
            agility: 3,
            intellect: 2,
        }
    }

    /// Bonus melee damage from strength: +1 at 4, +2 at 6, and so on.
    pub fn strength_bonus(&self) -> i32 {
        (self.strength / 2 - 1).max(0)
    }

    pub fn attack(&self) -> Attack {
        // Until items arrive in milestone 6, the fighter always wields a
        // plain sword doing 1-5 damage.
        let bonus = self.strength_bonus();
        Attack {
            accuracy: 1 + self.agility,
            damage: (1 + bonus, 5 + bonus),
        }
    }

    pub fn defense(&self) -> Defense {
        // ...and always wears leather armor worth 1 point.
        Defense {
            dodge: self.agility,
            armor: 1,
        }
    }
}
