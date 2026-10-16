//! Cache-format encoders: the strict inverses of the revision-aware decoders.
//!
//! Encoders here are deterministic and canonical. For a value that came from a decoder they
//! reproduce the original bytes whenever the source used the canonical opcode order (verified
//! against the pinned build-241 cache by `osrs-reference`'s `map_encode_roundtrip` example). They
//! never touch the cache transport: writing archives back is a separate, later concern.

mod crc;
mod group;
mod map;
mod reference_table;
#[cfg(not(target_arch = "wasm32"))]
mod store;

pub use crc::crc32;
pub use group::{
    ContainerCompression, ContainerError, decode_container, encode_container, pack_group,
    split_group,
};

pub use map::{MapEncodeError, encode_locations, encode_terrain};
pub use reference_table::{GroupEntry, ReferenceTable, ReferenceTableError};

#[cfg(not(target_arch = "wasm32"))]
pub use store::{
    CacheStore, GroupUpdate, REFERENCE_TABLE_INDEX, RepackReport, StoreError, repack_changed_groups, repack_groups,
};
