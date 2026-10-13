//! Curated display names for matchmaking ghosts.
//!
//! Keep the legacy handles verbatim and add only reviewed entries. Do not
//! manufacture bulk CamelCase pairs from generic words or add generic
//! action/fantasy suffixes.
//!
//! Keep every entry unique regardless of letter case; each active lobby draws
//! names from this pool without duplicates.

use std::sync::LazyLock;

pub const BOT_NAME_COUNT: usize = 7156;

pub static BOT_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    include_str!("names.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
});

#[cfg(test)]
mod tests {
    use super::{BOT_NAME_COUNT, BOT_NAMES};
    use std::collections::HashSet;

    #[test]
    fn bot_name_pool_is_unique_safe_and_complete() {
        assert_eq!(BOT_NAMES.len(), BOT_NAME_COUNT);

        let mut seen = HashSet::with_capacity(BOT_NAMES.len());
        for name in BOT_NAMES.iter() {
            assert!(seen.insert(name.to_lowercase()), "duplicate name: {name}");
            assert!(
                name.chars().count() <= sow_data::name_policy::MAX_CHARS,
                "name exceeds server limit: {name}"
            );
            assert!(
                sow_data::name_policy::is_allowed(name),
                "name rejected by server policy: {name}"
            );
        }
    }

    #[test]
    fn bot_hanzi_is_covered_by_the_gameplay_glyph_corpus() {
        let glyph_corpus = include_str!("../../../sow-render/src/text/unicode_name_glyphs.txt");
        for name in BOT_NAMES.iter() {
            for character in name.chars().filter(|character| is_han(*character)) {
                assert!(
                    glyph_corpus.contains(character),
                    "Han character {character:?} in ghost name {name:?} is missing from the gameplay glyph corpus"
                );
            }
        }
    }

    #[test]
    fn bot_hangul_is_covered_by_the_gameplay_glyph_corpus() {
        let glyph_corpus = include_str!("../../../sow-render/src/text/unicode_name_glyphs.txt");
        for name in BOT_NAMES.iter() {
            for character in name.chars().filter(|character| is_hangul(*character)) {
                assert!(
                    glyph_corpus.contains(character),
                    "Hangul character {character:?} in ghost name {name:?} is missing from the gameplay glyph corpus"
                );
            }
        }
    }

    fn is_hangul(character: char) -> bool {
        matches!(
            character as u32,
            0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF
        )
    }

    fn is_han(character: char) -> bool {
        matches!(
            character as u32,
            0x2E80..=0x2EFF
                | 0x2F00..=0x2FDF
                | 0x3005..=0x3007
                | 0x3021..=0x3029
                | 0x3038..=0x303B
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
                | 0x2CEB0..=0x2EBEF
                | 0x2EBF0..=0x2EE5F
                | 0x2F800..=0x2FA1F
                | 0x30000..=0x3134F
        )
    }
}
