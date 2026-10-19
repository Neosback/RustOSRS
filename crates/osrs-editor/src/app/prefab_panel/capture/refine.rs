//! Transient selection corrections; the document remains the only editing authority.
use super::*;
use osrs_scene::building_census::description::{Evidence, Member, MemberRole};
use osrs_scene::object_semantics::Placed;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum Layer {
    Rooms,
    #[default]
    Floor,
    Pieces,
    Upper,
}

pub(in crate::app) fn hover_placement(app: &EditorApp) -> Option<ObjectPlacement> {
    let pick = app.interaction.hover().object.as_ref()?;
    let loc = app.world.infos().get(&pick.region)?.locs.get(pick.index)?;
    let p = ObjectPlacement {
        object_id: loc.definition.identity.id,
        position: MapPosition::new(
            MapTile::new(loc.tile.0, loc.tile.1),
            SourcePlane::new(loc.plane)?,
        ),
        kind: osrs_edit::LocKind::new(loc.loc_type)?,
        orientation: osrs_edit::ObjectOrientation::new(loc.orientation)?,
    };
    app.session.document().contains_object(p).then_some(p)
}

pub(super) fn project(app: &mut EditorApp, review: &Review, additive: bool) {
    if !additive {
        app.interaction.clear_selection();
    }
    let plane = app.interaction.view_plane();
    for room in review
        .rooms
        .iter()
        .filter(|r| r.included && r.plane == plane)
    {
        for tile in &room.tiles {
            app.interaction.add_tile((tile.x, tile.y));
        }
    }
    for (region, info) in app.world.infos() {
        for (index, loc) in info.locs.iter().enumerate() {
            let selected = review.objects.iter().zip(&review.included).any(|(p, yes)| {
                *yes && p.object_id == loc.definition.identity.id
                    && (p.position.tile.x, p.position.tile.y) == loc.tile
                    && p.position.plane.index().get() == loc.plane
                    && p.kind.get() == loc.loc_type
                    && p.orientation.get() == loc.orientation
            });
            if selected {
                app.interaction.add_object(
                    crate::pick::ObjectPick {
                        region: *region,
                        index,
                    },
                    None,
                );
            }
        }
    }
}

pub(in crate::app) fn cancel(app: &mut EditorApp) {
    app.prefabs.capture_review = CaptureReview::default();
}

pub(in crate::app) fn open(app: &mut EditorApp) {
    if !app.prefabs.capture_review.refining {
        return;
    }
    app.prefabs.capture_review.open = true;
    app.settings.prefs.properties_panel = true;
    app.settings.prefs.rail_tab = crate::settings::RailTab::Prefabs;
    app.prefabs.page = crate::app::prefab_runtime::Page::Library;
}

pub(in crate::app) fn keys(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.prefabs.capture_review.refining || app.keys_blocked(ctx) {
        return;
    }
    let pressed = |id| ctx.input(|i| app.settings.keymap.pressed(i, id));
    let [review, grow, shrink, clear] = [
        "building.review",
        "building.grow",
        "building.shrink",
        "selection.clear",
    ]
    .map(pressed);
    if review {
        open(app);
    }
    if grow {
        resize(app, true);
    }
    if shrink {
        resize(app, false);
    }
    if clear {
        cancel(app);
    }
}

pub(in crate::app) fn window(app: &mut EditorApp, ctx: &egui::Context) {
    keys(app, ctx);
    if !app.prefabs.capture_review.refining {
        return;
    }
    if app.prefabs.capture_review.review.as_ref().is_some_and(|r| {
        (r.source, r.revision) != (app.project.generation(), app.session.map_revision())
    }) {
        cancel(app);
        app.ui
            .notify("The map changed. Select the building again before capture.");
        return;
    }
    egui::Window::new("Building selection")
        .id(egui::Id::new("building-refine"))
        .default_width(310.0)
        .resizable(true)
        .show(ctx, |ui| {
            controls(app, ui);
            ui.horizontal(|ui| {
                if ui.button("Review capture").clicked() {
                    open(app);
                }
                if ui.button("Cancel").clicked() {
                    cancel(app);
                }
            });
        });
}

pub(super) fn controls(app: &mut EditorApp, ui: &mut egui::Ui) {
    let state = &mut app.prefabs.capture_review;
    ui.label("Shift adds · Alt removes · [ / ] shrink / grow floor");
    ui.horizontal_wrapped(|ui| {
        for (layer, text) in [
            (Layer::Rooms, "Rooms"),
            (Layer::Floor, "Floor"),
            (Layer::Pieces, "Pieces"),
            (Layer::Upper, "Roof / upper"),
        ] {
            ui.selectable_value(&mut state.layer, layer, text);
        }
    });
    let mut side = None;
    if let Some(review) = &state.review {
        ui.label(format!(
            "{} rooms · {} selected pieces",
            review.rooms.iter().filter(|r| r.included).count(),
            review.included.iter().filter(|&&yes| yes).count()
        ));
        if review.sides.len() > 1 {
            ui.label("Shared wall: choose the interior side to capture.");
            for &(tile, plane) in &review.sides {
                if ui
                    .button(format!("Interior {},{} · floor {plane}", tile.x, tile.y))
                    .clicked()
                {
                    side = Some((tile, plane));
                }
            }
        }
        for issue in &review.description.review {
            ui.label(format!("Review: {issue}"));
        }
    } else {
        ui.spinner();
        ui.label("Analysing the edited map…");
    }
    if let Some((tile, plane)) = side {
        app.interaction.switch_plane(plane);
        analysis::start_selection(app, tile, false, false);
    }
    ui.horizontal(|ui| {
        if ui.button("Shrink floor").clicked() {
            resize(app, false);
        }
        if ui.button("Grow floor").clicked() {
            resize(app, true);
        }
    });
}

/// Correct membership before the ordinary Select controller consumes the gesture.
pub(in crate::app) fn input(app: &mut EditorApp, response: &egui::Response) -> bool {
    if !app.prefabs.capture_review.refining {
        return false;
    }
    let modifiers = response.ctx.input(|i| i.modifiers);
    if !(modifiers.shift || modifiers.alt)
        || !(response.clicked() || response.dragged_by(egui::PointerButton::Primary))
    {
        return false;
    }
    let Some(tile) = app
        .interaction
        .hover()
        .tile
        .map(|p| MapTile::new(p.tile.0, p.tile.1))
    else {
        return true;
    };
    let plane = app.interaction.view_plane();
    let object = hover_placement(app);
    // Dimension evidence may be projected, membership may not. Verify the occurrence above.
    let size = app
        .interaction
        .hover()
        .object
        .as_ref()
        .and_then(|p| app.world.infos().get(&p.region)?.locs.get(p.index))
        .map(|l| (l.definition.size_x, l.definition.size_y));
    let state = &mut app.prefabs.capture_review;
    let Some(review) = &mut state.review else {
        return true;
    };
    let include = !modifiers.alt;
    match state.layer {
        Layer::Floor => change_floor(review, tile, plane, include),
        Layer::Rooms => {
            for room in &mut review.rooms {
                if room.plane == plane && room.tiles.contains(&tile) {
                    room.included = include;
                }
            }
        }
        Layer::Pieces | Layer::Upper => {
            if let Some(object) = object
                && let Some(size) = size
            {
                change_piece(review, object, size, include);
            }
        }
    }
    super::membership::refresh(review);
    review.confirmed = false;
    let review = review.clone();
    project(app, &review, false);
    true
}

pub(super) fn change_floor(review: &mut Review, tile: MapTile, plane: u8, include: bool) {
    for room in &mut review.rooms {
        if room.plane == plane {
            room.tiles.retain(|t| *t != tile);
        }
    }
    if include {
        review.planes.insert(plane);
        if let Some(room) = review
            .rooms
            .iter_mut()
            .find(|r| r.included && r.plane == plane)
        {
            room.tiles.push(tile);
        } else {
            review.rooms.push(RoomChoice {
                plane,
                tiles: vec![tile],
                included: true,
                role: "general".into(),
            });
        }
    }
}

pub(super) fn change_piece(
    review: &mut Review,
    p: ObjectPlacement,
    size: (u16, u16),
    include: bool,
) {
    let index = if let Some(index) = review.objects.iter().position(|o| *o == p) {
        index
    } else {
        let role = match p.kind.get() {
            0..=3 => MemberRole::Boundary,
            4..=8 => MemberRole::Decoration,
            12..=21 => MemberRole::Roof,
            22 => MemberRole::GroundDecoration,
            _ => MemberRole::Furniture,
        };
        review.objects.push(p);
        review.included.push(false);
        review.description.members.push(Member {
            placement: Placed {
                id: p.object_id,
                tile: (p.position.tile.x, p.position.tile.y),
                plane: p.position.plane.index().get(),
                kind: p.kind.get(),
                orientation: p.orientation.get(),
            },
            role,
            evidence: Evidence::FootprintInside,
        });
        review.object_sizes.insert(p.object_id, size);
        review.objects.len() - 1
    };
    review.included[index] = include;
    review.overrides.retain(|(o, _)| *o != p);
    review.overrides.push((p, include));
}

fn resize(app: &mut EditorApp, grow: bool) {
    let plane = app.interaction.view_plane();
    let Some(review) = &mut app.prefabs.capture_review.review else {
        return;
    };
    let floor: BTreeSet<_> = review
        .rooms
        .iter()
        .filter(|r| r.included && r.plane == plane)
        .flat_map(|r| r.tiles.iter().copied())
        .collect();
    let mut changed = floor.clone();
    for tile in &floor {
        let neighbors = [
            (tile.x - 1, tile.y),
            (tile.x + 1, tile.y),
            (tile.x, tile.y - 1),
            (tile.x, tile.y + 1),
        ];
        if grow {
            for (x, y) in neighbors {
                let tile = MapTile::new(x, y);
                if SourcePlane::new(plane).is_some_and(|p| {
                    app.session
                        .document()
                        .height_at(HeightVertexCoord::new(tile, p))
                        .is_some()
                }) {
                    changed.insert(tile);
                }
            }
        } else if neighbors
            .iter()
            .any(|&(x, y)| !floor.contains(&MapTile::new(x, y)))
        {
            changed.remove(tile);
        }
    }
    for tile in floor.difference(&changed) {
        change_floor(review, *tile, plane, false);
    }
    for tile in changed.difference(&floor) {
        change_floor(review, *tile, plane, true);
    }
    super::membership::refresh(review);
    review.confirmed = false;
    let review = review.clone();
    project(app, &review, false);
}
