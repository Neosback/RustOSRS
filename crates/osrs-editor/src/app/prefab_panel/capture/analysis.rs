//! Immutable source analysis off the frame thread.
use super::*;

pub(in crate::app) fn start(app: &mut EditorApp, tile: MapTile) {
    start_selection(app, tile, false, true);
}

pub(in crate::app) fn start_selection(
    app: &mut EditorApp,
    tile: MapTile,
    additive: bool,
    open: bool,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let hit = super::refine::hover_placement(app).filter(|p| p.position.tile == tile);
        let source = app.project.source().clone();
        let book = app.names.book_arc();
        let custom = std::sync::Arc::new(app.names.custom().book().clone());
        let model = app.prefabs.build.learned_roles();
        let generation = app.project.generation();
        let revision = app.session.map_revision();
        let document = app.session.document().clone();
        let plane = app.interaction.view_plane();
        let available: BTreeSet<_> = app.session.available_regions().collect();
        let (send, receive) = std::sync::mpsc::channel();
        app.prefabs.capture_review.pending = Some(receive);
        app.prefabs.capture_review.review = None;
        app.prefabs.capture_review.refining = true;
        app.prefabs.capture_review.open = open;
        app.prefabs.capture_review.additive = additive;
        std::thread::spawn(move || {
            let result = (|| {
                let mut definitions =
                    osrs_world::WorldDefinitions::open(source).map_err(|e| e.to_string())?;
                let provenance = format!(
                    "{} build {} cache {} · placement rules melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc",
                    definitions.profile_id(),
                    definitions.build(),
                    definitions.cache().fingerprint()
                );
                let region = tile.split_region().0;
                let origin = ((region.x - 1) * 64, (region.y - 1) * 64);
                let placements: Vec<_> = document
                    .object_entries()
                    .into_iter()
                    .filter(|p| {
                        (origin.0..origin.0 + 192).contains(&p.position.tile.x)
                            && (origin.1..origin.1 + 192).contains(&p.position.tile.y)
                    })
                    .collect();
                let mut table = ObjectTable::new();
                for p in &placements {
                    if table.contains_key(&p.object_id) {
                        continue;
                    }
                    let d = definitions
                        .object(p.object_id)
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| format!("Missing definition {}", p.object_id.get()))?;
                    let (name, symbol) = osrs_edit::census_name(
                        &custom,
                        p.object_id.get(),
                        d.name.as_deref().unwrap_or_default(),
                        &book
                            .as_ref()
                            .and_then(|b| b.object(p.object_id.get()))
                            .unwrap_or_default()
                            .to_lowercase(),
                    );
                    table.insert(
                        p.object_id,
                        ObjectInfo {
                            name,
                            symbol,
                            actions: d
                                .actions
                                .iter()
                                .flatten()
                                .map(|s| s.to_lowercase())
                                .collect(),
                            size: (
                                u8::try_from(d.size_x)
                                    .map_err(|_| "object footprint exceeds capture limits")?,
                                u8::try_from(d.size_y)
                                    .map_err(|_| "object footprint exceeds capture limits")?,
                            ),
                        },
                    );
                }
                let mut area = CensusArea::new(origin, 192, 192);
                for plane in 0..4 {
                    let p = SourcePlane::new(plane).ok_or("plane")?;
                    for x in origin.0..origin.0 + 192 {
                        for y in origin.1..origin.1 + 192 {
                            let tile = MapTile::new(x, y);
                            let terrain = document.terrain_at(MapPosition::new(tile, p));
                            let height = document.height_at(HeightVertexCoord::new(tile, p));
                            area.set(
                                plane,
                                (x, y),
                                Ground {
                                    underlay: terrain.underlay.map_or(0, |v| v.get() as u16),
                                    overlay: terrain.overlay.map_or(0, |o| o.id.get() as u16),
                                    shape: terrain.overlay.map_or(0, |o| o.shape.get()),
                                    settings: terrain.flags,
                                    height: height.unwrap_or(0),
                                    height_known: height.is_some(),
                                },
                            );
                        }
                    }
                }
                let raw: Vec<_> = placements
                    .iter()
                    .map(|p| Placed {
                        id: p.object_id,
                        kind: p.kind.get(),
                        orientation: p.orientation.get(),
                        tile: (p.position.tile.x, p.position.tile.y),
                        plane: p.position.plane.index().get(),
                    })
                    .collect();
                let mut predictions = BTreeMap::new();
                if let Some(model) = &model
                    && model.sources.contains(&definitions.cache().fingerprint())
                {
                    for p in &raw {
                        let info = &table[&p.id];
                        let mut features: Vec<_> = osrs_scene::object_semantics::split_tokens(
                            &format!("{} {}", info.name, info.symbol),
                        )
                        .into_iter()
                        .map(|t| format!("token:{t}"))
                        .collect();
                        features.push(format!("kind:{}", p.kind));
                        features.push(format!("size:{}x{}", info.size.0, info.size.1));
                        features.extend(info.actions.iter().map(|a| format!("action:{a}")));
                        if let Some(d) = definitions.object(p.id).map_err(|e| e.to_string())?
                            && let Some(category) = d.category
                        {
                            features.push(format!("category:{}", category.get()));
                        }
                        if let Some((role, margin)) = model.predict(&features) {
                            predictions.insert(
                                (p.id, p.kind),
                                format!("{role} · evidence margin {margin}"),
                            );
                        }
                    }
                }
                let mut candidates = osrs_scene::building_census::description::pick_candidates(
                    &area,
                    &raw,
                    &table,
                    tile,
                    plane,
                    hit.map(|p| Placed {
                        id: p.object_id,
                        tile: (p.position.tile.x, p.position.tile.y),
                        plane: p.position.plane.index().get(),
                        kind: p.kind.get(),
                        orientation: p.orientation.get(),
                    }),
                );
                if candidates.is_empty() {
                    if let Some(candidate) =
                        osrs_scene::building_census::description::incomplete_candidate(
                            &area,
                            &raw,
                            &table,
                            tile,
                            plane,
                            hit.map(|p| Placed {
                                id: p.object_id,
                                tile: (p.position.tile.x, p.position.tile.y),
                                plane: p.position.plane.index().get(),
                                kind: p.kind.get(),
                                orientation: p.orientation.get(),
                            }),
                        )
                    {
                        candidates.push(candidate);
                    }
                }
                let sides = candidates.iter().map(|c| (c.seed, c.plane)).collect();
                let seed_description = candidates.into_iter().next().map(|c| c.description)
                    .ok_or("No enclosed building here. Load the surrounding map or select an interior floor tile.")?;
                let census = osrs_scene::building_census::analyze(&area, &raw, &table);
                let mut rooms = Vec::new();
                for b in &census {
                    if !seed_description
                        .floors
                        .get(&b.plane)
                        .is_some_and(|f| b.tiles().any(|(x, y)| f.contains(&MapTile::new(x, y))))
                    {
                        continue;
                    }
                    for r in &b.rooms {
                        let tiles: Vec<_> =
                            r.tiles.iter().map(|&(x, y)| MapTile::new(x, y)).collect();
                        let included = seed_description
                            .floors
                            .get(&r.plane)
                            .is_some_and(|f| tiles.iter().all(|t| f.contains(t)));
                        rooms.push(RoomChoice {
                            plane: r.plane,
                            tiles,
                            included,
                            role: r.tag.word().into(),
                        });
                    }
                }
                for (&plane, floor) in &seed_description.floors {
                    let missing: Vec<_> = floor
                        .iter()
                        .filter(|t| {
                            !rooms
                                .iter()
                                .any(|r| r.plane == plane && r.tiles.contains(t))
                        })
                        .copied()
                        .collect();
                    if !missing.is_empty() {
                        rooms.push(RoomChoice {
                            plane,
                            tiles: missing,
                            included: true,
                            role: "general".into(),
                        });
                    }
                }
                let mut all = seed_description.clone();
                for room in &rooms {
                    all.floors
                        .entry(room.plane)
                        .or_default()
                        .extend(room.tiles.iter().copied());
                }
                let description = osrs_scene::building_census::description::describe_floors(
                    &area, &raw, &table, all,
                );
                for floor in description.floors.values() {
                    for tile in floor {
                        for dx in -1..=1 {
                            for dy in -1..=1 {
                                if !available.contains(
                                    &MapTile::new(tile.x + dx, tile.y + dy).split_region().0,
                                ) {
                                    return Err(
                                        "Load neighboring regions before capturing this building."
                                            .into(),
                                    );
                                }
                            }
                        }
                    }
                }
                let objects = description
                    .members
                    .iter()
                    .map(|m| {
                        placements
                            .iter()
                            .find(|p| {
                                p.object_id == m.placement.id
                                    && p.kind.get() == m.placement.kind
                                    && p.orientation.get() == m.placement.orientation
                                    && p.position.tile
                                        == MapTile::new(m.placement.tile.0, m.placement.tile.1)
                                    && p.position.plane.index().get() == m.placement.plane
                            })
                            .copied()
                            .ok_or_else(|| "placement changed".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let planes = description.floors.keys().copied().collect();
                let included = description
                    .members
                    .iter()
                    .map(|m| {
                        seed_description
                            .members
                            .iter()
                            .any(|n| n.placement == m.placement)
                    })
                    .collect();
                Ok(Review {
                    inputs: std::sync::Arc::new(ReviewInputs {
                        area,
                        placements: raw,
                        table: table.clone(),
                    }),
                    rooms,
                    sides,
                    overrides: vec![],
                    labels: BTreeMap::new(),
                    predictions,
                    source: generation,
                    revision,
                    description,
                    objects,
                    included,
                    planes,
                    object_sizes: table
                        .iter()
                        .map(|(id, d)| (*id, (u16::from(d.size.0), u16::from(d.size.1))))
                        .collect(),
                    provenance,
                    confirmed: false,
                    options: Options::default(),
                })
            })();
            let _ = send.send(result);
        });
        app.prefabs
            .say("Analysing the building from the edited map…");
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = tile;
        app.prefabs
            .say("Building capture review currently requires the desktop app.");
    }
}
