//! What the player knows about items this run, and so how every item
//! looks and is named.
//!
//! Each run shuffles potion appearances, ring gems and scroll titles,
//! so "a murky potion" is healing in one run and decay in the next.
//! Drinking, reading, wearing or identifying items teaches the player
//! what they are. Weapon and armor *kinds* are always obvious; only
//! their enchantments are hidden, and that knowledge lives on each
//! item (`Item::known`) because every sword is different.

use std::collections::HashSet;

use crate::frame::Rgb;
use crate::item::{Item, ItemKind, PotionKind, RingKind, ScrollKind};
use crate::rng::Rng;
use crate::text::article;

/// Potion appearances, shuffled onto the potion kinds each run.
const POTION_LOOKS: [(&str, Rgb); 8] = [
    ("murky", Rgb(135, 120, 85)),
    ("crimson", Rgb(205, 45, 55)),
    ("bubbling", Rgb(90, 190, 160)),
    ("inky", Rgb(120, 105, 150)),
    ("milky", Rgb(225, 225, 215)),
    ("amber", Rgb(220, 150, 50)),
    ("violet", Rgb(165, 95, 205)),
    ("glowing", Rgb(230, 230, 120)),
];

/// Ring appearances, shuffled onto the ring kinds each run.
const RING_GEMS: [(&str, Rgb); 8] = [
    ("ruby", Rgb(215, 50, 70)),
    ("jade", Rgb(80, 175, 110)),
    ("onyx", Rgb(125, 125, 140)),
    ("opal", Rgb(210, 205, 230)),
    ("garnet", Rgb(165, 45, 60)),
    ("sapphire", Rgb(70, 105, 220)),
    ("iron", Rgb(150, 150, 150)),
    ("bone", Rgb(225, 215, 180)),
];

/// Scroll titles are made of these, e.g. "ZELGOR VEXITH".
const SYLLABLES: [&str; 20] = [
    "zel", "gor", "mak", "thu", "vex", "nar", "ool", "ith", "bra", "kul", "sar", "eth", "dro",
    "fen", "qua", "rim", "zu", "ta", "lo", "vin",
];

const WEAPON_COLOR: Rgb = Rgb(170, 175, 195);
const ARMOR_COLOR: Rgb = Rgb(160, 135, 100);
const SCROLL_COLOR: Rgb = Rgb(225, 215, 185);

pub struct Lore {
    /// Index into `POTION_LOOKS` for each potion kind, by position in
    /// `PotionKind::ALL`.
    potion_look: Vec<usize>,
    ring_gem: Vec<usize>,
    scroll_title: Vec<String>,
    /// Potion, scroll and ring kinds the player has learned.
    known: HashSet<ItemKind>,
}

/// Position of `kind` in `all`: its index into the shuffled tables.
fn index_of<T: PartialEq>(all: &[T], kind: &T) -> usize {
    all.iter()
        .position(|k| k == kind)
        .expect("every kind is listed in ALL")
}

/// A random ordering of `0..n`, using the Fisher-Yates shuffle.
fn shuffled(rng: &mut Rng, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, rng.index(i + 1));
    }
    order
}

impl Lore {
    /// A fresh run's appearances, with nothing learned yet.
    pub fn new(rng: &mut Rng) -> Self {
        let mut scroll_title: Vec<String> = Vec::new();
        while scroll_title.len() < ScrollKind::ALL.len() {
            let mut word = || {
                let a = SYLLABLES[rng.index(SYLLABLES.len())];
                let b = SYLLABLES[rng.index(SYLLABLES.len())];
                format!("{a}{b}").to_uppercase()
            };
            let title = format!("{} {}", word(), word());
            // Two scrolls with the same title would be a giveaway bug.
            if !scroll_title.contains(&title) {
                scroll_title.push(title);
            }
        }
        Self {
            potion_look: shuffled(rng, POTION_LOOKS.len()),
            ring_gem: shuffled(rng, RING_GEMS.len()),
            scroll_title,
            known: HashSet::new(),
        }
    }

    /// Does the player know what this kind of item is?
    pub fn knows(&self, kind: ItemKind) -> bool {
        match kind {
            ItemKind::Weapon(_) | ItemKind::Armor(_) => true,
            _ => self.known.contains(&kind),
        }
    }

    /// Learns a kind. Returns true if it wasn't already known.
    pub fn learn(&mut self, kind: ItemKind) -> bool {
        match kind {
            ItemKind::Weapon(_) | ItemKind::Armor(_) => false,
            _ => self.known.insert(kind),
        }
    }

    /// True if nothing about this item is hidden any more.
    pub fn fully_known(&self, item: &Item) -> bool {
        self.knows(item.kind) && (!item.kind.is_equipment() || item.known)
    }

    fn potion_look(&self, p: PotionKind) -> (&'static str, Rgb) {
        POTION_LOOKS[self.potion_look[index_of(&PotionKind::ALL, &p)]]
    }

    fn ring_gem(&self, r: RingKind) -> (&'static str, Rgb) {
        RING_GEMS[self.ring_gem[index_of(&RingKind::ALL, &r)]]
    }

    fn scroll_title(&self, s: ScrollKind) -> &str {
        &self.scroll_title[index_of(&ScrollKind::ALL, &s)]
    }

    /// How items of this kind are drawn. Potions and rings take the
    /// color of their appearance, so the color gives nothing away.
    pub fn color(&self, kind: ItemKind) -> Rgb {
        match kind {
            ItemKind::Weapon(_) => WEAPON_COLOR,
            ItemKind::Armor(_) => ARMOR_COLOR,
            ItemKind::Potion(p) => self.potion_look(p).1,
            ItemKind::Scroll(_) => SCROLL_COLOR,
            ItemKind::Ring(r) => self.ring_gem(r).1,
        }
    }

    /// The item's name as the player knows it, without an article:
    /// "+1 sword", "sword", "3 murky potions", "ring of accuracy".
    pub fn name(&self, item: &Item) -> String {
        let enchant = if item.kind.is_equipment() && item.known {
            format!("{:+} ", item.enchant)
        } else {
            String::new()
        };
        let plural = item.count > 1;
        let s = if plural { "s" } else { "" };
        let name = match item.kind {
            ItemKind::Weapon(w) => format!("{enchant}{}", w.stats().name),
            ItemKind::Armor(a) => format!("{enchant}{}", a.stats().name),
            ItemKind::Ring(r) if self.knows(item.kind) => {
                format!("{enchant}ring of {}", r.stats().name)
            }
            ItemKind::Ring(r) => format!("{enchant}{} ring", self.ring_gem(r).0),
            ItemKind::Potion(p) if self.knows(item.kind) => {
                format!("potion{s} of {}", p.stats().name)
            }
            ItemKind::Potion(p) => format!("{} potion{s}", self.potion_look(p).0),
            ItemKind::Scroll(k) if self.knows(item.kind) => {
                format!("scroll{s} of {}", k.stats().name)
            }
            ItemKind::Scroll(k) => format!("scroll{s} titled \"{}\"", self.scroll_title(k)),
        };
        if plural {
            format!("{} {name}", item.count)
        } else {
            name
        }
    }

    /// The name with "a"/"an" in front when there is just one. Armor
    /// names like "chain mail" read better without one.
    pub fn with_article(&self, item: &Item) -> String {
        let name = self.name(item);
        if item.count > 1 || matches!(item.kind, ItemKind::Armor(_)) {
            name
        } else {
            format!("{} {name}", article(&name))
        }
    }

    /// The true name of a kind, for "That was a potion of decay."
    pub fn true_name(kind: ItemKind) -> String {
        match kind {
            ItemKind::Weapon(w) => w.stats().name.to_string(),
            ItemKind::Armor(a) => a.stats().name.to_string(),
            ItemKind::Potion(p) => format!("potion of {}", p.stats().name),
            ItemKind::Scroll(s) => format!("scroll of {}", s.stats().name),
            ItemKind::Ring(r) => format!("ring of {}", r.stats().name),
        }
    }

    /// Lines describing the item for the inventory screen, revealing
    /// only what the player knows.
    pub fn describe(&self, item: &Item) -> Vec<String> {
        let e = if item.known { item.enchant } else { 0 };
        let unknown = if item.known {
            ""
        } else {
            " Its enchantment is unknown."
        };
        let mut lines = match item.kind {
            ItemKind::Weapon(w) => {
                let s = w.stats();
                vec![
                    s.about.to_string(),
                    format!(
                        "Damage {}-{}, accuracy {:+}.{unknown}",
                        (s.damage.0 + e).max(1),
                        (s.damage.1 + e).max(1),
                        s.accuracy + e
                    ),
                ]
            }
            ItemKind::Armor(a) => {
                let s = a.stats();
                vec![
                    s.about.to_string(),
                    format!(
                        "Armor {}, dodge {:+}.{unknown}",
                        (s.armor + e).max(0),
                        s.dodge
                    ),
                ]
            }
            ItemKind::Ring(r) if self.knows(item.kind) => {
                let strength = if item.known {
                    format!("Its strength is {:+}.", item.enchant)
                } else {
                    "Its strength is unknown.".to_string()
                };
                vec![r.stats().about.to_string(), strength]
            }
            ItemKind::Ring(r) => vec![format!(
                "A band set with {}. Wear it to learn its power.",
                self.ring_gem(r).0
            )],
            ItemKind::Potion(p) if self.knows(item.kind) => vec![p.stats().about.to_string()],
            ItemKind::Potion(_) => {
                vec!["You don't know what this does. Drinking it will tell you.".into()]
            }
            ItemKind::Scroll(k) if self.knows(item.kind) => vec![k.stats().about.to_string()],
            ItemKind::Scroll(_) => vec!["Reading it will reveal what it does.".into()],
        };
        if item.is_stuck() && item.known {
            lines.push(format!(
                "It is cursed, and stuck to you for {} more turns.",
                item.curse_turns
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ArmorKind, WeaponKind};

    fn lore() -> Lore {
        Lore::new(&mut Rng::new(1))
    }

    #[test]
    fn every_kind_gets_a_different_appearance() {
        let lore = lore();
        let potions: HashSet<_> = PotionKind::ALL
            .iter()
            .map(|&p| lore.potion_look(p).0)
            .collect();
        assert_eq!(potions.len(), PotionKind::ALL.len());
        let rings: HashSet<_> = RingKind::ALL.iter().map(|&r| lore.ring_gem(r).0).collect();
        assert_eq!(rings.len(), RingKind::ALL.len());
        let titles: HashSet<_> = ScrollKind::ALL
            .iter()
            .map(|&s| lore.scroll_title(s))
            .collect();
        assert_eq!(titles.len(), ScrollKind::ALL.len());
    }

    #[test]
    fn appearances_are_fixed_per_seed_and_vary_between_seeds() {
        let healing = |seed| {
            Lore::new(&mut Rng::new(seed))
                .potion_look(PotionKind::Healing)
                .0
        };
        assert_eq!(healing(5), healing(5));
        let looks: HashSet<_> = (0..20).map(healing).collect();
        assert!(looks.len() > 3, "healing always looks like {looks:?}");
    }

    #[test]
    fn unknown_things_hide_their_names() {
        let mut lore = lore();
        let mut potion = Item::new(ItemKind::Potion(PotionKind::Decay));
        let look = lore.potion_look(PotionKind::Decay).0;
        assert_eq!(
            lore.with_article(&potion),
            format!("{} {look} potion", article(look))
        );
        potion.count = 2;
        assert_eq!(lore.name(&potion), format!("2 {look} potions"));
        assert!(lore.learn(potion.kind));
        assert!(!lore.learn(potion.kind), "learning twice is not news");
        assert_eq!(lore.name(&potion), "2 potions of decay");

        let scroll = Item::new(ItemKind::Scroll(ScrollKind::Identify));
        assert!(lore.name(&scroll).starts_with("scroll titled \""));
    }

    #[test]
    fn enchantments_show_only_once_known() {
        let lore = lore();
        let mut sword = Item::enchanted(ItemKind::Weapon(WeaponKind::Sword), 1);
        assert_eq!(lore.with_article(&sword), "a sword");
        sword.known = true;
        assert_eq!(lore.with_article(&sword), "a +1 sword");
        let mut plain = Item::new(ItemKind::Armor(ArmorKind::Leather));
        plain.known = true;
        assert_eq!(lore.with_article(&plain), "+0 leather armor");
    }

    #[test]
    fn rings_reveal_kind_and_strength_separately() {
        let mut lore = lore();
        let mut ring = Item::enchanted(ItemKind::Ring(RingKind::Accuracy), 2);
        let gem = lore.ring_gem(RingKind::Accuracy).0;
        assert_eq!(lore.name(&ring), format!("{gem} ring"));
        lore.learn(ring.kind);
        assert_eq!(lore.name(&ring), "ring of accuracy");
        ring.known = true;
        assert_eq!(lore.name(&ring), "+2 ring of accuracy");
    }

    #[test]
    fn descriptions_hide_unknown_numbers() {
        let lore = lore();
        let sword = Item::enchanted(ItemKind::Weapon(WeaponKind::Sword), 2);
        let text = lore.describe(&sword).join(" ");
        assert!(
            text.contains("Damage 1-5") && text.contains("unknown"),
            "{text}"
        );
        let potion = Item::new(ItemKind::Potion(PotionKind::Healing));
        assert!(lore.describe(&potion)[0].contains("Drinking it will tell you"));
    }
}
