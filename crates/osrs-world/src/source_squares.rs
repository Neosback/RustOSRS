//! Pristine map squares read straight from the cache, for exporting edits against their source.
//!
//! Unlike `WorldDefinitions::region`, nothing here applies the editor's worker-side edit layer:
//! an export diffs the authoritative document against exactly what the cache holds.

use crate::WorldError;
use osrs_cache::{
    decode::{DecodedLocations, DecodedTerrain, decode_locations, decode_terrain},
    source::CacheSource,
    transport::{CacheErrorKind, CacheRepository},
};
use osrs_core::coords::RegionCoord;

const MAP_INDEX: u8 = 5;
const TERRAIN_FILE: u32 = 0;
const LOCATION_FILE: u32 = 1;

/// One map square as stored: decoded terrain and locations, plus every file of its group (build
/// 241 groups also carry files 2..=4, which an export must preserve).
pub struct SourceSquare {
    pub region: RegionCoord,
    pub group_id: u32,
    pub terrain: DecodedTerrain,
    pub locations: DecodedLocations,
    pub files: Vec<(u32, Vec<u8>)>,
}

/// A cache opened only to read pristine map squares.
pub struct SourceSquares {
    cache: CacheRepository,
    profile_id: String,
}

impl SourceSquares {
    pub fn open(source: impl Into<CacheSource>) -> Result<Self, WorldError> {
        let profiles = osrs_cache::registry::bundled_profiles()?;
        let (cache, profile) = CacheRepository::open_matching(source, &profiles)?;
        Ok(Self {
            cache,
            profile_id: profile.id,
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn fingerprint(&self) -> String {
        self.cache.fingerprint()
    }

    /// The stored square, or `None` when the cache has no map data for `region`.
    pub fn read(&self, region: RegionCoord) -> Result<Option<SourceSquare>, WorldError> {
        let group_id = ((region.x as u32) << 8) | region.y as u32;
        let files = match self.cache.read_group_files(MAP_INDEX, group_id) {
            Ok(files) => files,
            Err(error)
                if matches!(
                    error.kind(),
                    CacheErrorKind::MapResolution { .. }
                        | CacheErrorKind::MissingGroup
                        | CacheErrorKind::MissingMetadata
                        | CacheErrorKind::MissingFile
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let square = self.cache.read_map_square(region)?;
        let terrain = decode_terrain(
            &square.terrain.bytes,
            self.cache.context(),
            &square.terrain.provenance,
            region,
        )?;
        let locations = decode_locations(
            &square.locations.bytes,
            self.cache.context(),
            &square.locations.provenance,
            region,
        )?;
        let files = files
            .into_iter()
            .map(|(id, file)| (id, file.bytes))
            .collect::<Vec<_>>();
        debug_assert!(files.iter().any(|(id, _)| *id == TERRAIN_FILE));
        debug_assert!(files.iter().any(|(id, _)| *id == LOCATION_FILE));
        Ok(Some(SourceSquare {
            region,
            group_id,
            terrain,
            locations,
            files,
        }))
    }
}
