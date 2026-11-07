//! File names from the user's template, such as `NAME (ARTIST) [SET] {NUMBER}`.

/// What a template's placeholders are filled with.
pub struct NameParts<'a> {
    pub name: &'a str,
    pub artist: &'a str,
    pub set: &'a str,
    pub number: &'a str,
}

/// The placeholders a template can use.
pub const PLACEHOLDERS: [&str; 4] = ["NAME", "ARTIST", "SET", "NUMBER"];

/// The template filled in, in one pass — so a card or artist name that happens
/// to contain `SET` or `NUMBER` is left alone — and made safe as a file name
/// (without its extension).
pub fn file_stem(template: &str, parts: &NameParts) -> String {
    let mut filled = String::with_capacity(template.len() + 32);
    let mut rest = template;
    'outer: while !rest.is_empty() {
        for placeholder in PLACEHOLDERS {
            if let Some(after) = rest.strip_prefix(placeholder) {
                filled.push_str(match placeholder {
                    "NAME" => parts.name,
                    "ARTIST" => parts.artist,
                    "SET" => parts.set,
                    _ => parts.number,
                });
                rest = after;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap_or_default();
        filled.push(c);
        rest = &rest[c.len_utf8()..];
    }
    sanitize(&filled)
}

/// Longest file name kept, in characters, leaving room for ` (12).jpg` and
/// for the folders above it on systems with short path limits.
const MAX_STEM: usize = 180;

/// A string made safe as one path component on Windows, macOS and Linux.
pub fn sanitize(name: &str) -> String {
    let name = name.replace(" // ", " - ").replace("//", "-");
    let mut out = String::with_capacity(name.len());
    let mut space = false;
    for c in name.chars() {
        if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
            continue;
        }
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    let mut out: String = out.chars().take(MAX_STEM).collect();
    // Windows drops trailing dots and spaces, so `..` would become nothing.
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    if out.is_empty() {
        return "card".into();
    }
    if is_reserved(&out) {
        out.insert(0, '_');
    }
    out
}

/// Names Windows won't create a file as, with any extension.
fn is_reserved(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0')
}

#[cfg(test)]
mod tests {
    use super::*;

    const KARN: NameParts = NameParts {
        name: "Karn, Scion of Urza",
        artist: "Chase Stone",
        set: "DOM",
        number: "1",
    };

    #[test]
    fn fills_the_default_template() {
        assert_eq!(
            file_stem("NAME (ARTIST) [SET] {NUMBER}", &KARN),
            "Karn, Scion of Urza (Chase Stone) [DOM] {1}"
        );
    }

    #[test]
    fn fills_in_one_pass() {
        let parts = NameParts {
            name: "SETTLE the NUMBERs",
            artist: "NAME",
            ..KARN
        };
        assert_eq!(file_stem("NAME - ARTIST", &parts), "SETTLE the NUMBERs - NAME");
    }

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize("Fire // Ice"), "Fire - Ice");
        assert_eq!(sanitize(r#"Kongming, "Sleeping Dragon""#), "Kongming, Sleeping Dragon");
        assert_eq!(sanitize("Who/What/When/Where/Why"), "WhoWhatWhenWhereWhy");
        assert_eq!(sanitize("  a \t b  "), "a b");
        assert_eq!(sanitize(".."), "card");
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("com1.jpg"), "_com1.jpg");
        assert_eq!(sanitize("Command Tower"), "Command Tower");
        assert_eq!(sanitize(&"x".repeat(300)).len(), MAX_STEM);
    }
}
