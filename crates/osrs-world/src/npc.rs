//! NPC asset acquisition and entity-only geometry extraction on the world worker.
//! Simulation never runs here; all poses/positions arrive as immutable snapshots.
use crate::{
    WorldDefinitions,
    hd::{
        HdContext,
        model::{ModelSite, model_faces_resolved},
    },
};
use osrs_core::{
    actor_pose::pose_legacy_actor,
    actor_snapshot::{ActorStatus, PlaytestIdentity, PlaytestSnapshot, PlaytestStamp},
    animation_pose::LegacyAnimationFrame,
    entity::ActorSequenceState,
    ids::{NpcId, VarpId},
    player::PlayerLook,
    scenario::{ActorLocation, NpcSpawnId},
};
use osrs_playtest::runtime::RuntimeAssets;
use osrs_render::{ModelPlacement, SceneGeometry};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub struct NpcAssetsUpdate {
    pub identity: PlaytestIdentity,
    pub assets: Arc<RuntimeAssets>,
    pub errors: Vec<String>,
}
/// A player standing in the world, with its look and where its idle animation is.
pub struct PlayerFrame {
    /// Echoed in the result so the caller can tell which request it answers.
    pub token: u64,
    pub look: PlayerLook,
    pub location: ActorLocation,
    /// Absolute position in local units (128 per tile).
    pub position: (i32, i32),
    pub rotation: u16,
    pub ground_height: i32,
    /// The looping sequence to show and how many client cycles it has been playing.
    pub sequence: Option<osrs_core::ids::SequenceId>,
    pub cycles: u64,
}

pub struct NpcFrameInput {
    pub snapshot: PlaytestSnapshot,
    pub ground_heights: BTreeMap<NpcSpawnId, i32>,
    pub varps: BTreeMap<VarpId, i32>,
}
pub struct NpcGeometryUpdate {
    pub stamp: PlaytestStamp,
    pub geometry: SceneGeometry,
    pub errors: Vec<String>,
}

pub fn load_assets(
    definitions: &mut WorldDefinitions,
    identity: PlaytestIdentity,
    ids: &BTreeSet<NpcId>,
) -> NpcAssetsUpdate {
    let mut assets = RuntimeAssets {
        client_fingerprint: definitions.cache().fingerprint(),
        ..Default::default()
    };
    let mut errors = Vec::new();
    definitions.retain_npc_bases(ids);
    for id in ids {
        match definitions.npc(*id) {
            Ok(Some(npc)) => {
                let m = npc.movement_sequences;
                for seq in [
                    m.idle,
                    m.turn_left,
                    m.turn_right,
                    m.walk,
                    m.walk_back,
                    m.walk_left,
                    m.walk_right,
                ]
                .into_iter()
                .chain(m.run)
                .chain(m.crawl)
                .flatten()
                {
                    if assets.sequences.contains_key(&seq) {
                        continue;
                    }
                    match definitions.sequence(seq) {
                        Ok(Some(sequence)) => {
                            assets.sequences.insert(seq, sequence);
                        }
                        Ok(None) => errors.push(format!("NPC {id}: missing sequence {seq}")),
                        Err(e) => errors.push(format!("NPC {id}: {e}")),
                    }
                }
                assets.npcs.insert(*id, npc);
            }
            Ok(None) => errors.push(format!("missing NPC definition {id}")),
            Err(e) => errors.push(e),
        }
    }
    NpcAssetsUpdate {
        identity,
        assets: Arc::new(assets),
        errors,
    }
}

pub fn extract_npcs(
    definitions: &mut WorldDefinitions,
    input: NpcFrameInput,
    hd: Option<&HdContext>,
) -> NpcGeometryUpdate {
    let mut geometry = SceneGeometry::default();
    let mut errors = Vec::new();
    for actor in &input.snapshot.actors {
        if matches!(
            actor.status,
            ActorStatus::Parked | ActorStatus::MissingDefinition
        ) {
            continue;
        }
        let Some(&height) = input.ground_heights.get(&actor.spawn) else {
            continue;
        };
        let result = (|| -> Result<(), String> {
            let Some(id) = definitions.resolve_npc(actor.npc, &input.varps)? else {
                return Ok(());
            };
            let definition = definitions
                .npc(id)?
                .ok_or_else(|| format!("missing NPC {id}"))?;
            let Some(base) = definitions.npc_base(id)? else {
                return Ok(());
            };
            let selection = actor.sequences.select_action_and_pose(
                definition.movement_sequences.idle,
                true,
                true,
            );
            let action = frame(definitions, selection.action)?;
            let pose = frame(definitions, selection.pose)?;
            let action_sequence = selection
                .action
                .and_then(|s| s.sequence)
                .map(|id| definitions.sequence(id).map_err(|e| e.to_string()))
                .transpose()?
                .flatten();
            let model = pose_legacy_actor(
                &base,
                action.as_deref(),
                pose.as_deref(),
                action_sequence
                    .as_ref()
                    .and_then(|s| s.interleave.as_deref()),
                definition.scale,
            )
            .map_err(|e| e.to_string())?;
            emit(
                &mut geometry,
                hd,
                &model,
                actor.location,
                (actor.position.x.units(), actor.position.z.units()),
                actor.rotation,
                height,
                id.get(),
            );
            Ok(())
        })();
        if let Err(e) = result {
            errors.push(format!("spawn {} NPC {}: {e}", actor.spawn.0, actor.npc));
        }
    }
    for zone in &mut geometry.zones {
        for group in &mut zone.groups {
            group.geometry.finish_hd();
        }
    }
    NpcGeometryUpdate {
        stamp: input.snapshot.stamp,
        geometry,
        errors,
    }
}
fn frame(
    definitions: &mut WorldDefinitions,
    slot: Option<ActorSequenceState>,
) -> Result<Option<Arc<LegacyAnimationFrame>>, String> {
    let Some(slot) = slot else {
        return Ok(None);
    };
    let Some(id) = slot.sequence else {
        return Ok(None);
    };
    let sequence = definitions
        .sequence(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("missing actor sequence {id}"))?;
    if sequence.skeletal_animation.is_some() {
        return Err(format!(
            "skeletal actor sequence {id} requires skeletal posing; no static substitution"
        ));
    }
    let index = slot.frame.unwrap_or(0) as usize;
    let frame = sequence
        .frame_ids
        .get(index)
        .ok_or_else(|| format!("sequence {id} has no frame {index}"))?;
    definitions
        .legacy_frame(*frame)
        .map_err(|e| e.to_string())?
        .map(Some)
        .ok_or_else(|| format!("missing actor frame {frame}"))
}

/// Which frame of a looping sequence is showing `cycles` client cycles after it started: each
/// frame lasts its delay (at least one cycle), then the sequence loops.
fn frame_at(delays: &[u16], cycles: u64) -> usize {
    let lasts = |d: &u16| u64::from((*d).max(1));
    let total: u64 = delays.iter().map(lasts).sum();
    if total == 0 {
        return 0;
    }
    let mut left = cycles % total;
    for (index, delay) in delays.iter().enumerate() {
        if left < lasts(delay) {
            return index;
        }
        left -= lasts(delay);
    }
    0
}

pub struct PlayerGeometryUpdate {
    pub token: u64,
    pub geometry: SceneGeometry,
    pub errors: Vec<String>,
}

/// Geometry of one player: its own entity layer, replaced whole by each result.
pub fn extract_player_frame(
    definitions: &mut WorldDefinitions,
    player: &PlayerFrame,
    hd: Option<&HdContext>,
) -> PlayerGeometryUpdate {
    let mut geometry = SceneGeometry::default();
    let mut errors = Vec::new();
    if let Err(e) = extract_player(definitions, player, hd, &mut geometry) {
        errors.push(format!("player: {e}"));
    }
    for zone in &mut geometry.zones {
        for group in &mut zone.groups {
            group.geometry.finish_hd();
        }
    }
    PlayerGeometryUpdate {
        token: player.token,
        geometry,
        errors,
    }
}

fn extract_player(
    definitions: &mut WorldDefinitions,
    player: &PlayerFrame,
    hd: Option<&HdContext>,
    geometry: &mut SceneGeometry,
) -> Result<(), String> {
    let appearance = definitions.compose_player(&player.look)?;
    let Some(base) = definitions.player_base(&appearance)? else {
        return Err("this look has no models".into());
    };
    let posed = match player.sequence {
        None => None,
        Some(id) => {
            let sequence = definitions
                .sequence(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("missing sequence {id}"))?;
            if sequence.skeletal_animation.is_some() {
                return Err(format!("sequence {id} is skeletal; not posed"));
            }
            let index = frame_at(&sequence.frame_delays, player.cycles);
            let frame_id = sequence
                .frame_ids
                .get(index)
                .ok_or_else(|| format!("sequence {id} has no frame {index}"))?;
            definitions
                .legacy_frame(*frame_id)
                .map_err(|e| e.to_string())?
        }
    };
    let model = pose_legacy_actor(
        &base,
        None,
        posed.as_deref(),
        None,
        osrs_core::entity::EntityScale::IDENTITY,
    )
    .map_err(|e| e.to_string())?;
    emit(
        geometry,
        hd,
        &model,
        player.location,
        player.position,
        player.rotation,
        player.ground_height,
        u32::MAX,
    );
    Ok(())
}

/// Place a posed model in the entity geometry (HD faces when the HD profile is on).
#[allow(clippy::too_many_arguments)]
fn emit(
    geometry: &mut SceneGeometry,
    hd: Option<&HdContext>,
    model: &osrs_core::lighting::ReferenceLitModel,
    location: ActorLocation,
    position: (i32, i32),
    rotation: u16,
    height: i32,
    override_id: u32,
) {
    let (x, z) = position;
    let zone_x = x.div_euclid(1024);
    let zone_z = z.div_euclid(1024);
    let placement = ModelPlacement {
        x: x - zone_x * 1024,
        y: height,
        z: z - zone_z * 1024,
        orientation: rotation,
    };
    let builder = geometry
        .zone_mut(zone_x, zone_z)
        .group_mut(location.plane.get(), location.plane.get());
    if let Some(hd) = hd {
        let world = (
            location.tile.x,
            location.tile.y,
            i32::from(location.plane.get()),
        );
        let override_ = hd.models.get_npc(override_id, world, &hd.areas);
        let faces = model_faces_resolved(
            &hd.materials,
            hd.settings.hide_fake_shadows(),
            override_,
            model,
            ModelSite {
                object_id: override_id,
                world,
                placement,
                zone_origin_world: (zone_x * 1024, zone_z * 1024),
            },
        );
        builder.push_hd_faces(&faces, [placement.x, placement.y, placement.z]);
    } else {
        builder.push_model(model, placement);
    }
}

#[cfg(test)]
mod tests {
    use super::frame_at;

    #[test]
    fn a_looping_sequence_shows_each_frame_for_its_delay_then_repeats() {
        let delays = [3, 2, 0];
        let shown: Vec<usize> = (0..8).map(|cycle| frame_at(&delays, cycle)).collect();
        // Frame 0 for 3 cycles, frame 1 for 2, frame 2 (delay 0 counts as 1) for 1, then again.
        assert_eq!(shown, [0, 0, 0, 1, 1, 2, 0, 0]);
        assert_eq!(frame_at(&[], 5), 0);
        assert_eq!(frame_at(&delays, 6 * 1_000_000), 0);
    }
}
