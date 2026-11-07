//! Text helpers shared by the parsers.

use std::sync::LazyLock;

use regex::Regex;

/// Decodes HTML entities: the named basics and numeric ones (`&#039;`, `&#x27;`).
/// Anything it doesn't recognise is left as it is.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        // `;` is ASCII, so its byte position is always a char boundary.
        let decoded = rest.bytes().take(12).position(|b| b == b';').and_then(|end| {
            let entity = &rest[1..end];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix('#')
                    .and_then(|n| match n.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => n.parse().ok(),
                    })
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());

/// The text of an HTML fragment: tags removed, entities decoded, trimmed.
pub fn strip_tags(html: &str) -> String {
    decode_entities(&TAG.replace_all(html, "")).trim().to_owned()
}

/// The loose form names and artists are compared in: ASCII, lower case,
/// apostrophes dropped, and every other run of punctuation or space reduced to
/// one space. `Agadeem's Awakening/Agadeem` and `agadeems awakening agadeem`
/// fold to the same thing.
pub fn fold(s: &str) -> String {
    let ascii = deunicode::deunicode(s).to_lowercase();
    let mut out = String::with_capacity(ascii.len());
    let mut space = false;
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else if c != '\'' {
            space = true;
        }
    }
    out
}

/// Whether two artist credits name the same artist, allowing for how the two
/// sites write them: one contains the other once folded (a co-credit `A & B`
/// contains `A`), or they end in the same surname (`Dan Scott` and `Dan
/// Murayama Scott`).
pub fn same_artist(a: &str, b: &str) -> bool {
    let (a, b) = (fold(a), fold(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a.contains(&b) || b.contains(&a) || a.rsplit(' ').next() == b.rsplit(' ').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_artists_loosely() {
        assert!(same_artist("Chase Stone", "chase stone"));
        assert!(same_artist("Dan Murayama Scott", "Dan Scott"));
        assert!(same_artist("Jesper Ejsing & Ron Spencer", "Ron Spencer"));
        assert!(same_artist("Lim Dûl", "Lim Dul"));
        assert!(!same_artist("Lauren YS", "Mark Tedin"));
        assert!(!same_artist("", "Mark Tedin"));
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("Agadeem&#039;s"), "Agadeem's");
        assert_eq!(decode_entities("a &amp; b &lt;c&gt; &#x27;"), "a & b <c> '");
        assert_eq!(decode_entities("AT&T; &unknown; &"), "AT&T; &unknown; &");
        assert_eq!(decode_entities("Lim-D&#251;l"), "Lim-Dûl");
    }

    #[test]
    fn strips_tags() {
        assert_eq!(
            strip_tags(r#" <a href=card?ref=dom001>Karn, Scion of Urza</a> "#),
            "Karn, Scion of Urza"
        );
    }

    #[test]
    fn folds() {
        assert_eq!(
            fold("Agadeem's Awakening/Agadeem, the Undercrypt"),
            "agadeems awakening agadeem the undercrypt"
        );
        assert_eq!(fold("  Lim-Dûl's Vault "), "lim duls vault");
        assert_eq!(fold("Fire // Ice"), "fire ice");
    }
}
