//! Camera, picking, selection, and in-progress tool gestures.
//!
//! `InteractionRuntime` owns transient input-side state. It emits edit intents that the
//! application routes to `EditorSession`; it never commits map mutations itself.

use super::{gizmo, object_tool, spline_tool};
use crate::camera::FlyCamera;
use crate::pick::{ObjectPick, Pick};
use eframe::egui;
use std::collections::HashSet;
use web_time::Instant;

/// A box being dragged out with the left button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Marquee {
    pub(super) start: egui::Pos2,
    pub(super) current: egui::Pos2,
    pub(super) additive: bool,
    pub(super) toggle: bool,
}

impl Marquee {
    pub(super) fn rect(&self) -> egui::Rect {
        egui::Rect::from_two_pos(self.start, self.current)
    }
}

pub(super) struct InteractionRuntime {
    pub(super) camera: FlyCamera,
    pub(super) view_plane: u8,
    /// What the cursor is over, and what the viewport context menu was opened on.
    pub(super) hover: Pick,
    pub(super) context_pick: Pick,
    /// The last tile the cursor was over (the Tile tab keeps showing it after the cursor leaves).
    pub(super) last_hover_tile: Option<(i32, i32)>,
    /// The tile the Inspector shows when it is not following the cursor: the last one selected
    /// (or the tile under the last object selected).
    pub(super) inspect_tile: Option<(i32, i32)>,
    /// Every selected object (the last added is `selected`) and every selected tile.
    pub(super) selected: Option<ObjectPick>,
    pub(super) selection_objects: Vec<ObjectPick>,
    pub(super) selection_tiles: HashSet<(i32, i32)>,
    /// Placements to select once their rebuilt region arrives (after an object edit).
    pub(super) pending_reselect: Vec<osrs_world::LocSpec>,
    /// The box being dragged with the Select tool.
    marquee: Option<Marquee>,
    /// The spline tool's control points and preview.
    pub(super) spline: spline_tool::SplineState,
    /// A held tool key: the tool to return to on release, the key and when it went down.
    pub(super) peek: Option<(crate::paint::Tool, egui::Key, Instant)>,
    /// The selected objects being dragged by the Objects tool.
    pub(super) object_drag: Option<object_tool::ObjectDrag>,
    /// The Objects tool's transform gizmo, its drag in progress, and whether it has the pointer.
    pub(super) gizmo: transform_gizmo_egui::Gizmo,
    pub(super) gizmo_drag: Option<gizmo::GizmoDrag>,
    pub(super) gizmo_busy: bool,
}

impl InteractionRuntime {
    pub(super) fn new(camera: FlyCamera) -> Self {
        Self {
            camera,
            view_plane: 0,
            hover: Pick::default(),
            context_pick: Pick::default(),
            last_hover_tile: None,
            inspect_tile: None,
            selected: None,
            selection_objects: Vec::new(),
            selection_tiles: HashSet::new(),
            pending_reselect: Vec::new(),
            marquee: None,
            spline: spline_tool::SplineState::default(),
            peek: None,
            object_drag: None,
            gizmo: transform_gizmo_egui::Gizmo::default(),
            gizmo_drag: None,
            gizmo_busy: false,
        }
    }

    fn same_object(a: &ObjectPick, b: &ObjectPick) -> bool {
        a.region == b.region && a.index == b.index
    }

    pub(super) fn set_hover(&mut self, hover: Pick) {
        if let Some(tile) = hover.tile {
            self.last_hover_tile = Some(tile.tile);
        }
        self.hover = hover;
    }

    pub(super) fn clear_hover(&mut self) {
        self.hover = Pick::default();
    }

    pub(super) fn capture_context_pick(&mut self) {
        self.context_pick = self.hover.clone();
    }

    pub(super) fn is_primary_object(&self, object: &ObjectPick) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|selected| Self::same_object(selected, object))
    }

    pub(super) fn clear_selection(&mut self) {
        self.selected = None;
        self.selection_objects.clear();
        self.selection_tiles.clear();
        self.inspect_tile = None;
    }

    pub(super) fn clear_objects(&mut self) {
        self.selected = None;
        self.selection_objects.clear();
    }

    pub(super) fn clear_tiles(&mut self) {
        self.selection_tiles.clear();
    }

    pub(super) fn select_only_object(
        &mut self,
        object: ObjectPick,
        inspect_tile: Option<(i32, i32)>,
    ) {
        if let Some(tile) = inspect_tile {
            self.inspect_tile = Some(tile);
        }
        self.selection_objects = vec![object.clone()];
        self.selected = Some(object);
    }

    pub(super) fn replace_objects(&mut self, objects: Vec<ObjectPick>) {
        self.selected = objects.last().cloned();
        self.selection_objects = objects;
    }

    pub(super) fn add_object(&mut self, object: ObjectPick, inspect_tile: Option<(i32, i32)>) {
        if let Some(tile) = inspect_tile {
            self.inspect_tile = Some(tile);
        }
        if !self
            .selection_objects
            .iter()
            .any(|selected| Self::same_object(selected, &object))
        {
            self.selection_objects.push(object.clone());
        }
        self.selected = Some(object);
    }

    pub(super) fn toggle_object(&mut self, object: ObjectPick, inspect_tile: Option<(i32, i32)>) {
        if let Some(index) = self
            .selection_objects
            .iter()
            .position(|selected| Self::same_object(selected, &object))
        {
            self.selection_objects.remove(index);
            self.selected = self.selection_objects.last().cloned();
        } else {
            self.add_object(object, inspect_tile);
        }
    }

    pub(super) fn add_tile(&mut self, tile: (i32, i32)) {
        self.selection_tiles.insert(tile);
    }

    pub(super) fn select_tile(&mut self, tile: (i32, i32)) {
        self.selection_tiles.insert(tile);
        self.inspect_tile = Some(tile);
    }

    pub(super) fn toggle_tile(&mut self, tile: (i32, i32)) {
        if !self.selection_tiles.remove(&tile) {
            self.select_tile(tile);
        }
    }

    pub(super) fn selection_counts(&self) -> (usize, usize) {
        (self.selection_objects.len(), self.selection_tiles.len())
    }

    pub(super) fn begin_marquee(&mut self, position: egui::Pos2, additive: bool, toggle: bool) {
        self.marquee = Some(Marquee {
            start: position,
            current: position,
            additive,
            toggle,
        });
    }

    pub(super) fn update_marquee(
        &mut self,
        position: Option<egui::Pos2>,
        bounds: egui::Rect,
        additive: bool,
        toggle: bool,
    ) {
        let Some(marquee) = &mut self.marquee else {
            return;
        };
        if let Some(position) = position {
            marquee.current = position.clamp(bounds.min, bounds.max);
        }
        marquee.additive |= additive;
        marquee.toggle |= toggle;
    }

    pub(super) fn cancel_marquee(&mut self) {
        self.marquee = None;
    }

    pub(super) fn finish_marquee(&mut self) -> Option<Marquee> {
        self.marquee.take()
    }

    pub(super) fn marquee(&self) -> Option<&Marquee> {
        self.marquee.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::InteractionRuntime;
    use crate::camera::FlyCamera;
    use crate::pick::{ObjectPick, Pick, TilePick};
    use eframe::egui::{Pos2, Rect};

    fn runtime() -> InteractionRuntime {
        InteractionRuntime::new(FlyCamera::new(0.0, 0.0, 0.0))
    }

    fn object(region: (i32, i32), index: usize) -> ObjectPick {
        ObjectPick { region, index }
    }

    #[test]
    fn hover_bookkeeping_latches_last_tile_and_clear_preserves_it() {
        let mut interaction = runtime();
        interaction.set_hover(Pick {
            object: Some(object((1, 2), 3)),
            tile: Some(TilePick {
                tile: (10, 11),
                at: (10.5, 11.5),
            }),
        });

        assert_eq!(interaction.last_hover_tile, Some((10, 11)));
        assert_eq!(interaction.hover.tile.map(|pick| pick.tile), Some((10, 11)));

        interaction.clear_hover();

        assert!(interaction.hover.object.is_none());
        assert!(interaction.hover.tile.is_none());
        assert_eq!(interaction.last_hover_tile, Some((10, 11)));
    }

    #[test]
    fn context_pick_captures_hover_snapshot() {
        let mut interaction = runtime();
        interaction.set_hover(Pick {
            object: Some(object((1, 2), 3)),
            tile: Some(TilePick {
                tile: (10, 11),
                at: (10.5, 11.5),
            }),
        });
        interaction.capture_context_pick();
        interaction.set_hover(Pick {
            object: Some(object((4, 5), 6)),
            tile: Some(TilePick {
                tile: (12, 13),
                at: (12.5, 13.5),
            }),
        });

        assert_eq!(
            interaction
                .context_pick
                .object
                .as_ref()
                .map(|object| (object.region, object.index)),
            Some(((1, 2), 3))
        );
        assert_eq!(
            interaction.context_pick.tile.map(|pick| pick.tile),
            Some((10, 11))
        );
    }

    #[test]
    fn object_selection_transitions_keep_last_selected_primary() {
        let mut interaction = runtime();
        let first = object((1, 2), 3);
        let second = object((4, 5), 6);

        interaction.add_object(first.clone(), Some((10, 11)));
        interaction.add_object(first.clone(), Some((10, 11)));
        assert_eq!(interaction.selection_objects.len(), 1);
        assert!(interaction.is_primary_object(&first));

        interaction.add_object(second.clone(), Some((12, 13)));
        assert_eq!(interaction.selection_objects.len(), 2);
        assert!(interaction.is_primary_object(&second));
        assert_eq!(interaction.inspect_tile, Some((12, 13)));

        interaction.toggle_object(second.clone(), Some((99, 99)));
        assert_eq!(interaction.selection_objects.len(), 1);
        assert!(interaction.is_primary_object(&first));
        assert_eq!(interaction.inspect_tile, Some((12, 13)));

        interaction.toggle_object(second.clone(), Some((14, 15)));
        assert_eq!(interaction.selection_objects.len(), 2);
        assert!(interaction.is_primary_object(&second));
        assert_eq!(interaction.inspect_tile, Some((14, 15)));
    }

    #[test]
    fn replacing_objects_tracks_the_last_object_without_changing_inspector() {
        let mut interaction = runtime();
        interaction.inspect_tile = Some((7, 8));
        let first = object((1, 1), 1);
        let second = object((2, 2), 2);

        interaction.replace_objects(vec![first, second.clone()]);

        assert!(interaction.is_primary_object(&second));
        assert_eq!(interaction.inspect_tile, Some((7, 8)));
    }

    #[test]
    fn tile_selection_updates_inspector_only_when_a_tile_is_added_as_focus() {
        let mut interaction = runtime();
        interaction.add_tile((1, 2));
        assert_eq!(interaction.inspect_tile, None);

        interaction.select_tile((3, 4));
        assert_eq!(interaction.inspect_tile, Some((3, 4)));

        interaction.toggle_tile((3, 4));
        assert!(!interaction.selection_tiles.contains(&(3, 4)));
        assert_eq!(interaction.inspect_tile, Some((3, 4)));

        interaction.toggle_tile((5, 6));
        assert!(interaction.selection_tiles.contains(&(5, 6)));
        assert_eq!(interaction.inspect_tile, Some((5, 6)));
    }

    #[test]
    fn clear_selection_clears_all_selection_and_inspector_state() {
        let mut interaction = runtime();
        interaction.add_object(object((1, 2), 3), Some((10, 11)));
        interaction.add_tile((12, 13));

        interaction.clear_selection();

        assert!(interaction.selected.is_none());
        assert!(interaction.selection_objects.is_empty());
        assert!(interaction.selection_tiles.is_empty());
        assert!(interaction.inspect_tile.is_none());
    }

    #[test]
    fn marquee_lifecycle_clamps_pointer_and_latches_modifiers() {
        let mut interaction = runtime();
        let bounds = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0));

        interaction.begin_marquee(Pos2::new(10.0, 20.0), false, false);
        interaction.update_marquee(Some(Pos2::new(150.0, -5.0)), bounds, true, false);
        interaction.update_marquee(None, bounds, false, true);

        let finished = interaction.finish_marquee().map(|marquee| {
            (
                marquee.start,
                marquee.current,
                marquee.additive,
                marquee.toggle,
            )
        });
        assert_eq!(
            finished,
            Some((Pos2::new(10.0, 20.0), Pos2::new(100.0, 0.0), true, true,))
        );
        assert!(interaction.marquee().is_none());
    }

    #[test]
    fn cancel_marquee_discards_in_progress_gesture() {
        let mut interaction = runtime();
        interaction.begin_marquee(Pos2::new(10.0, 20.0), true, false);

        interaction.cancel_marquee();

        assert!(interaction.finish_marquee().is_none());
    }
}
