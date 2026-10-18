//! Names people give to objects, kept per object *id* so a name given in one building shows up in
//! every other building and prefab that uses the same object.
//!
//! The cache leaves most objects unnamed, and the game's internal symbols are often opaque
//! (`loc.00`). A name book holds what a person (or an accepted suggestion) says an object is. It
//! never changes the map: it is read wherever a name is shown or used to tell a door from a wall.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NAME_BOOK_FORMAT: &str = "rustosrs-name-book";
pub const NAME_BOOK_VERSION: u32 = 1;

/// Roles an object can have, offered in menus (a custom word is also allowed).
pub const ROLES: &[&str] = &[
    "wall",
    "door",
    "window",
    "stairs",
    "ladder",
    "roof",
    "bed",
    "table",
    "chair",
    "stool",
    "bench",
    "counter",
    "shelf",
    "bookcase",
    "chest",
    "barrel",
    "crate",
    "range",
    "sink",
    "fireplace",
    "lamp",
    "torch",
    "painting",
    "banner",
    "rug",
    "plant",
    "statue",
    "altar",
    "anvil",
    "furnace",
    "other",
];

/// Where a name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    /// Typed by a person.
    You,
    /// The cache's own name for the object.
    Cache,
    /// The game's internal symbol (`castle door l`).
    Symbol,
    /// A suggestion a person accepted.
    Accepted,
    /// Nothing names it.
    Unnamed,
}

impl NameSource {
    /// The words a person reads, never a technical term.
    pub const fn label(self) -> &'static str {
        match self {
            Self::You => "named by you",
            Self::Cache => "the game's name",
            Self::Symbol => "the game's internal name",
            Self::Accepted => "a guess you accepted",
            Self::Unnamed => "no name yet",
        }
    }
}

/// What the cache called the object when it was named, so a different cache (the object ids can
/// change between game revisions) is noticed instead of silently trusted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Witness {
    pub cache_name: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it is for: `door`, `bed`... (see [`ROLES`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The set of pieces it belongs to, as a short word (`brick town`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Which quarter turn faces into the room, for furniture and decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front: Option<u8>,
    pub source: Option<NameSource>,
    /// How sure a suggestion was when it was accepted, 0 to 100 (people are always 100).
    #[serde(default)]
    pub confidence: u8,
    #[serde(default)]
    pub witness: Witness,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A name for floor ground (an underlay or overlay id).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorName {
    pub name: String,
    /// `plank`, `flagstone`, `carpet`...
    #[serde(default)]
    pub class: String,
}

/// The name a person should see, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub text: String,
    pub source: NameSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectNames {
    pub format: String,
    pub version: u32,
    /// The cache build the names were made against (`0` when unknown).
    #[serde(default)]
    pub build: u32,
    #[serde(default)]
    pub entries: BTreeMap<u32, NameEntry>,
    #[serde(default)]
    pub floors: BTreeMap<u32, FloorName>,
}

impl Default for ObjectNames {
    fn default() -> Self {
        Self::new(0)
    }
}

/// True when no word carries meaning: `loc.00`, `loc.1to2`, empty. Same rule the object
/// semantics use to decide a name cannot be kept.
pub fn is_opaque(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    let text = lower.strip_prefix("loc.").unwrap_or(&lower);
    // A run of at least three letters (digits and punctuation split words) that is not "null".
    !text
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|run| run.len() >= 3 && run != "null")
}

fn usable(text: Option<&str>) -> Option<&str> {
    text.map(str::trim)
        .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("null"))
}

impl ObjectNames {
    pub fn new(build: u32) -> Self {
        Self {
            format: NAME_BOOK_FORMAT.to_owned(),
            version: NAME_BOOK_VERSION,
            build,
            entries: BTreeMap::new(),
            floors: BTreeMap::new(),
        }
    }

    pub fn get(&self, id: u32) -> Option<&NameEntry> {
        self.entries.get(&id)
    }

    /// How many objects have a name from a person.
    pub fn named_count(&self) -> usize {
        self.entries.values().filter(|e| e.name.is_some()).count()
    }

    fn entry_mut(&mut self, id: u32, witness: Witness) -> &mut NameEntry {
        let entry = self.entries.entry(id).or_default();
        entry.witness = witness;
        entry
    }

    /// A person named the object.
    pub fn set_name(&mut self, id: u32, name: &str, witness: Witness) {
        let name = name.trim();
        if name.is_empty() {
            self.clear_name(id);
            return;
        }
        let entry = self.entry_mut(id, witness);
        entry.name = Some(name.to_owned());
        entry.source = Some(NameSource::You);
        entry.confidence = 100;
    }

    pub fn set_role(&mut self, id: u32, role: &str, witness: Witness) {
        let role = role.trim().to_lowercase();
        let entry = self.entry_mut(id, witness);
        entry.role = (!role.is_empty()).then_some(role);
        if entry.source.is_none() {
            entry.source = Some(NameSource::You);
            entry.confidence = 100;
        }
    }

    pub fn set_family(&mut self, id: u32, family: &str, witness: Witness) {
        let family = family.trim().to_lowercase();
        self.entry_mut(id, witness).family = (!family.is_empty()).then_some(family);
    }

    /// A person accepted a suggestion (`confidence` is how sure it was, 0 to 100).
    pub fn accept_guess(
        &mut self,
        id: u32,
        name: &str,
        role: Option<&str>,
        confidence: u8,
        witness: Witness,
    ) {
        let entry = self.entry_mut(id, witness);
        if entry.source != Some(NameSource::You) || entry.name.is_none() {
            entry.name = Some(name.trim().to_owned());
            entry.source = Some(NameSource::Accepted);
            entry.confidence = confidence.min(100);
        }
        if let Some(role) = role {
            entry.role = Some(role.trim().to_lowercase());
        }
    }

    /// Forget the name (the role and family stay).
    pub fn clear_name(&mut self, id: u32) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.name = None;
            entry.source = None;
            entry.confidence = 0;
            if *entry == NameEntry::default() {
                self.entries.remove(&id);
            }
        }
    }

    pub fn remove(&mut self, id: u32) {
        self.entries.remove(&id);
    }

    /// The name to show for an object: your name, then the cache's, then the game's internal
    /// symbol, then a suggestion you accepted.
    pub fn resolve(&self, id: u32, cache_name: Option<&str>, symbol: Option<&str>) -> Resolved {
        let entry = self.entries.get(&id);
        let own = entry.and_then(|e| e.name.as_deref().map(|n| (n, e.source)));
        if let Some((name, Some(NameSource::You))) = own {
            return Resolved {
                text: name.to_owned(),
                source: NameSource::You,
            };
        }
        if let Some(name) = usable(cache_name) {
            return Resolved {
                text: name.to_owned(),
                source: NameSource::Cache,
            };
        }
        if let Some(symbol) = usable(symbol).filter(|s| !is_opaque(s)) {
            return Resolved {
                text: symbol.to_owned(),
                source: NameSource::Symbol,
            };
        }
        if let Some((name, _)) = own {
            return Resolved {
                text: name.to_owned(),
                source: NameSource::Accepted,
            };
        }
        Resolved {
            text: String::new(),
            source: NameSource::Unnamed,
        }
    }

    /// Whether the cache's names for the object are not what they were when it was named (a
    /// different cache revision, where the id may mean something else).
    pub fn witness_differs(&self, id: u32, cache_name: &str, symbol: &str) -> bool {
        self.entries.get(&id).is_some_and(|e| {
            let w = &e.witness;
            (!w.cache_name.is_empty() || !w.symbol.is_empty())
                && (w.cache_name != cache_name || w.symbol != symbol)
        })
    }

    /// Take names from `other`; where both name an object, `keep_mine` decides.
    pub fn merge(&mut self, other: &Self, keep_mine: bool) {
        for (id, theirs) in &other.entries {
            if !keep_mine || !self.entries.contains_key(id) {
                self.entries.insert(*id, theirs.clone());
            }
        }
        for (id, theirs) in &other.floors {
            if !keep_mine || !self.floors.contains_key(id) {
                self.floors.insert(*id, theirs.clone());
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != NAME_BOOK_FORMAT {
            return Err(format!("not a name book (format {:?})", self.format));
        }
        if self.version != NAME_BOOK_VERSION {
            return Err(format!(
                "name book version {} is not supported",
                self.version
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witness() -> Witness {
        Witness {
            cache_name: "null".into(),
            symbol: "loc.00".into(),
        }
    }

    #[test]
    fn your_name_beats_the_cache_which_beats_the_symbol_which_beats_an_accepted_guess() {
        let mut book = ObjectNames::new(240);
        assert_eq!(
            book.resolve(7, None, Some("loc.00")).source,
            NameSource::Unnamed
        );
        assert_eq!(
            book.resolve(7, Some("Bed"), Some("loc.bed_double")).text,
            "Bed"
        );
        assert_eq!(
            book.resolve(7, Some("null"), Some("bed double")).source,
            NameSource::Symbol
        );
        book.accept_guess(7, "bed", Some("bed"), 72, witness());
        assert_eq!(book.resolve(7, None, None).source, NameSource::Accepted);
        // The game's own name still wins over a guess.
        assert_eq!(book.resolve(7, Some("Cot"), None).text, "Cot");
        book.set_name(7, "  Old bed ", witness());
        let resolved = book.resolve(7, Some("Cot"), Some("bed double"));
        assert_eq!(
            (resolved.text.as_str(), resolved.source),
            ("Old bed", NameSource::You)
        );
        // Accepting a guess later never overwrites your own name.
        book.accept_guess(7, "bed", None, 90, witness());
        assert_eq!(book.resolve(7, None, None).text, "Old bed");
    }

    #[test]
    fn opaque_symbols_are_not_names() {
        assert!(is_opaque("loc.00"));
        assert!(is_opaque("loc.1to2"));
        assert!(is_opaque(""));
        assert!(!is_opaque("loc.castle_door_l"));
    }

    #[test]
    fn a_different_cache_is_noticed_and_roles_and_families_are_lowercased() {
        let mut book = ObjectNames::new(240);
        book.set_name(9, "Window", witness());
        book.set_role(9, "Window", witness());
        book.set_family(9, "Brick Town", witness());
        assert_eq!(book.get(9).and_then(|e| e.role.as_deref()), Some("window"));
        assert_eq!(
            book.get(9).and_then(|e| e.family.as_deref()),
            Some("brick town")
        );
        assert!(!book.witness_differs(9, "null", "loc.00"));
        assert!(
            book.witness_differs(9, "Rock", "loc.rocks"),
            "id means something else now"
        );
        let mut wrong = book.clone();
        wrong.format = "x".into();
        assert!(wrong.validate().is_err());
        assert!(book.validate().is_ok());
    }

    #[test]
    fn clearing_a_name_keeps_the_role_and_an_empty_entry_goes_away() {
        let mut book = ObjectNames::new(0);
        book.set_name(3, "Chair", witness());
        book.set_role(3, "chair", witness());
        book.clear_name(3);
        assert_eq!(book.get(3).and_then(|e| e.role.as_deref()), Some("chair"));
        assert_eq!(book.named_count(), 0);
        book.remove(3);
        assert!(book.get(3).is_none());
        book.set_name(4, "Lamp", witness());
        book.clear_name(4);
        assert!(
            book.get(4).is_none() || book.get(4).is_some_and(|e| e.witness != Witness::default())
        );
    }

    #[test]
    fn merging_can_keep_your_names() {
        let (mut mine, mut theirs) = (ObjectNames::new(0), ObjectNames::new(0));
        mine.set_name(1, "Mine", witness());
        theirs.set_name(1, "Theirs", witness());
        theirs.set_name(2, "Other", witness());
        let mut kept = mine.clone();
        kept.merge(&theirs, true);
        assert_eq!(kept.resolve(1, None, None).text, "Mine");
        assert_eq!(kept.resolve(2, None, None).text, "Other");
        mine.merge(&theirs, false);
        assert_eq!(mine.resolve(1, None, None).text, "Theirs");
    }
}
