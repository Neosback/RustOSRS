//! Retain the accepted FIFO identity on disposable static and inspection artifacts.
//!
//! This reuses the build ledger's stamp. Its revision records dispatch context, not a content hash
//! or an extra freshness gate: unrelated hydration may advance the session while a build is valid.
//! Legacy preview mirrors remain subject to their separate cancellation/restoration contract.
use super::{
    RegionBuildScope,
    builds::{BuildStamp, RegionBuild},
};
use osrs_core::{coords::RegionCoord, provenance::TargetProvenance};
use osrs_world::{RegionGeometry, RegionRenderPath, TerrainPresentation};
use std::sync::Arc;

/// Immutable provenance shared by one accepted build's picker and newly uploaded static zones.
/// Untouched zones retain their original identity through a scoped refresh.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) struct RegionArtifactIdentity {
    region: RegionCoord,
    stamp: BuildStamp,
    target: Arc<TargetProvenance>,
    presentation: TerrainPresentation,
    render_path: RegionRenderPath,
}

impl RegionBuild {
    pub(in crate::app) fn into_scope(self) -> RegionBuildScope {
        self.scope
    }

    /// Called only after the existing FIFO, presentation and semantic-adaptation acceptance gates.
    pub(in crate::app) fn artifact_identity(
        &self,
        candidate: &RegionGeometry,
    ) -> Arc<RegionArtifactIdentity> {
        debug_assert_eq!(self.key, (candidate.region.x, candidate.region.y));
        Arc::new(RegionArtifactIdentity::new(self.stamp, candidate))
    }
}

impl RegionArtifactIdentity {
    pub(super) fn new(stamp: BuildStamp, candidate: &RegionGeometry) -> Self {
        Self {
            region: candidate.region,
            stamp,
            target: Arc::clone(&candidate.target),
            presentation: candidate.presentation,
            render_path: candidate.render_path,
        }
    }

    pub(in crate::app) const fn region(&self) -> RegionCoord {
        self.region
    }

    pub(super) const fn projection_epoch(&self) -> u64 {
        self.stamp.projection_epoch
    }

    pub(super) const fn presentation_generation(&self) -> u32 {
        self.stamp.presentation_generation
    }

    pub(super) const fn source_generation(&self) -> u64 {
        self.stamp.source
    }

    /// Source metadata only: a retained artifact cannot identify a replacement source.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn target_for_source(&self, source: u64) -> Option<&TargetProvenance> {
        (self.stamp.source == source).then_some(self.target.as_ref())
    }

    /// Observational identity of the last accepted artifact, even while newer work is pending.
    pub(in crate::app) fn describe(&self) -> Vec<String> {
        vec![
            format!(
                "region {},{}; build {}/{}; issued revision {}; epoch {}; presentation {}",
                self.region.x,
                self.region.y,
                self.stamp.source,
                self.stamp.request_id,
                self.stamp.revision,
                self.stamp.projection_epoch,
                self.stamp.presentation_generation,
            ),
            format!(
                "profile {}; digest {}; decoder schema {}",
                self.target.profile_id(),
                self.target.profile_digest(),
                self.target.decoder_schema_version()
            ),
            format!("cache {}", self.target.cache_fingerprint()),
            format!("extraction {:?}; {:?}", self.render_path, self.presentation),
        ]
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod reproduction_tests {
    use super::*;
    #[test]
    fn reproduction_source_metadata_cannot_relabel_a_retained_prior_source() {
        let candidate = super::super::test_support::candidate();
        let stamp = BuildStamp {
            request_id: 3,
            revision: 9,
            source: 7,
            projection_epoch: 1,
            presentation_generation: 1,
        };
        let identity = RegionArtifactIdentity::new(stamp, &candidate);
        assert_eq!(
            identity.target_for_source(7),
            Some(candidate.target.as_ref())
        );
        assert!(identity.target_for_source(8).is_none());
        assert_eq!(
            identity.target_for_source(7),
            Some(candidate.target.as_ref())
        );
    }
    #[test]
    fn reproduction_source_query_uses_world_generation_independently_of_project_generation() {
        let mut world = super::super::test_support::world();
        let candidate = super::super::test_support::candidate();
        let tile =
            osrs_core::coords::MapTile::new(candidate.region.x * 64, candidate.region.y * 64);
        super::super::test_support::seed_info(&mut world, &candidate);
        let project = crate::app::project_runtime::ProjectRuntime::default();
        assert_ne!(world.builds.source_generation(), project.generation());
        assert_eq!(world.source_target(tile), Some(candidate.target.as_ref()));
        world.builds.reset_source();
        assert!(world.source_target(tile).is_none());
        super::super::test_support::seed_info(&mut world, &candidate);
        assert_eq!(world.source_target(tile), Some(candidate.target.as_ref()));
    }
}
