//! Actor image and immutable accepted extraction inputs share a GPU lifetime.
use super::RenderRuntime;
use crate::app::{
    npc_runtime::{NPC_KEY, NpcArtifactIdentity},
    player_runtime::{PLAYER_KEY, PlayerArtifactIdentity},
};
use osrs_render::SceneGeometry;
use std::sync::Arc;

impl RenderRuntime {
    pub(in crate::app) fn upload_npc(
        &mut self,
        identity: Arc<NpcArtifactIdentity>,
        geometry: &SceneGeometry,
    ) {
        self.upload_transient(NPC_KEY, geometry);
        if geometry.vertex_count() != 0 {
            self.npc_identity = Some(identity);
        }
    }

    pub(in crate::app) fn upload_player(
        &mut self,
        identity: Arc<PlayerArtifactIdentity>,
        geometry: &SceneGeometry,
    ) {
        self.upload_transient(PLAYER_KEY, geometry);
        if geometry.vertex_count() != 0 {
            self.player_identity = Some(identity);
        }
    }

    pub(in crate::app) fn npc_identity(&self) -> Option<&NpcArtifactIdentity> {
        self.npc_identity.as_deref()
    }

    pub(in crate::app) fn player_identity(&self) -> Option<&PlayerArtifactIdentity> {
        self.player_identity.as_deref()
    }
}
