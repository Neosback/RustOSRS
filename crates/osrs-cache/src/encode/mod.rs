//! Cache-format encoders: the strict inverses of the revision-aware decoders.
//!
//! Encoders here are deterministic and canonical. For a value that came from a decoder they
//! reproduce the original bytes whenever the source used the canonical opcode order (verified
//! against the pinned build-241 cache by `osrs-reference`'s `map_encode_roundtrip` example). They
//! never touch the cache transport: writing archives back is a separate, later concern.

mod map;

pub use map::{MapEncodeError, encode_locations, encode_terrain};
