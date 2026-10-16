//! Splitting names and gameval symbols into comparable words.

/// A structural word found in a name. Deliberately small: only words that name a *role* a piece
/// plays in a building, not what it is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hint {
    Door,
    Gate,
    Window,
    Wall,
    Fence,
    Stairs,
    Ladder,
    Arch,
    Pillar,
    Roof,
    Railing,
    Trapdoor,
}

/// Lowercase words of a name or symbol. A leading `loc.` table prefix is dropped, and letters
/// and digits split apart (`wall2b` is `wall`, `2`, `b`).
pub fn split_tokens(text: &str) -> Vec<String> {
    let text = text.trim().to_ascii_lowercase();
    let text = text.strip_prefix("loc.").unwrap_or(&text);
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut digits = false;
    for c in text.chars() {
        if !c.is_ascii_alphanumeric() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            continue;
        }
        if !word.is_empty() && c.is_ascii_digit() != digits {
            tokens.push(std::mem::take(&mut word));
        }
        digits = c.is_ascii_digit();
        word.push(c);
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

/// True when no word carries meaning: `loc.00`, `loc.1to2`, empty. Such names cannot be kept as
/// the label of an object, so a better one may be suggested.
pub fn is_opaque(text: &str) -> bool {
    !split_tokens(text)
        .iter()
        .any(|t| t.len() >= 3 && t.bytes().all(|b| b.is_ascii_alphabetic()) && t != "null")
}

/// Structural words in `tokens`, in order of appearance, without duplicates.
pub fn hints(tokens: &[String]) -> Vec<Hint> {
    let mut found = Vec::new();
    for token in tokens {
        let word = token
            .strip_suffix('s')
            .filter(|w| w.len() >= 3)
            .unwrap_or(token);
        let hint = match word {
            "door" | "doorway" => Hint::Door,
            "gate" => Hint::Gate,
            "window" => Hint::Window,
            "wall" => Hint::Wall,
            "fence" => Hint::Fence,
            "stair" | "staircase" => Hint::Stairs,
            "ladder" => Hint::Ladder,
            "arch" | "archway" => Hint::Arch,
            "pillar" | "column" => Hint::Pillar,
            "roof" => Hint::Roof,
            "rail" | "railing" => Hint::Railing,
            "trapdoor" => Hint::Trapdoor,
            _ => continue,
        };
        if !found.contains(&hint) {
            found.push(hint);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_and_names_split_into_words_and_numbers() {
        assert_eq!(split_tokens("loc.castledoubledoorl"), ["castledoubledoorl"]);
        assert_eq!(
            split_tokens("loc.Merlin_crate-wall2b"),
            ["merlin", "crate", "wall", "2", "b"]
        );
        assert_eq!(split_tokens("Large door"), ["large", "door"]);
        assert!(split_tokens("  ").is_empty());
    }

    #[test]
    fn opaque_names_have_no_word_of_three_letters() {
        for text in ["loc.00", "loc.1to2", "", "null", "loc.4to5", "x_7"] {
            assert!(is_opaque(text), "{text}");
        }
        for text in ["loc.levera", "Wall", "loc.farming_patch_shadow"] {
            assert!(!is_opaque(text), "{text}");
        }
    }

    #[test]
    fn hints_find_roles_once_and_handle_plurals() {
        let tokens = split_tokens("loc.stone_doors_gate_door");
        assert_eq!(hints(&tokens), [Hint::Door, Hint::Gate]);
        assert!(hints(&split_tokens("loc.00")).is_empty());
        assert_eq!(hints(&split_tokens("Stairs")), [Hint::Stairs]);
    }
}
