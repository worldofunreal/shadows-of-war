//! Server-side display-name policy for public multiplayer.
//!
//! Poki does not allow unmoderated player chat or unsafe player-facing text.
//! Names are filtered before they are stored or broadcast. The list is kept
//! in code so the policy version is part of every server build.

pub const POLICY_VERSION: u32 = 1;
pub const MAX_CHARS: usize = 16;

const BLOCKED_TERMS: &[&str] = &[
    "asshole",
    "bastard",
    "bitch",
    "cabron",
    "connard",
    "cojones",
    "cunt",
    "encule",
    "faggot",
    "fotze",
    "fuck",
    "hurensohn",
    "maricon",
    "merde",
    "mierda",
    "nazi",
    "pendejo",
    "piss",
    "putain",
    "puta",
    "puto",
    "retard",
    "salope",
    "scheisse",
    "shit",
    "slut",
    "verga",
    "whore",
    "zorra",
];

fn fold_char(ch: char) -> Option<char> {
    let lower = ch.to_lowercase().next().unwrap_or(ch);
    let folded = match lower {
        '0' => 'o',
        '1' => 'i',
        '3' => 'e',
        '4' => 'a',
        '5' => 's',
        '7' => 't',
        '8' => 'b',
        '$' => 's',
        '@' => 'a',
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'ç' => 'c',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        other => other,
    };
    folded.is_ascii_alphanumeric().then_some(folded)
}

/// Unicode has format characters that render invisibly but can make two
/// player names look identical or hide a blocked word. They are rejected
/// instead of being silently preserved in stored or broadcast names.
fn is_disallowed_invisible(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{061C}'
                | '\u{115F}'
                | '\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// Produce the compact form used to detect separators, accents, and common
/// leetspeak without changing the name shown to other players.
pub fn filter_key(value: &str) -> String {
    value.chars().filter_map(fold_char).collect()
}

pub fn is_allowed(value: &str) -> bool {
    if value.chars().any(is_disallowed_invisible) {
        return false;
    }
    let key = filter_key(value);
    !key.is_empty() && !BLOCKED_TERMS.iter().any(|term| key.contains(term))
}

#[cfg(test)]
mod tests {
    use super::{filter_key, is_allowed};

    #[test]
    fn catches_separators_accents_and_leetspeak() {
        assert_eq!(filter_key("F.u.c.k"), "fuck");
        assert_eq!(filter_key("pütà"), "puta");
        assert!(!is_allowed("F.u.c.k"));
        assert!(!is_allowed("pütà"));
        assert!(!is_allowed("sh1t"));
    }

    #[test]
    fn permits_normal_player_names() {
        assert!(is_allowed("Caesar"));
        assert!(is_allowed("Lady Six Sky"));
        assert!(is_allowed("Ragnar-42"));
    }

    #[test]
    fn rejects_invisible_unicode_that_can_spoof_a_name() {
        assert!(!is_allowed("Caes\u{200B}ar"));
        assert!(!is_allowed("F\u{2060}u.c.k"));
        assert!(!is_allowed("\u{FEFF}"));
    }

    #[test]
    fn filters_uppercase_accented_variants() {
        assert!(!is_allowed("PÜTÀ"));
        assert_eq!(filter_key("ÇAÑÓN"), "canon");
    }
}
