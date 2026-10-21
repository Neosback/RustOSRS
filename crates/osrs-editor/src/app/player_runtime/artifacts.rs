//! Immutable request inputs survive acceptance without retaining scene or semantic state.
use super::*;
use crate::app::world_runtime::RegionArtifactIdentity;
use osrs_world::npc::ActorGeometryContext;

pub(super) struct PlayerRequestIdentity {
    pub(super) key: SentKey,
    pub(super) frame: PlayerFrame,
    pub(super) ground: Arc<RegionArtifactIdentity>,
}

pub(in crate::app) struct PlayerArtifactIdentity {
    request: Arc<PlayerRequestIdentity>,
    context: ActorGeometryContext,
}

impl PlayerArtifactIdentity {
    pub(super) fn new(request: Arc<PlayerRequestIdentity>, context: ActorGeometryContext) -> Self {
        Self { request, context }
    }

    pub(in crate::app) fn describe(&self) -> Vec<String> {
        let frame = &self.request.frame;
        let key = &self.request.key;
        let mut lines = vec![
            format!(
                "player token {}; cycle {}; frame bucket {}; ground version {}; presentation {}",
                frame.token,
                frame.cycles,
                key.frame,
                key.infos_version,
                key.presentation_generation
            ),
            format!(
                "location {:?}; position {:?}; rotation {}; sequence {:?}; ground {}",
                frame.location, frame.position, frame.rotation, frame.sequence, frame.ground_height
            ),
            format!("look {:?}", frame.look),
            format!(
                "actor extraction {:?}; profile {}; digest {}; decoder schema {}",
                self.context.render_path,
                self.context.target.profile_id(),
                self.context.target.profile_digest(),
                self.context.target.decoder_schema_version()
            ),
            format!("cache {}", self.context.target.cache_fingerprint()),
        ];
        lines.extend(self.request.ground.describe());
        lines
    }
}
