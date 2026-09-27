//! Attack resolution, shared by the player and monsters.
//!
//! One attack goes in two steps:
//! 1. Roll to hit. The chance starts at 75% and moves 5% for every
//!    point of difference between accuracy and dodge, always staying
//!    between 5% and 95% so nothing is ever certain.
//! 2. Roll damage in the weapon's range and subtract armor. A hit
//!    always does at least 1 damage, so armor can't make you immune.

use crate::rng::Rng;

const BASE_HIT_PERCENT: i32 = 75;
const PERCENT_PER_POINT: i32 = 5;

/// What an attacker brings to a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attack {
    pub accuracy: i32,
    /// Damage range, inclusive: (min, max).
    pub damage: (i32, i32),
}

/// What a defender brings to a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defense {
    pub dodge: i32,
    pub armor: i32,
}

pub fn hit_chance(accuracy: i32, dodge: i32) -> i32 {
    (BASE_HIT_PERCENT + PERCENT_PER_POINT * (accuracy - dodge)).clamp(5, 95)
}

/// Resolves one attack. Returns the damage dealt, or `None` on a miss.
pub fn resolve(rng: &mut Rng, attack: Attack, defense: Defense) -> Option<i32> {
    if !rng.chance(hit_chance(attack.accuracy, defense.dodge)) {
        return None;
    }
    let (min, max) = attack.damage;
    let roll = rng.range(min, max + 1);
    Some((roll - defense.armor).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_chance_is_clamped() {
        assert_eq!(hit_chance(3, 3), 75);
        assert_eq!(hit_chance(5, 3), 85);
        assert_eq!(hit_chance(50, 0), 95);
        assert_eq!(hit_chance(0, 50), 5);
    }

    #[test]
    fn armor_never_blocks_all_damage() {
        let mut rng = Rng::new(1);
        let attack = Attack {
            accuracy: 100,
            damage: (1, 2),
        };
        let defense = Defense {
            dodge: 0,
            armor: 10,
        };
        for _ in 0..100 {
            if let Some(d) = resolve(&mut rng, attack, defense) {
                assert_eq!(d, 1);
            }
        }
    }

    #[test]
    fn hits_land_about_as_often_as_the_chance_says() {
        let mut rng = Rng::new(2);
        let attack = Attack {
            accuracy: 3,
            damage: (1, 1),
        };
        let defense = Defense { dodge: 3, armor: 0 };
        let hits = (0..10_000)
            .filter(|_| resolve(&mut rng, attack, defense).is_some())
            .count();
        // 75% of 10,000, with room for luck.
        assert!((7_200..7_800).contains(&hits), "{hits} hits");
    }
}
