//! Matching scrobbled strings to catalog entries.

use std::borrow::Cow;

use unicode_normalization::UnicodeNormalization;

/// Key under which a name is looked up in the `*_alias` tables: NFKC, lowercase,
/// trimmed, inner whitespace collapsed. Accents stay significant ("Jóga" ≠ "Joga").
pub fn name_key(name: &str) -> String {
    let normalized: String = name.nfkc().collect::<String>().to_lowercase();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Undoes UTF-8 that was read as Windows-1252 and stored as UTF-8 again
/// ("BÃ¶hse Onkelz" → "Böhse Onkelz", "Die Ã„rzte" → "Die Ärzte").
///
/// Only applied when the round trip yields valid UTF-8, so correct names such as
/// "Björk" (whose bytes are not valid UTF-8 once mapped back) stay untouched.
/// Repeats to undo multiple layers.
pub fn repair_mojibake(s: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(s);
    for _ in 0..3 {
        match undo_one_layer(&out) {
            Some(fixed) if fixed != *out => out = Cow::Owned(fixed),
            _ => break,
        }
    }
    out
}

fn undo_one_layer(s: &str) -> Option<String> {
    if s.is_ascii() {
        return None;
    }
    let bytes = s.chars().map(cp1252_byte).collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

/// The byte a character came from when bytes were decoded as Windows-1252
/// (MySQL's "latin1", which also passes 0x81, 0x8D, 0x8F, 0x90, 0x9D through).
fn cp1252_byte(c: char) -> Option<u8> {
    let b = match c {
        '\u{0}'..='\u{FF}' => c as u8,
        '€' => 0x80,
        '‚' => 0x82,
        'ƒ' => 0x83,
        '„' => 0x84,
        '…' => 0x85,
        '†' => 0x86,
        '‡' => 0x87,
        'ˆ' => 0x88,
        '‰' => 0x89,
        'Š' => 0x8A,
        '‹' => 0x8B,
        'Œ' => 0x8C,
        'Ž' => 0x8E,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '˜' => 0x98,
        '™' => 0x99,
        'š' => 0x9A,
        '›' => 0x9B,
        'œ' => 0x9C,
        'ž' => 0x9E,
        'Ÿ' => 0x9F,
        _ => return None,
    };
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_double_encoded_names() {
        assert_eq!(repair_mojibake("BÃ¶hse Onkelz"), "Böhse Onkelz");
        assert_eq!(repair_mojibake("Kein KÃ¼nstler"), "Kein Künstler");
        assert_eq!(repair_mojibake("TiÃ«sto"), "Tiësto");
        // Ä is C3 84, and 0x84 is „ in Windows-1252.
        assert_eq!(repair_mojibake("Die Ã„rzte"), "Die Ärzte");
        // ß is C3 9F, and 0x9F is Ÿ in Windows-1252.
        assert_eq!(repair_mojibake("WeiÃŸ"), "Weiß");
        assert_eq!(repair_mojibake("BÃƒÂ¶hse"), "Böhse");
    }

    #[test]
    fn leaves_correct_names_alone() {
        for name in [
            "Böhse Onkelz",
            "Björk",
            "Die Ärzte",
            "Sigur Rós",
            "Motörhead",
            "AC/DC",
            "Café del Mar",
            "東京事変",
            "",
        ] {
            assert!(matches!(repair_mojibake(name), Cow::Borrowed(_)), "{name}");
        }
    }

    #[test]
    fn name_key_folds_case_and_whitespace_but_not_accents() {
        assert_eq!(name_key("  Die   ÄRZTE "), "die ärzte");
        assert_eq!(name_key("björk"), name_key("Björk"));
        assert_ne!(name_key("Jóga"), name_key("Joga"));
        // NFKC: composed and decomposed forms match.
        assert_eq!(name_key("Bjo\u{308}rk"), name_key("Björk"));
    }
}
