//! Canonical kit realization shared by procedural and author-drawn room layouts.
use super::*;

impl BuildingKit {
    pub fn learned_roles(&self) -> Option<&crate::LearnedRoleModel> {
        self.finish.as_ref()?.learned_roles.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Generate one building for the request in `snapshot`: rooms sized to the site, walls, doors
    /// and windows from the kit, and its roof. `variation` re-rolls the windows and doors without
    /// moving rooms or corridors. Supports 1..=3 floors.
    pub fn generate(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
    ) -> Result<MapPatch, KitGenerateError> {
        self.generate_with_rooms(snapshot, variation, None)
    }

    /// [`Self::generate`] with at most `max_rooms` rooms (the program's most important ones).
    pub fn generate_with_rooms(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        max_rooms: Option<usize>,
    ) -> Result<MapPatch, KitGenerateError> {
        self.generate_with_layout(snapshot, variation, max_rooms, LayoutStyle::default())
    }

    /// [`Self::generate_with_rooms`] with the room layout chosen.
    pub fn generate_with_layout(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        max_rooms: Option<usize>,
        style: LayoutStyle,
    ) -> Result<MapPatch, KitGenerateError> {
        self.generate_configured(snapshot, variation, max_rooms, None, style)
    }

    pub fn generate_in_rooms(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        sketch: &crate::BuildingSketch,
    ) -> Result<MapPatch, KitGenerateError> {
        let building = crate::plan_authored_rooms(snapshot.request(), sketch)
            .map_err(KitGenerateError::Design)?;
        self.generate_configured(
            snapshot,
            variation,
            None,
            Some(building),
            LayoutStyle::default(),
        )
    }

    fn generate_configured(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        max_rooms: Option<usize>,
        authored: Option<crate::BspBuildingPlan>,
        style: LayoutStyle,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let request = snapshot.request();
        if !(1..=3).contains(&request.floors) {
            return Err(wrap(BuildingGenerationError::UnsupportedFloorCount {
                floors: request.floors,
            }));
        }
        let extra_stories = request.floors.saturating_sub(1);
        if extra_stories != 0 && self.stairs.is_none() {
            return Err(KitGenerateError::MissingStairs);
        }
        let roof_offset = extra_stories.saturating_add(self.roof_plane_offset);
        let top_plane = request.base_plane.index().get().saturating_add(roof_offset);
        if osrs_core::coords::SourcePlane::new(top_plane).is_none() {
            return Err(KitGenerateError::RoofPlane { plane: top_plane });
        }

        let floor0_request = crate::BuildingRequest {
            floors: 1,
            ..request
        };
        let floor0_snapshot = snapshot.with_request(floor0_request);

        let site = floor0_request.site;
        // Each storey gets a plan of its own when the layout is generated and the roof is a hip
        // (the hip roof follows any outline, so a smaller floor above is fine). Drawn rooms and
        // the corridor layout keep the older way: the ground floor's walls copied upward.
        let own_storeys = extra_stories != 0
            && authored.is_none()
            && style == LayoutStyle::Compact
            && self.hip.is_some()
            && self.patterns.is_none();
        let building = if let Some(building) = authored {
            building
        } else if style == LayoutStyle::Compact {
            let (program, layout) = plan_compact_layout(
                floor0_request.archetype,
                site,
                floor0_request.seed,
                max_rooms,
            )
            .map_err(|e| wrap(BuildingGenerationError::Layout(e)))?;
            build_compact_building_plan(floor0_request, &program, &layout)
                .map_err(|e| wrap(BuildingGenerationError::Topology(e)))?
        } else {
            let interior = u32::from(site.width().saturating_sub(2))
                * u32::from(site.height().saturating_sub(2));
            let mut program = building_program(floor0_request.archetype).fitted_to_area(interior);
            if let Some(count) = max_rooms {
                program = program.limited_to(count);
            }
            let (program, layout) = pack_fitted_program(&program, site, floor0_request.seed)
                .map_err(|e| wrap(BuildingGenerationError::Layout(e)))?;
            let building = build_bsp_building_plan(floor0_request, &program, &layout)
                .map_err(|e| wrap(BuildingGenerationError::Topology(e)))?;
            route_program_connections(&building)
                .map_err(|e| wrap(BuildingGenerationError::Routing(e)))?;
            building
        };
        let plan: &BuildingPlan = &building.plan;
        let architecture = derive_building_architecture(plan)
            .map_err(|e| wrap(BuildingGenerationError::Architecture(e)))?;
        let refined = refine_exterior_walls_for_variation(
            &architecture,
            floor0_request.seed,
            variation,
            FacadeWfcConfig::default(),
            FacadeRefinementPolicy::default(),
        )
        .map_err(|error| {
            wrap(BuildingGenerationError::Detail(
                BuildingDetailError::Refinement { error },
            ))
        })?;

        let body = if own_storeys {
            // Walls only: the roofs follow once every storey is known.
            build_detailed_wfc_building_variation_patch(
                &floor0_snapshot,
                plan,
                &self.detail,
                variation,
                FacadeWfcConfig::default(),
                FacadeRefinementPolicy::default(),
            )
            .map_err(|e| wrap(BuildingGenerationError::Detail(e)))?
        } else if let (Some(hip), None) = (self.hip, &self.patterns) {
            self.with_hip_roof(
                &floor0_snapshot,
                plan,
                &architecture,
                hip,
                variation,
                roof_offset,
            )?
        } else {
            let Some(roof) = &self.patterns else {
                // `compile` guarantees one of the two roofs.
                return Err(wrap(BuildingGenerationError::UnsupportedFloorCount {
                    floors: 0,
                }));
            };
            let patch = generate_building_plan_patch(
                &floor0_snapshot,
                plan,
                BuildingGenerationConfig {
                    detail_palette: &self.detail,
                    facade_variation: variation,
                    facade_config: FacadeWfcConfig::default(),
                    facade_policy: FacadeRefinementPolicy::default(),
                    roof_catalog: &roof.catalog,
                    roof_patterns: roof.patterns,
                    roof_palette: &roof.palette,
                    roof_junction_patterns: &roof.junctions,
                    roof_max_backtracks: 64,
                },
            )
            .map_err(wrap)?;
            self.lift_roof(patch, roof_offset)
        };
        // One wall piece per tile, or the game drops the extras.
        let (body, unplaced) = crate::resolve_wall_slots(&body);
        if unplaced != 0 {
            return Err(KitGenerateError::Design(
                "Wall edges exceed available scene slots; change this room layout",
            ));
        }
        let ground_and_roof = match &self.finish {
            Some(finish) => finish.apply(&body, &building, refined.edges(), &floor0_snapshot),
            None => body,
        };
        if extra_stories == 0 {
            return Ok(ground_and_roof);
        }
        if own_storeys {
            return self.build_storeys(
                snapshot,
                &building,
                &architecture,
                ground_and_roof,
                variation,
            );
        }
        self.stack_upper_floors(snapshot, request, plan, &architecture, ground_and_roof)
    }

    /// Replace each exterior door of `plan` in `body` with the kit's exterior wall: an upper floor
    /// has no way in from outside, but its plan still needs the one entrance to be valid.
    fn exterior_doors_to_walls(&self, body: &MapPatch, plan: &BuildingPlan) -> MapPatch {
        let doorways: std::collections::BTreeSet<_> = plan
            .connections
            .iter()
            .filter(|c| c.from.is_none())
            .map(|c| c.doorway)
            .collect();
        let mut door_ids: std::collections::BTreeSet<ObjectId> =
            std::collections::BTreeSet::from([self.detail.base.door.object_id]);
        for variant in &self.detail.door_variants {
            door_ids.insert(variant.asset.object_id);
        }
        let ops: Vec<MapOp> = body
            .ops()
            .iter()
            .map(|op| match *op {
                MapOp::AddObject { mut placement }
                    if (0..=9).contains(&placement.kind.get())
                        && door_ids.contains(&placement.object_id)
                        && doorways.contains(&placement.position.tile) =>
                {
                    placement.object_id = self.detail.base.exterior_wall.object_id;
                    placement.kind = self.detail.base.exterior_wall.kind;
                    MapOp::AddObject { placement }
                }
                other => other,
            })
            .collect();
        MapPatch::new(ops)
    }

    /// A building whose every storey has its own rooms: each upper floor is laid out inside a
    /// rectangle on the floor below (see `floors::upper_site`), walled, windowed and furnished
    /// like a ground floor, stairs join the floors where both stand in one room, and each exposed
    /// level (the top, and any step back) gets its own hip roof.
    fn build_storeys(
        &self,
        snapshot: &GenerationSnapshot,
        ground: &crate::BspBuildingPlan,
        ground_architecture: &BuildingArchitecture,
        ground_patch: MapPatch,
        variation: u32,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let request = snapshot.request();
        let hip = self
            .hip
            .ok_or(KitGenerateError::Design("Storeys need a hip roof kit"))?;
        let base = request.base_plane.index().get();
        let plane_of = |floor: u8| {
            let value = base.saturating_add(floor);
            osrs_core::coords::SourcePlane::new(value)
                .ok_or(KitGenerateError::RoofPlane { plane: value })
        };
        let mut floor_tiles: Vec<std::collections::BTreeSet<osrs_core::coords::MapTile>> =
            vec![ground_architecture.floor_tiles().iter().copied().collect()];
        let mut plans = vec![ground.clone()];
        let mut ops: Vec<MapOp> = ground_patch.ops().to_vec();

        for floor in 1..request.floors {
            let site = super::floors::upper_site(
                &floor_tiles[usize::from(floor) - 1],
                request.seed,
                floor,
            )
            .ok_or(KitGenerateError::Design(
                "No room fits on top of the floor below; use fewer floors",
            ))?;
            let floor_seed = request.seed ^ (u64::from(floor) << 40) ^ 0x570e_1e5f_100a_0001;
            let floor_request = crate::BuildingRequest {
                site,
                base_plane: plane_of(floor)?,
                floors: 1,
                ..request
            };
            let (program, layout) = plan_compact_layout_for(
                upper_floor_program(request.archetype),
                site,
                floor_seed,
                None,
            )
            .map_err(|e| wrap(BuildingGenerationError::Layout(e)))?;
            let plan_k = build_compact_building_plan(floor_request, &program, &layout)
                .map_err(|e| wrap(BuildingGenerationError::Topology(e)))?;
            let architecture = derive_building_architecture(&plan_k.plan)
                .map_err(|e| wrap(BuildingGenerationError::Architecture(e)))?;
            let refined = refine_exterior_walls_for_variation(
                &architecture,
                floor_request.seed,
                variation,
                FacadeWfcConfig::default(),
                FacadeRefinementPolicy::default(),
            )
            .map_err(|error| {
                wrap(BuildingGenerationError::Detail(
                    BuildingDetailError::Refinement { error },
                ))
            })?;
            let floor_snapshot =
                snapshot
                    .for_floor(floor, site)
                    .ok_or(KitGenerateError::Design(
                        "The floor above is outside the site or its terrain is missing",
                    ))?;
            let walls = build_detailed_wfc_building_variation_patch(
                &floor_snapshot,
                &plan_k.plan,
                &self.detail,
                variation,
                FacadeWfcConfig::default(),
                FacadeRefinementPolicy::default(),
            )
            .map_err(|e| wrap(BuildingGenerationError::Detail(e)))?;
            let walls = self.exterior_doors_to_walls(&walls, &plan_k.plan);
            let (walls, unplaced) = crate::resolve_wall_slots(&walls);
            if unplaced != 0 {
                return Err(KitGenerateError::Design(
                    "Wall edges exceed available scene slots on an upper floor; change this room layout",
                ));
            }
            let finished = match &self.finish {
                Some(finish) => {
                    finish.apply_to_floor(&walls, &plan_k, refined.edges(), &floor_snapshot, false)
                }
                None => walls,
            };
            ops.extend_from_slice(finished.ops());
            floor_tiles.push(architecture.floor_tiles().iter().copied().collect());
            plans.push(plan_k);
        }

        // Stairs: one room on both floors, off the doors, against a wall where possible.
        let layouts: Vec<crate::FloorLayout> = plans
            .iter()
            .zip(&floor_tiles)
            .map(|(plan, tiles)| floor_layout(&plan.plan, tiles))
            .collect();
        let vertical = crate::plan_vertical_connectivity_in_layouts(request, &layouts)
            .map_err(|e| wrap(BuildingGenerationError::Vertical(e)))?;
        let mut reserved: Vec<std::collections::BTreeSet<osrs_core::coords::MapTile>> =
            vec![std::collections::BTreeSet::new(); usize::from(request.floors)];
        for stair in vertical.stairs() {
            for floor in [stair.lower_floor, stair.upper_floor] {
                reserved[usize::from(floor.get())].insert(stair.tile);
                if let Some(approach) = stair.approach_tile() {
                    reserved[usize::from(floor.get())].insert(approach);
                }
            }
        }
        ops.retain(|op| {
            !matches!(op, MapOp::AddObject { placement }
                if placement.kind.get() == 10
                    && placement
                        .position
                        .plane
                        .index()
                        .get()
                        .checked_sub(base)
                        .is_some_and(|floor| reserved
                            .get(usize::from(floor))
                            .is_some_and(|tiles| tiles.contains(&placement.position.tile))))
        });
        let stairs = crate::build_stair_patch(
            &vertical,
            self.stairs.ok_or(KitGenerateError::MissingStairs)?,
        )
        .map_err(KitGenerateError::Stairs)?;
        ops.extend_from_slice(stairs.ops());

        // Roofs: each level's exposed part under a hip of its own.
        for floor in 0..request.floors {
            let plane = plane_of(floor.saturating_add(self.roof_plane_offset))?;
            let footprint = super::floors::roof_footprint(
                &floor_tiles[usize::from(floor)],
                floor_tiles.get(usize::from(floor) + 1),
            );
            ops.extend(
                hip_roof_placements(&footprint, hip, plane)
                    .into_iter()
                    .map(|placement| MapOp::AddObject { placement }),
            );
        }
        Ok(MapPatch::new(ops))
    }

    fn stack_upper_floors(
        &self,
        snapshot: &GenerationSnapshot,
        request: crate::BuildingRequest,
        plan: &BuildingPlan,
        architecture: &BuildingArchitecture,
        ground_and_roof: MapPatch,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let floor_tiles = vec![architecture.floor_tiles().to_vec(); usize::from(request.floors)];
        // The tiles either side of every door and doorway stay free of stairs.
        let mut keep_clear = std::collections::BTreeSet::new();
        for connection in &plan.connections {
            let doorway = connection.doorway;
            keep_clear.insert(doorway);
            for direction in crate::CardinalDirection::ALL {
                if let Some(side) = direction.neighbor(doorway) {
                    keep_clear.insert(side);
                }
            }
        }
        let one_layout = crate::FloorLayout {
            tiles: architecture.floor_tiles().to_vec(),
            keep_clear,
            ..crate::FloorLayout::default()
        };
        let _ = floor_tiles;
        let vertical = crate::plan_vertical_connectivity_in_layouts(
            request,
            &vec![one_layout; usize::from(request.floors)],
        )
        .map_err(|e| wrap(BuildingGenerationError::Vertical(e)))?;

        let mut stair_reserved = std::collections::BTreeSet::new();
        for stair in vertical.stairs() {
            stair_reserved.insert(stair.tile);
            if let Some(approach) = stair.approach_tile() {
                stair_reserved.insert(approach);
            }
        }

        let exterior_door_tiles: std::collections::BTreeSet<_> = plan
            .connections
            .iter()
            .filter(|c| c.from.is_none())
            .map(|c| c.doorway)
            .collect();
        let mut door_ids: std::collections::BTreeSet<ObjectId> =
            std::collections::BTreeSet::from([self.detail.base.door.object_id]);
        for variant in &self.detail.door_variants {
            door_ids.insert(variant.asset.object_id);
        }

        let mut ops = Vec::new();
        for &op in ground_and_roof.ops() {
            if let MapOp::AddObject { placement } = op
                && placement.position.plane == request.base_plane
                && placement.kind.get() == 10
                && stair_reserved.contains(&placement.position.tile)
            {
                continue;
            }
            ops.push(op);
        }

        for story in 1..request.floors {
            let Some(story_plane) = osrs_core::coords::SourcePlane::new(
                request.base_plane.index().get().saturating_add(story),
            ) else {
                continue;
            };
            for &op in ground_and_roof.ops() {
                match op {
                    MapOp::SetTerrain { coord, after, .. } if coord.plane == request.base_plane => {
                        if let Some(before) = snapshot.terrain_at_plane(coord.tile, story_plane)
                            && before != after
                        {
                            ops.push(MapOp::SetTerrain {
                                coord: crate::MapPosition::new(coord.tile, story_plane),
                                before,
                                after,
                            });
                        }
                    }
                    MapOp::AddObject { mut placement }
                        if placement.position.plane == request.base_plane
                            && (0..=9).contains(&placement.kind.get()) =>
                    {
                        placement.position.plane = story_plane;
                        if door_ids.contains(&placement.object_id)
                            && exterior_door_tiles.contains(&placement.position.tile)
                        {
                            placement.object_id = self.detail.base.exterior_wall.object_id;
                            placement.kind = self.detail.base.exterior_wall.kind;
                        }
                        ops.push(MapOp::AddObject { placement });
                    }
                    _ => {}
                }
            }
        }

        let stairs = crate::build_stair_patch(
            &vertical,
            self.stairs.ok_or(KitGenerateError::MissingStairs)?,
        )
        .map_err(KitGenerateError::Stairs)?;
        ops.extend_from_slice(stairs.ops());
        Ok(MapPatch::new(ops))
    }

    /// The walls and floor from the facade pipeline, then a hip roof over the footprint and one
    /// tile beyond it (real roofs overhang the walls).
    fn with_hip_roof(
        &self,
        snapshot: &GenerationSnapshot,
        plan: &BuildingPlan,
        architecture: &BuildingArchitecture,
        hip: HipRoofAssets,
        variation: u32,
        roof_offset: u8,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let body = build_detailed_wfc_building_variation_patch(
            snapshot,
            plan,
            &self.detail,
            variation,
            FacadeWfcConfig::default(),
            FacadeRefinementPolicy::default(),
        )
        .map_err(|e| wrap(BuildingGenerationError::Detail(e)))?;
        let base = plan.request.base_plane.index().get();
        let plane = base.saturating_add(roof_offset);
        let plane = osrs_core::coords::SourcePlane::new(plane)
            .ok_or(KitGenerateError::RoofPlane { plane })?;
        let mut footprint: std::collections::BTreeSet<osrs_core::coords::MapTile> =
            std::collections::BTreeSet::new();
        for tile in architecture.floor_tiles() {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    footprint.insert(osrs_core::coords::MapTile::new(tile.x + dx, tile.y + dy));
                }
            }
        }
        let footprint: Vec<_> = footprint.into_iter().collect();
        let mut ops: Vec<MapOp> = body.ops().to_vec();
        ops.extend(
            hip_roof_placements(&footprint, hip, plane)
                .into_iter()
                .map(|placement| MapOp::AddObject { placement }),
        );
        Ok(MapPatch::new(ops))
    }

    /// Move roof pieces up by the kit's roof plane offset (plus any upper floors).
    fn lift_roof(&self, patch: MapPatch, roof_offset: u8) -> MapPatch {
        if roof_offset == 0 {
            return patch;
        }
        let mut ops = Vec::with_capacity(patch.ops().len());
        for op in patch.ops() {
            ops.push(match *op {
                MapOp::AddObject { mut placement } if (12..=21).contains(&placement.kind.get()) => {
                    let plane = placement
                        .position
                        .plane
                        .index()
                        .get()
                        .saturating_add(roof_offset);
                    // Above the top plane there is nowhere to put a roof: keep it where it is.
                    if let Some(plane) = osrs_core::coords::SourcePlane::new(plane) {
                        placement.position.plane = plane;
                    }
                    MapOp::AddObject { placement }
                }
                other => other,
            });
        }
        MapPatch::new(ops)
    }
}

/// The stair planner's view of one floor: its tiles, the room of each, and the tiles beside every
/// door or doorway (and the doorway itself), which a stair may not use.
fn floor_layout(
    plan: &BuildingPlan,
    tiles: &std::collections::BTreeSet<osrs_core::coords::MapTile>,
) -> crate::FloorLayout {
    let mut room_of = std::collections::BTreeMap::new();
    for room in &plan.rooms {
        for dy in 0..room.rect.height {
            for dx in 0..room.rect.width {
                room_of.insert(
                    osrs_core::coords::MapTile::new(
                        room.rect.origin.x + i32::from(dx),
                        room.rect.origin.y + i32::from(dy),
                    ),
                    room.id.get(),
                );
            }
        }
    }
    let mut keep_clear = std::collections::BTreeSet::new();
    for connection in &plan.connections {
        keep_clear.insert(connection.doorway);
        for direction in crate::CardinalDirection::ALL {
            if let Some(side) = direction.neighbor(connection.doorway) {
                keep_clear.insert(side);
            }
        }
    }
    crate::FloorLayout {
        tiles: tiles.iter().copied().collect(),
        room_of,
        keep_clear,
    }
}
