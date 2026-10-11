//! The target profiles that ship with RustOSRS, one per supported cache build.
//!
//! Adding a build is adding a profile file under `profiles/` (the `make_profile` example writes it from a
//! downloaded cache) and one line here. Opening a cache by fingerprint (`CacheRepository::open_matching`)
//! then needs no configuration: the right profile is the one whose fingerprint the directory matches.

use crate::profile::{TargetProfile, TargetProfileError};

/// Profile files, newest first.
const BUNDLED: &[&str] = &[
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml"),
    include_str!("../../../profiles/osrs-live-240-2026-09-23-openrs2-2720.yaml"),
];

/// Every bundled profile, parsed and validated.
pub fn bundled_profiles() -> Result<Vec<TargetProfile>, TargetProfileError> {
    BUNDLED
        .iter()
        .map(|yaml| TargetProfile::from_yaml_str(yaml))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_profile_is_valid_and_has_a_distinct_fingerprint()
    -> Result<(), TargetProfileError> {
        let profiles = bundled_profiles()?;
        assert!(!profiles.is_empty());
        let mut fingerprints: Vec<_> = profiles
            .iter()
            .map(|p| p.cache_fingerprint.value.clone())
            .collect();
        fingerprints.sort();
        fingerprints.dedup();
        assert_eq!(fingerprints.len(), profiles.len());
        let mut ids: Vec<_> = profiles.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), profiles.len());
        Ok(())
    }
}
