//! Capture actual worker presentation with actor extraction, before publishing its owned result.
use super::StreamWorker;
use crate::npc::{NpcFrameInput, NpcGeometryUpdate, PlayerFrame, PlayerGeometryUpdate};

impl StreamWorker {
    pub(super) fn player_geometry(&mut self, frame: &PlayerFrame) -> PlayerGeometryUpdate {
        crate::npc::extract_player_frame(
            &mut self.definitions,
            frame,
            self.hd.as_ref(),
            self.generation,
            self.presentation.hd_geometry,
        )
    }

    pub(super) fn npc_geometry(&mut self, input: NpcFrameInput) -> NpcGeometryUpdate {
        crate::npc::extract_npcs(
            &mut self.definitions,
            input,
            self.hd.as_ref(),
            self.generation,
            self.presentation.hd_geometry,
        )
    }
}
