//! Player settings kept between runs, in a small text file next to the
//! high scores (`settings.txt`). One `name value` pair per line; lines
//! that can't be read are ignored, so a missing or damaged file just
//! means the defaults.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Walking onto an item picks it up.
    pub auto_pickup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_pickup: true }
    }
}

pub fn default_path() -> Option<PathBuf> {
    let scores = crate::scores::default_path()?;
    Some(scores.parent()?.join("settings.txt"))
}

/// The saved settings, or the defaults if there are none.
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .map(|text| parse(&text))
        .unwrap_or_default()
}

fn parse(text: &str) -> Settings {
    let mut settings = Settings::default();
    for line in text.lines() {
        match line.split_whitespace().collect::<Vec<_>>()[..] {
            ["auto_pickup", "on"] => settings.auto_pickup = true,
            ["auto_pickup", "off"] => settings.auto_pickup = false,
            _ => {}
        }
    }
    settings
}

fn text(settings: Settings) -> String {
    let on_off = |b| if b { "on" } else { "off" };
    format!("auto_pickup {}\n", on_off(settings.auto_pickup))
}

pub fn save(path: &Path, settings: Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text(settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        for auto_pickup in [true, false] {
            let settings = Settings { auto_pickup };
            assert_eq!(parse(&text(settings)), settings);
        }
    }

    #[test]
    fn unreadable_lines_leave_the_defaults() {
        assert_eq!(parse("auto_pickup maybe\nnonsense\n"), Settings::default());
        assert_eq!(parse(""), Settings::default());
    }
}
