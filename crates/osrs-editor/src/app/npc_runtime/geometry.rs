//! FIFO actor extraction and frame-boundary publication; simulation stays in the parent owner.
use super::*;
use crate::app::world_runtime::RegionArtifactIdentity;
use osrs_world::npc::NpcFrameInput;

pub(super) struct NpcRequestIdentity {
    pub(super) key: SentKey,
    ground_inputs: BTreeMap<(i32, i32), Arc<RegionArtifactIdentity>>,
    selected_actors: usize,
}

pub(in crate::app) struct NpcArtifactIdentity {
    request: Arc<NpcRequestIdentity>,
    context: osrs_world::npc::ActorGeometryContext,
}

impl NpcArtifactIdentity {
    pub(in crate::app) fn describe(&self, tile: osrs_core::coords::MapTile) -> Vec<String> {
        let key = &self.request.key;
        let mut lines = vec![
            format!(
                "NPC {:?}; tick {}; cycle {}; ground version {}; presentation {}",
                key.stamp.identity,
                key.stamp.server_tick,
                key.stamp.client_cycle,
                key.infos_version,
                key.presentation_generation
            ),
            format!(
                "camera {:?}; range {}; limit {}; {} selected actors; {} vars",
                key.camera,
                key.distance_tiles,
                key.max_drawn,
                self.request.selected_actors,
                key.varps.len()
            ),
            format!(
                "actor extraction {:?}; profile {}; digest {}; decoder schema {}",
                self.context.render_path,
                self.context.target.profile_id(),
                self.context.target.profile_digest(),
                self.context.target.decoder_schema_version()
            ),
            format!("cache {}", self.context.target.cache_fingerprint()),
        ];
        let region = tile.split_region().0;
        if let Some(identity) = self.request.ground_inputs.get(&(region.x, region.y)) {
            lines.extend(identity.describe());
        } else {
            lines.push("No ground input from this region.".into());
        }
        lines
    }
}

impl NpcRuntime {
    /// Match the issued snapshot before retiring its slot. Publication waits for update().
    pub(in crate::app) fn geometry_arrived(&mut self, update: NpcGeometryUpdate) {
        if self.geometry_inflight != Some(update.stamp) {
            self.stale_rejected += 1;
            return;
        }
        self.geometry_inflight = None;
        if self
            .geometry_sent
            .as_ref()
            .is_some_and(|sent| sent.key.stamp == update.stamp)
        {
            self.geometry_pending = Some(update);
        } else {
            self.stale_rejected += 1;
        }
    }

    /// Remove the entity layer from the screen (once).
    pub(super) fn hide(&mut self, renderer: &mut RenderRuntime) {
        if self.geometry_shown {
            renderer.upload_transient(NPC_KEY, &SceneGeometry::default());
            self.geometry_shown = false;
        }
        // Keep the physical FIFO slot until its completion drains. A hide/show can otherwise
        // issue the same tick stamp twice, which the unchanged worker protocol cannot distinguish.
        self.geometry_sent = None;
        self.geometry_pending = None;
    }

    pub(super) fn request_geometry(&mut self, host: &mut NpcHost<'_>) {
        if !host.display.models {
            self.hide(host.renderer);
            return;
        }
        let Some(live) = &self.live else {
            return;
        };
        // Arrival is staged by stream(); reconcile authored/display inputs before publishing.
        if let Some(update) = self.geometry_pending.take() {
            let current = self
                .geometry_sent
                .as_ref()
                .filter(|sent| {
                    // Compare the issued stamp at arrival, but allow ordinary simulation advancement:
                    // demanding the latest client cycle here would starve a continuously running actor.
                    identity_current(update.stamp.identity, live.sim.identity())
                        && sent.key.infos_version == host.world.infos_version()
                        && host
                            .world
                            .is_current_generation(update.context.presentation_generation)
                        && sent.key.presentation_generation
                            == update.context.presentation_generation
                        && sent.key.camera == host.camera
                        && sent.key.distance_tiles == host.display.distance_tiles
                        && sent.key.max_drawn == host.display.max_drawn
                        && sent.key.varps == host.varps
                })
                .cloned();
            if let Some(request) = current {
                host.renderer.upload_npc(
                    Arc::new(NpcArtifactIdentity {
                        request,
                        context: update.context,
                    }),
                    &update.geometry,
                );
                self.geometry_shown = !update.geometry.zones.is_empty();
                for error in update.errors.into_iter().take(16) {
                    self.report(error);
                }
            } else {
                self.stale_rejected += 1;
                self.geometry_sent = None;
            }
        }
        let Some(live) = &self.live else {
            return;
        };
        if self.geometry_inflight.is_some() {
            return;
        }
        let key = SentKey {
            stamp: live.sim.snapshot_stamp(),
            infos_version: host.world.infos_version(),
            presentation_generation: host.world.presentation_generation(),
            camera: host.camera,
            distance_tiles: host.display.distance_tiles,
            max_drawn: host.display.max_drawn,
            varps: host.varps.clone(),
        };
        if self
            .geometry_sent
            .as_ref()
            .is_some_and(|sent| sent.key == key)
        {
            return;
        }
        let (cx, cy) = host.camera;
        let radius = i64::from(host.display.distance_tiles);
        let mut snapshot = live.sim.snapshot_where(|actor| {
            let dx = i64::from(actor.location.tile.x - cx);
            let dy = i64::from(actor.location.tile.y - cy);
            dx * dx + dy * dy <= radius * radius
        });
        if snapshot.actors.len() > host.display.max_drawn {
            snapshot.actors.sort_by_key(|actor| {
                let dx = i64::from(actor.location.tile.x - cx);
                let dy = i64::from(actor.location.tile.y - cy);
                dx * dx + dy * dy
            });
            snapshot.actors.truncate(host.display.max_drawn);
        }
        let mut ground_inputs = BTreeMap::new();
        let ground_heights = snapshot
            .actors
            .iter()
            .filter_map(|actor| {
                let ground = host.world.actor_ground_sample(
                    (actor.position.x.units(), actor.position.z.units()),
                    actor.location.plane.get(),
                )?;
                let region = ground.identity.region();
                ground_inputs
                    .entry((region.x, region.y))
                    .or_insert_with(|| Arc::clone(ground.identity));
                Some((actor.spawn, ground.height))
            })
            .collect();
        self.geometry_inflight = Some(snapshot.stamp);
        self.geometry_sent = Some(Arc::new(NpcRequestIdentity {
            key,
            ground_inputs,
            selected_actors: snapshot.actors.len(),
        }));
        host.world.request_npc_geometry(NpcFrameInput {
            snapshot,
            ground_heights,
            varps: host.varps.clone(),
        });
    }
}
