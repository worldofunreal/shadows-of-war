//! Curated display names for matchmaking ghosts.
//!
//! Keep the legacy handles verbatim and add only reviewed entries. Do not
//! manufacture bulk CamelCase pairs from generic words or add generic
//! action/fantasy suffixes.
//!
//! Keep every entry unique regardless of letter case; each active lobby draws
//! names from this pool without duplicates.

use std::sync::LazyLock;

pub const BOT_NAME_COUNT: usize = 2_982;

pub static BOT_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    include_str!("names.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
});

#[cfg(test)]
mod tests {
    use super::{BOT_NAME_COUNT, BOT_NAMES};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn bot_name_pool_is_unique_safe_and_complete() {
        assert_eq!(BOT_NAMES.len(), BOT_NAME_COUNT);

        let expected_groups = [
            ("legacy", 821),
            ("latin_iberian", 43),
            ("latin_east_asian", 39),
            ("latin_african_arabic", 41),
            ("latin_nordic_turkic_andean", 38),
            ("japanese_katakana", 500),
            ("korean_hangul", 500),
            ("chinese_simplified", 500),
            ("arabic", 500),
        ];
        let mut group_counts = HashMap::<&str, usize>::new();
        let mut current_group = "";
        for line in include_str!("names.txt").lines() {
            if let Some(group) = line.strip_prefix("# ") {
                current_group = group;
                continue;
            }
            if line.is_empty() {
                continue;
            }
            *group_counts.entry(current_group).or_default() += 1;
            if current_group.starts_with("latin_") {
                assert_eq!(
                    line.chars()
                        .filter(|character| character.is_ascii_uppercase())
                        .count(),
                    1,
                    "Latin additions must be one handle, not a generic CamelCase compound: {line}"
                );
            }
            if current_group == "arabic" {
                let mut words = line.split_whitespace();
                let noun = words.next().unwrap_or_default();
                let adjective = words.next().unwrap_or_default();
                if matches!(noun, "نار" | "شمس" | "ريح" | "مجرة") {
                    assert!(
                        adjective.ends_with('ة') || adjective.ends_with("اء"),
                        "Arabic adjective must agree with feminine noun: {line}"
                    );
                }
            }
        }
        for (group, count) in expected_groups {
            assert_eq!(
                group_counts.get(group).copied(),
                Some(count),
                "group {group}"
            );
        }

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
}
