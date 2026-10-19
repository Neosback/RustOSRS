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
        self.generate_configured(snapshot, variation, max_rooms, None)
    }

    pub fn generate_in_rooms(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        sketch: &crate::BuildingSketch,
    ) -> Result<MapPatch, KitGenerateError> {
        let building = crate::plan_authored_rooms(snapshot.request(), sketch)
            .map_err(KitGenerateError::Design)?;
        self.generate_configured(snapshot, variation, None, Some(building))
    }

    fn generate_configured(
        &self,
        snapshot: &GenerationSnapshot,
        variation: u32,
        max_rooms: Option<usize>,
        authored: Option<crate::BspBuildingPlan>,
    ) -> Result<MapPatch, KitGenerateError> {
        let wrap = KitGenerateError::Generation;
        let request = snapshot.request();
        if !(1..=3).contains(&request.floors) {
            return Err(wrap(BuildingGenerationError::UnsupportedFloorCount {
                floors: request.floors,
            }));
        }
        let extra_stories = request.floors.saturating_sub(1);
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
        let interior =
            u32::from(site.width().saturating_sub(2)) * u32::from(site.height().saturating_sub(2));
        let mut program = building_program(floor0_request.archetype).fitted_to_area(interior);
        if let Some(count) = max_rooms {
            program = program.limited_to(count);
        }
        let building = if let Some(building) = authored {
            building
        } else {
            let layout = pack_building_program(&program, site, floor0_request.seed)
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

        let body = if let (Some(hip), None) = (self.hip, &self.patterns) {
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
        self.stack_upper_floors(snapshot, request, plan, &architecture, ground_and_roof)
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
        let vertical = crate::plan_vertical_connectivity(request, &floor_tiles)
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

        let stair_orientations =
            [0, 1, 2, 3].map(|o| ObjectOrientation::new(o).unwrap_or_else(|| unreachable!()));
        let stair_palette = crate::StairPalette {
            up: crate::StairAsset {
                object_id: ObjectId::new(16683),
                kind: LocKind::new(10).unwrap_or_else(|| unreachable!()),
                orientations: stair_orientations,
            },
            down: crate::StairAsset {
                object_id: ObjectId::new(16679),
                kind: LocKind::new(10).unwrap_or_else(|| unreachable!()),
                orientations: stair_orientations,
            },
        };
        if let Ok(stairs) = crate::build_stair_patch(&vertical, stair_palette) {
            ops.extend_from_slice(stairs.ops());
        }
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
