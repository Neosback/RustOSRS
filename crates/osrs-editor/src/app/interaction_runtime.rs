//! Camera, picking, selection, and in-progress tool gestures.
//!
//! `InteractionRuntime` owns transient input-side state. It emits edit intents that the
//! application routes to `EditorSession`; it never commits map mutations itself.

use super::{gizmo, object_tool, spline_tool};
use crate::camera::FlyCamera;
use crate::paint::Tool;
use crate::pick::{ObjectPick, Pick};
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
}

impl Marquee {
    pub(super) fn rect(&self) -> egui::Rect {
        egui::Rect::from_two_pos(self.start, self.current)
    }
}

#[derive(Debug, Clone, Copy)]
struct ToolPeek {
    return_tool: Tool,
    key: egui::Key,
    since: Instant,
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
    /// A held tool key and the tool to return to if the hold becomes a peek.
    peek: Option<ToolPeek>,
    /// The selected objects being dragged by the Objects tool.
    object_drag: Option<object_tool::ObjectDrag>,
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
}

#[cfg(test)]
mod tests;
