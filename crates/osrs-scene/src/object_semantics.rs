//! Evidence for what an object *is*, for objects whose cache names are missing or opaque.
//!
//! Four independent signals, none of which is authoritative on its own:
//! 1. given names (cache name, OpenRune gameval symbol) split into tokens (`tokens`);
//! 2. structure: which loc-type slots the definition fills, its size and menu verbs (`structure`);
//! 3. relations measured on real placements: what continues, caps, corners or sits inside what
//!    (`relations`);
//! 4. propagation of family and role from related objects (`infer`).
//!
//! Everything here is pure analysis over canonical values. It reads no cache and writes nothing;
//! a suggestion carries its evidence and confidence so a person can accept or reject it.

mod infer;
mod relations;
mod structure;
mod tokens;

pub use infer::{FamilyId, LabelSource, ObjectFacts, Role, Suggestion, infer};
pub use relations::{Census, End, Placed, Relation};
pub use structure::{Slot, Structure, slot_of, structure_of};
pub use tokens::{Hint, hints, is_opaque, split_tokens};
