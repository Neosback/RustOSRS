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
    scenario::NpcSpawnId,
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
            let x = actor.position.x.units();
            let z = actor.position.z.units();
            let zone_x = x.div_euclid(1024);
            let zone_z = z.div_euclid(1024);
            let placement = ModelPlacement {
                x: x - zone_x * 1024,
                y: height,
                z: z - zone_z * 1024,
                orientation: actor.rotation,
            };
            let builder = geometry
                .zone_mut(zone_x, zone_z)
                .group_mut(actor.location.plane.get(), actor.location.plane.get());
            if let Some(hd) = hd {
                let world = (
                    actor.location.tile.x,
                    actor.location.tile.y,
                    i32::from(actor.location.plane.get()),
                );
                let override_ = hd.models.get_npc(id.get(), world, &hd.areas);
                let faces = model_faces_resolved(
                    &hd.materials,
                    hd.settings.hide_fake_shadows(),
                    override_,
                    &model,
                    ModelSite {
                        object_id: id.get(),
                        world,
                        placement,
                        zone_origin_world: (zone_x * 1024, zone_z * 1024),
                    },
                );
                builder.push_hd_faces(&faces, [placement.x, placement.y, placement.z]);
            } else {
                builder.push_model(&model, placement);
            }
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
