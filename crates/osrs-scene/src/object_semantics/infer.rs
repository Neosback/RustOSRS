//! Roles, families and suggested labels from names, structure and measured relations.
use super::{
    relations::{Census, End, Relation},
    structure::{Slot, Structure},
    tokens::{Hint, hints, is_opaque, split_tokens},
};
use osrs_core::ids::ObjectId;
use std::collections::{BTreeMap, BTreeSet};

/// A relation seen fewer times than this is treated as noise when grouping families.
const MIN_RELATION_COUNT: u32 = 2;
/// Share of descriptive family members that must carry a word for it to name the family.
const FAMILY_WORD_SHARE: f32 = 0.5;

/// The part a piece plays in a building shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Straight,
    Corner,
    /// A straight piece that only ever finishes runs (never continues one).
    EndCap,
    Pillar,
    Door,
    Window,
    /// Fills a gap in a wall but is neither a door nor a window by name or verb.
    Opening,
    Decoration,
    Roof,
}

impl Role {
    pub fn word(self) -> &'static str {
        match self {
            Role::Straight => "straight",
            Role::Corner => "corner",
            Role::EndCap => "end cap",
            Role::Pillar => "pillar",
            Role::Door => "door",
            Role::Window => "window",
            Role::Opening => "opening",
            Role::Decoration => "decoration",
            Role::Roof => "roof",
        }
    }
}

/// A set of objects that join each other into one wall style. The ID is the smallest member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FamilyId(pub ObjectId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSource {
    /// Taken from the cache name or gameval symbol.
    Given,
    /// Built from the family and role; needs a person's confirmation.
    Inferred,
}

/// Everything known about one object before relations are considered.
#[derive(Debug, Clone)]
pub struct ObjectFacts {
    pub id: ObjectId,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub structure: Structure,
}

impl ObjectFacts {
    /// The best given label: a descriptive cache name, else a descriptive symbol.
    fn given(&self) -> Option<&str> {
        [self.name.as_deref(), self.symbol.as_deref()]
            .into_iter()
            .flatten()
            .find(|text| !is_opaque(text))
    }

    fn hints(&self) -> Vec<Hint> {
        let mut all = Vec::new();
        for text in [self.name.as_deref(), self.symbol.as_deref()]
            .into_iter()
            .flatten()
        {
            for hint in hints(&split_tokens(text)) {
                if !all.contains(&hint) {
                    all.push(hint);
                }
            }
        }
        all
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub id: ObjectId,
    pub role: Option<Role>,
    pub family: Option<FamilyId>,
    pub label: Option<String>,
    pub label_source: LabelSource,
    /// 0 to 1. Given labels are 1; inferred ones grow with how often the evidence was seen.
    pub confidence: f32,
    pub evidence: Vec<String>,
}

struct Disjoint(BTreeMap<ObjectId, ObjectId>);

impl Disjoint {
    fn find(&mut self, id: ObjectId) -> ObjectId {
        let parent = *self.0.entry(id).or_insert(id);
        if parent == id {
            return id;
        }
        let root = self.find(parent);
        self.0.insert(id, root);
        root
    }

    fn union(&mut self, a: ObjectId, b: ObjectId) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0.insert(a.max(b), a.min(b));
        }
    }
}

#[derive(Default)]
struct Tally {
    continues: u32,
    corner: u32,
    arm_of: u32,
    run_ends: u32,
    inset: u32,
    decorates: u32,
}

/// Suggestions for every object in `facts`, sorted by ID.
pub fn infer(census: &Census, facts: &BTreeMap<ObjectId, ObjectFacts>) -> Vec<Suggestion> {
    let mut tally: BTreeMap<ObjectId, Tally> = BTreeMap::new();
    let mut families = Disjoint(BTreeMap::new());
    let mut evidence: BTreeMap<ObjectId, Vec<String>> = BTreeMap::new();
    let mut note = |id: ObjectId, text: String| evidence.entry(id).or_default().push(text);
    for (relation, &n) in census.relations() {
        let strong = n >= MIN_RELATION_COUNT;
        match *relation {
            Relation::Continues(a, b) => {
                tally.entry(a).or_default().continues += n;
                tally.entry(b).or_default().continues += n;
                if strong {
                    families.union(a, b);
                }
                note(a, format!("continues {} x{n}", b.get()));
                if a != b {
                    note(b, format!("continues {} x{n}", a.get()));
                }
            }
            Relation::CornerArm { corner, arm } => {
                tally.entry(corner).or_default().corner += n;
                tally.entry(arm).or_default().arm_of += n;
                if strong {
                    families.union(corner, arm);
                }
                note(corner, format!("corner between runs of {} x{n}", arm.get()));
                note(arm, format!("run ends at corner {} x{n}", corner.get()));
            }
            Relation::RunEnd { wall, end } => {
                tally.entry(wall).or_default().run_ends += n;
                let what = match end {
                    End::Open => "open end".to_owned(),
                    End::Perpendicular(id) => format!("meets wall {}", id.get()),
                    End::Pillar(id) => format!("capped by pillar {}", id.get()),
                };
                note(wall, format!("{what} x{n}"));
            }
            Relation::Inset { host, inset } => {
                tally.entry(inset).or_default().inset += n;
                if strong {
                    families.union(host, inset);
                }
                note(inset, format!("sits inside wall {} x{n}", host.get()));
            }
            Relation::Decorates { decoration, host } => {
                tally.entry(decoration).or_default().decorates += n;
                note(decoration, format!("decorates wall {} x{n}", host.get()));
            }
        }
    }
    let member_family: BTreeMap<ObjectId, FamilyId> = tally
        .keys()
        .copied()
        .filter(|id| {
            let t = &tally[id];
            t.decorates == 0 || t.continues + t.corner + t.arm_of + t.inset > 0
        })
        .map(|id| (id, FamilyId(families.find(id))))
        .collect();
    let family_words = family_words(&member_family, facts);
    let no_tally = Tally::default();
    let mut out = Vec::new();
    for (id, f) in facts {
        let t = tally.get(id);
        let role = role_of(f, t.unwrap_or(&no_tally), census.occurrences(*id));
        let family = member_family.get(id).copied();
        let words = family.and_then(|fam| family_words.get(&fam));
        let mut lines = evidence.remove(id).unwrap_or_default();
        lines.truncate(8);
        let (label, label_source, confidence) = if let Some(given) = f.given() {
            (Some(given.to_owned()), LabelSource::Given, 1.0)
        } else if let (Some(words), Some(role)) = (words, role) {
            let seen = t.map_or(0, |t| {
                t.continues + t.corner + t.arm_of + t.inset + t.decorates
            });
            let confidence = (0.4 + 0.1 * (seen.min(5) as f32)).min(0.9);
            (
                Some(format!("{} {}", words.join(" "), role.word())),
                LabelSource::Inferred,
                confidence,
            )
        } else {
            (None, LabelSource::Inferred, 0.0)
        };
        out.push(Suggestion {
            id: *id,
            role,
            family,
            label,
            label_source,
            confidence,
            evidence: lines,
        });
    }
    out
}

fn role_of(f: &ObjectFacts, t: &Tally, occurrences: u32) -> Option<Role> {
    let slots = &f.structure.slots;
    if slots.contains(&Slot::Roof) {
        return Some(Role::Roof);
    }
    if t.inset > 0 && t.inset * 2 >= occurrences {
        let h = f.hints();
        return Some(
            if f.structure.door_like() || h.contains(&Hint::Door) || h.contains(&Hint::Gate) {
                Role::Door
            } else if h.contains(&Hint::Window) {
                Role::Window
            } else {
                Role::Opening
            },
        );
    }
    if t.corner > 0 || slots.contains(&Slot::WallCorner) {
        return Some(Role::Corner);
    }
    if slots.contains(&Slot::Pillar) {
        return Some(Role::Pillar);
    }
    if t.decorates > 0 || slots.contains(&Slot::WallDecoration) {
        return Some(Role::Decoration);
    }
    if slots.contains(&Slot::Wall) || t.continues > 0 || t.arm_of > 0 {
        let only_ends =
            occurrences > 0 && t.continues == 0 && t.arm_of == 0 && t.run_ends >= occurrences;
        return Some(if only_ends {
            Role::EndCap
        } else {
            Role::Straight
        });
    }
    None
}

/// Words shared by at least half the descriptive members of each family, with structural role
/// words removed so the label can name the style rather than one piece's role.
fn family_words(
    members: &BTreeMap<ObjectId, FamilyId>,
    facts: &BTreeMap<ObjectId, ObjectFacts>,
) -> BTreeMap<FamilyId, Vec<String>> {
    let mut by_family: BTreeMap<FamilyId, Vec<BTreeSet<String>>> = BTreeMap::new();
    for (id, family) in members {
        let Some(given) = facts.get(id).and_then(ObjectFacts::given) else {
            continue;
        };
        let words: BTreeSet<String> = split_tokens(given)
            .into_iter()
            .filter(|w| w.len() >= 3 && w.bytes().all(|b| b.is_ascii_alphabetic()))
            .filter(|w| {
                hints(std::slice::from_ref(w)).is_empty() && !ROLE_WORDS.contains(&w.as_str())
            })
            .collect();
        by_family.entry(*family).or_default().push(words);
    }
    let mut out = BTreeMap::new();
    for (family, sets) in by_family {
        let mut counts: BTreeMap<&String, usize> = BTreeMap::new();
        for set in &sets {
            for word in set {
                *counts.entry(word).or_default() += 1;
            }
        }
        let need = (sets.len() as f32 * FAMILY_WORD_SHARE).ceil() as usize;
        let mut words: Vec<String> = counts
            .into_iter()
            .filter(|(_, n)| *n >= need.max(1))
            .map(|(w, _)| w.clone())
            .collect();
        words.sort();
        if !words.is_empty() {
            out.insert(family, words);
        }
    }
    out
}

/// Role-describing words that name a position in a shell rather than a style.
const ROLE_WORDS: [&str; 6] = ["corner", "end", "cap", "straight", "left", "right"];

#[cfg(test)]
mod tests;
