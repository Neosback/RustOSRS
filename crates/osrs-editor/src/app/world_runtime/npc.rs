//! Offline-NPC requests to the world worker (assets, geometry, definition catalog).

use super::WorldRuntime;
use std::collections::BTreeSet;

impl WorldRuntime {
    pub(in crate::app) fn request_npc_catalog(&self) {
        self.streamer.request_npc_catalog();
    }

    pub(in crate::app) fn request_npc_assets(
        &self,
        identity: osrs_core::actor_snapshot::PlaytestIdentity,
        ids: BTreeSet<osrs_core::ids::NpcId>,
    ) {
        self.streamer.request_npc_assets(identity, ids);
    }

    pub(in crate::app) fn request_npc_geometry(&self, input: osrs_world::npc::NpcFrameInput) {
        self.streamer.request_npc_geometry(input);
    }
}
