//! Camera, picking, selection, and in-progress tool gestures.
//!
//! `InteractionRuntime` owns transient input-side state. It emits edit intents that the
//! application routes to `EditorSession`; it never commits map mutations itself.

mod camera;
mod selection;
pub(super) use selection::EscapeLayer;
mod spline;
pub(super) use spline::{PathExtent, PathOptions, SplineStatus, fit_points};

use super::{gizmo, object_tool};
use crate::camera::FlyCamera;
use crate::paint::Tool;
use crate::pick::{ObjectPick, Pick};
pub(super) use camera::CameraInput;
use eframe::egui;
use std::{collections::HashSet, time::Duration};
use web_time::Instant;

/// A box being dragged out with the left button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Marquee {
    pub(super) start: egui::Pos2,
    pub(super) current: egui::Pos2,
    pub(super) additive: bool,
    pub(super) toggle: bool,
    /// The ground tile under the press and under the pointer now: the box is the rectangle of
    /// tiles between them, in the map's own axes, not a rectangle on the screen.
    pub(super) start_tile: Option<(i32, i32)>,
    pub(super) current_tile: Option<(i32, i32)>,
}

impl Marquee {
    pub(super) fn rect(&self) -> egui::Rect {
        egui::Rect::from_two_pos(self.start, self.current)
    }

    /// The south-west and north-east tile of the box, once both corners are on the ground.
    pub(super) fn tile_box(&self) -> Option<((i32, i32), (i32, i32))> {
        let (a, b) = (self.start_tile?, self.current_tile?);
        Some(((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1))))
    }
}

#[derive(Debug, Clone, Copy)]
struct ToolPeek {
    return_tool: Tool,
    key: egui::Key,
    since: Instant,
}

pub(super) struct InteractionRuntime {
    clipboard: Option<super::tile_clipboard::TileClipboard>,
    camera: FlyCamera,
    view_plane: u8,
    /// What the cursor is over, and what the viewport context menu was opened on.
    hover: Pick,
    context_pick: Pick,
    /// The last tile the cursor was over (the Tile tab keeps showing it after the cursor leaves).
    last_hover_tile: Option<(i32, i32)>,
    /// The tile the Inspector shows when it is not following the cursor: the last one selected
    /// (or the tile under the last object selected).
    inspect_tile: Option<(i32, i32)>,
    /// Every selected object (the last added is `selected`) and every selected tile.
    selected: Option<ObjectPick>,
    selection_objects: Vec<ObjectPick>,
    selection_tiles: HashSet<(i32, i32)>,
    /// Placements to select once their rebuilt region arrives (after an object edit).
    pending_reselect: Vec<osrs_world::LocSpec>,
    /// The box being dragged with the Select tool.
    marquee: Option<Marquee>,
    /// The spline tool's control points and preview.
    spline: spline::SplineState,
    /// A held tool key and the tool to return to if the hold becomes a peek.
    peek: Option<ToolPeek>,
    /// The selected objects being dragged by the Objects tool.
    object_drag: Option<object_tool::ObjectDrag>,
    /// The Objects tool's transform gizmo plus its transient drag/capture lifecycle.
    gizmo: transform_gizmo_egui::Gizmo,
    gizmo_drag: Option<gizmo::GizmoDrag>,
    gizmo_busy: bool,
    /// Smoke runs only: where picking acts as if the pointer were, when it has no real one.
    scripted_pointer: Option<ScriptedPointer>,
    /// The roof the cursor peeled away (kept while the cursor stays over that building).
    peeled_roof: Option<u32>,
}

/// A pointer position a script supplies (smoke runs), so picking runs through the real pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ScriptedPointer {
    /// The middle of the viewport.
    Centre,
    /// A position in points from the viewport's top-left corner.
    At(f32, f32),
}

impl InteractionRuntime {
    pub(super) fn new(camera: FlyCamera) -> Self {
        Self {
            camera,
            clipboard: None,
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
            spline: spline::SplineState::default(),
            peek: None,
            object_drag: None,
            gizmo: transform_gizmo_egui::Gizmo::default(),
            gizmo_drag: None,
            gizmo_busy: false,
            peeled_roof: None,
            scripted_pointer: None,
        }
    }

    pub(super) fn peeled_roof(&self) -> Option<u32> {
        self.peeled_roof
    }

    pub(super) fn set_peeled_roof(&mut self, roof: Option<u32>) {
        self.peeled_roof = roof;
    }

    pub(super) fn scripted_pointer(&self) -> Option<ScriptedPointer> {
        self.scripted_pointer
    }

    pub(super) fn set_scripted_pointer(&mut self, pointer: Option<ScriptedPointer>) {
        self.scripted_pointer = pointer;
    }

    pub(super) fn hover(&self) -> &Pick {
        &self.hover
    }
    pub(super) fn context_pick(&self) -> &Pick {
        &self.context_pick
    }
    pub(super) fn set_context_pick(&mut self, pick: Pick) {
        self.context_pick = pick;
    }
    pub(super) fn last_hover_tile(&self) -> Option<(i32, i32)> {
        self.last_hover_tile
    }
    pub(super) fn inspect_tile(&self) -> Option<(i32, i32)> {
        self.inspect_tile
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn inspect(&mut self, tile: Option<(i32, i32)>) {
        self.inspect_tile = tile;
    }
    pub(super) fn selected(&self) -> &Option<ObjectPick> {
        &self.selected
    }
    pub(super) fn selection_objects(&self) -> &[ObjectPick] {
        &self.selection_objects
    }
    pub(super) fn selection_tiles(&self) -> &HashSet<(i32, i32)> {
        &self.selection_tiles
    }
    pub(super) fn replace_tiles(&mut self, tiles: impl IntoIterator<Item = (i32, i32)>) {
        self.selection_tiles = tiles.into_iter().collect();
    }
    /// Retain the inspector's primary object while returning to a single-object selection.
    pub(super) fn retain_primary_object(&mut self) {
        self.selection_objects.clear();
    }
    pub(super) fn set_primary_object(&mut self, object: Option<ObjectPick>) {
        self.selected = object;
    }
    pub(super) fn view_plane(&self) -> u8 {
        self.view_plane
    }
    pub(super) fn set_view_plane(&mut self, plane: u8) {
        self.switch_plane(plane);
    }
    pub(super) fn gizmo_busy(&self) -> bool {
        self.gizmo_busy
    }

    pub(super) fn gizmo(&self) -> &transform_gizmo_egui::Gizmo {
        &self.gizmo
    }
    pub(super) fn configure_gizmo(&mut self, config: transform_gizmo_egui::GizmoConfig) {
        self.gizmo.update_config(config);
    }
    pub(super) fn sample_gizmo(
        &mut self,
        input: transform_gizmo_egui::GizmoInteraction,
        targets: &[transform_gizmo_egui::math::Transform],
    ) -> Option<(
        transform_gizmo_egui::GizmoResult,
        Vec<transform_gizmo_egui::math::Transform>,
    )> {
        self.gizmo.update(input, targets)
    }

    pub(super) fn clipboard(&self) -> &Option<super::tile_clipboard::TileClipboard> {
        &self.clipboard
    }
    pub(super) fn copy_tiles(&mut self, clipboard: super::tile_clipboard::TileClipboard) {
        self.clipboard = Some(clipboard);
    }

    pub(super) fn press_tool_key(
        &mut self,
        current: Tool,
        pressed: Tool,
        key: Option<egui::Key>,
        now: Instant,
    ) -> Tool {
        if pressed == current && pressed != Tool::Select {
            self.peek = None;
            Tool::Select
        } else {
            self.peek = key.map(|key| ToolPeek {
                return_tool: current,
                key,
                since: now,
            });
            pressed
        }
    }

    pub(super) fn peek_key(&self) -> Option<egui::Key> {
        self.peek.map(|peek| peek.key)
    }

    pub(super) fn release_tool_key(
        &mut self,
        key_down: bool,
        now: Instant,
        peek_after: Duration,
    ) -> Option<Tool> {
        let peek = self.peek?;
        if key_down {
            return None;
        }
        self.peek = None;
        (now.duration_since(peek.since) > peek_after).then_some(peek.return_tool)
    }

    pub(super) fn begin_object_drag(
        &mut self,
        anchor: (i32, i32),
        specs: Vec<osrs_world::LocSpec>,
    ) {
        self.object_drag = Some(object_tool::ObjectDrag {
            anchor,
            specs,
            delta: (0, 0),
        });
    }

    pub(super) fn update_object_drag(&mut self, tile: (i32, i32)) {
        let Some(drag) = &mut self.object_drag else {
            return;
        };
        drag.delta = (tile.0 - drag.anchor.0, tile.1 - drag.anchor.1);
    }

    pub(super) fn cancel_object_drag(&mut self) {
        self.object_drag = None;
    }

    pub(super) fn finish_object_drag(&mut self) -> Option<object_tool::ObjectDrag> {
        self.object_drag.take()
    }

    pub(super) fn object_drag(&self) -> Option<&object_tool::ObjectDrag> {
        self.object_drag.as_ref()
    }

    pub(super) fn update_gizmo_drag(
        &mut self,
        start: transform_gizmo_egui::math::Transform,
        current: transform_gizmo_egui::math::Transform,
        specs: Vec<osrs_world::LocSpec>,
    ) -> gizmo::GizmoDrag {
        let drag = self
            .gizmo_drag
            .get_or_insert_with(|| gizmo::GizmoDrag::new(start, specs));
        drag.update(current);
        drag.clone()
    }

    pub(super) fn cancel_gizmo_drag(&mut self) {
        self.gizmo_drag = None;
        self.gizmo_busy = false;
    }

    pub(super) fn finish_gizmo_drag(&mut self) -> Option<gizmo::GizmoDrag> {
        self.gizmo_busy = false;
        self.gizmo_drag.take()
    }

    pub(super) fn gizmo_drag(&self) -> Option<&gizmo::GizmoDrag> {
        self.gizmo_drag.as_ref()
    }

    pub(super) fn update_gizmo_busy(&mut self, focused: bool) {
        self.gizmo_busy = focused || self.gizmo_drag.is_some();
    }
}

#[cfg(test)]
mod tests;
