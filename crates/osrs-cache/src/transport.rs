//! Experimental M1 transport spike over `rune-fs`.
//!
//! This module is intentionally private until the M1 dependency ADR accepts or
//! rejects the transport strategy. No `rune-fs` type crosses the `osrs-cache`
//! crate boundary.

use runefs::{Dat2, Indices, MAIN_DATA, REFERENCE_TABLE_ID};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const CACHE_FINGERPRINT_DOMAIN: &[u8] = b"rustosrs-cache-v1\0";

#[derive(Debug)]
pub(crate) enum TransportError {
    RuneFs(runefs::Error),
    MissingIndex(u8),
    MissingGroup { index: u8, group: u32 },
    MissingNamedGroup { index: u8, name: String },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuneFs(error) => write!(f, "cache transport failure: {error}"),
            Self::MissingIndex(index) => write!(f, "cache index {index} is not present"),
            Self::MissingGroup { index, group } => {
                write!(f, "cache group {group} is not present in index {index}")
            }
            Self::MissingNamedGroup { index, name } => {
                write!(
                    f,
                    "cache group named {name:?} is not present in index {index}"
                )
            }
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuneFs(error) => Some(error),
            _ => None,
        }
    }
}

impl From<runefs::Error> for TransportError {
    fn from(value: runefs::Error) -> Self {
        Self::RuneFs(value)
    }
}

pub(crate) type Result<T> = std::result::Result<T, TransportError>;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct CacheFingerprint([u8; 32]);

impl CacheFingerprint {
    pub(crate) fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 0x0f) as usize] as char);
        }
        output
    }
}

impl fmt::Debug for CacheFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CacheFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

pub(crate) struct CacheTransport {
    data: Dat2,
    indices: Indices,
}

impl CacheTransport {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        Ok(Self {
            data: Dat2::new(path.join(MAIN_DATA))?,
            indices: Indices::new(path)?,
        })
    }

    pub(crate) fn content_index_ids(&self) -> Vec<u8> {
        let mut ids: Vec<u8> = (&self.indices)
            .into_iter()
            .map(|(id, _)| *id)
            .filter(|id| *id != REFERENCE_TABLE_ID)
            .collect();
        ids.sort_unstable();
        ids
    }

    pub(crate) fn group_ids(&self, index_id: u8) -> Result<Vec<u32>> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let mut ids: Vec<u32> = index.metadata.iter().map(|metadata| metadata.id).collect();
        ids.sort_unstable();
        Ok(ids)
    }

    pub(crate) fn group_count(&self) -> Result<usize> {
        self.content_index_ids()
            .into_iter()
            .try_fold(0usize, |total, index| {
                self.group_ids(index).map(|groups| total + groups.len())
            })
    }

    pub(crate) fn group_id_by_name(&self, index_id: u8, name: &str) -> Result<u32> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let hash = jagex_name_hash(name);
        index
            .metadata
            .iter()
            .find(|metadata| metadata.name_hash == hash)
            .map(|metadata| metadata.id)
            .ok_or_else(|| TransportError::MissingNamedGroup {
                index: index_id,
                name: name.to_owned(),
            })
    }

    pub(crate) fn read_encoded_group(&self, index_id: u8, group_id: u32) -> Result<Vec<u8>> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let archive = index
            .archive_refs
            .get(&group_id)
            .ok_or(TransportError::MissingGroup {
                index: index_id,
                group: group_id,
            })?;
        Ok(self.data.read(archive)?.finalize())
    }

    pub(crate) fn read_decoded_group(
        &self,
        index_id: u8,
        group_id: u32,
        xtea: Option<[u32; 4]>,
    ) -> Result<Vec<u8>> {
        let index = self
            .indices
            .get(&index_id)
            .ok_or(TransportError::MissingIndex(index_id))?;
        let archive = index
            .archive_refs
            .get(&group_id)
            .ok_or(TransportError::MissingGroup {
                index: index_id,
                group: group_id,
            })?;
        let encoded = self.data.read(archive)?;
        let encoded = match xtea {
            Some(keys) => encoded.with_xtea_keys(keys),
            None => encoded,
        };
        Ok(encoded.decode()?.finalize())
    }

    pub(crate) fn fingerprint_v1(&self) -> Result<CacheFingerprint> {
        let mut digest = Sha256::new();
        digest.update(CACHE_FINGERPRINT_DOMAIN);

        let reference_index = self
            .indices
            .get(&REFERENCE_TABLE_ID)
            .ok_or(TransportError::MissingIndex(REFERENCE_TABLE_ID))?;

        for index_id in self.content_index_ids() {
            digest.update(u16::from(index_id).to_be_bytes());

            let reference_archive = reference_index
                .archive_refs
                .get(&u32::from(index_id))
                .ok_or(TransportError::MissingGroup {
                    index: REFERENCE_TABLE_ID,
                    group: u32::from(index_id),
                })?;
            let reference_bytes = self.data.read(reference_archive)?.finalize();
            update_hashed_blob(&mut digest, &reference_bytes);

            for group_id in self.group_ids(index_id)? {
                digest.update(group_id.to_be_bytes());
                let group = self.read_encoded_group(index_id, group_id)?;
                update_hashed_blob(&mut digest, &group);
            }
        }

        Ok(CacheFingerprint(digest.finalize().into()))
    }
}

fn update_hashed_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(Sha256::digest(bytes));
}

fn jagex_name_hash(name: &str) -> i32 {
    name.chars().fold(0i32, |hash, character| {
        hash.wrapping_mul(31).wrapping_add(character as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    const MAP_INDEX: u8 = 5;
    const CONFIG_INDEX: u8 = 2;
    const MODEL_INDEX: u8 = 7;
    const LUMBRIDGE_REGION: &str = "50_50";
    const LEGACY_LUMBRIDGE_XTEA: [u32; 4] =
        [3_030_157_619, 2_364_842_415, 3_297_319_647, 1_973_582_566];
    const LEGACY_CACHE_FINGERPRINT: &str =
        "ad37f18dedd911eba2085d06029f2edf5db3c6285e56f7cb38eddd1cdce04636";
    const BUILD_241_CACHE_FINGERPRINT: &str =
        "ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38";

    fn repository_cache() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rs-cache-master/data/osrs_cache")
    }

    #[test]
    fn name_hash_matches_known_rs_cache_example() {
        assert_eq!(jagex_name_hash("huffman"), 1_258_058_669);
    }

    #[test]
    fn legacy_cache_enumerates_indices_and_groups() -> Result<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let indices = cache.content_index_ids();
        assert!(indices.contains(&0));
        assert!(indices.contains(&CONFIG_INDEX));
        assert!(indices.contains(&MAP_INDEX));
        assert!(!indices.contains(&REFERENCE_TABLE_ID));
        assert!(!cache.group_ids(MAP_INDEX)?.is_empty());
        Ok(())
    }

    #[test]
    fn legacy_cache_supports_name_lookup_and_plain_decompression() -> Result<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let map_group = cache.group_id_by_name(MAP_INDEX, &format!("m{LUMBRIDGE_REGION}"))?;
        let encoded = cache.read_encoded_group(MAP_INDEX, map_group)?;
        let decoded = cache.read_decoded_group(MAP_INDEX, map_group, None)?;
        assert!(!encoded.is_empty());
        assert!(!decoded.is_empty());
        Ok(())
    }

    #[test]
    fn legacy_cache_supports_lumbridge_xtea_boundary() -> Result<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let loc_group = cache.group_id_by_name(MAP_INDEX, &format!("l{LUMBRIDGE_REGION}"))?;
        let decoded =
            cache.read_decoded_group(MAP_INDEX, loc_group, Some(LEGACY_LUMBRIDGE_XTEA))?;
        assert!(!decoded.is_empty());
        Ok(())
    }

    #[test]
    fn fingerprint_v1_is_deterministic_for_legacy_fixture() -> Result<()> {
        let cache = CacheTransport::open(repository_cache())?;
        let first = cache.fingerprint_v1()?;
        let second = cache.fingerprint_v1()?;
        assert_eq!(first, second);
        assert_eq!(first.to_hex(), LEGACY_CACHE_FINGERPRINT);
        Ok(())
    }

    #[test]
    fn configured_build_241_cache_matches_pinned_source_shape() -> Result<()> {
        let Some(path) = env::var_os("RUSTOSRS_TARGET_CACHE_DIR") else {
            return Ok(());
        };
        let cache = CacheTransport::open(path)?;
        let indices = cache.content_index_ids();
        let group_count = cache.group_count()?;
        eprintln!("build-241 physical content indices={indices:?}");
        eprintln!("build-241 physical group count={group_count}");

        assert_eq!(indices.len(), 23);
        assert!(!indices.contains(&16));
        assert!(!indices.contains(&23));
        assert!(indices.contains(&CONFIG_INDEX));
        assert!(indices.contains(&MAP_INDEX));
        assert!(indices.contains(&MODEL_INDEX));
        assert_eq!(group_count, 117_584);

        let map_index = cache
            .indices
            .get(&MAP_INDEX)
            .ok_or(TransportError::MissingIndex(MAP_INDEX))?;
        let named_group_count = map_index
            .metadata
            .iter()
            .filter(|metadata| metadata.name_hash != 0)
            .count();
        eprintln!("build-241 map groups with nonzero name hashes={named_group_count}");
        assert_eq!(named_group_count, 0);

        let map_groups = cache.group_ids(MAP_INDEX)?;
        let Some(map_group) = map_groups.first().copied() else {
            return Err(TransportError::MissingGroup {
                index: MAP_INDEX,
                group: 0,
            });
        };
        let model_groups = cache.group_ids(MODEL_INDEX)?;
        let Some(model_group) = model_groups.first().copied() else {
            return Err(TransportError::MissingGroup {
                index: MODEL_INDEX,
                group: 0,
            });
        };
        assert!(
            !cache
                .read_decoded_group(MAP_INDEX, map_group, None)?
                .is_empty()
        );
        assert!(
            !cache
                .read_decoded_group(MODEL_INDEX, model_group, None)?
                .is_empty()
        );
        assert!(!cache.read_decoded_group(CONFIG_INDEX, 6, None)?.is_empty());

        let fingerprint = cache.fingerprint_v1()?;
        assert_eq!(fingerprint.to_hex(), BUILD_241_CACHE_FINGERPRINT);
        Ok(())
    }
}
