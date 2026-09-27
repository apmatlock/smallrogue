//! Small helpers for writing English messages.

/// "a" or "an", to go before a word.
pub fn article(word: &str) -> &'static str {
    match word.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articles() {
        assert_eq!(article("rat"), "a");
        assert_eq!(article("orc"), "an");
        assert_eq!(article("+1 sword"), "a");
    }
}
