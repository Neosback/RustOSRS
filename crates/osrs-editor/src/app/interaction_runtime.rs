//! Camera, picking, selection, and in-progress tool gestures.
//!
//! `InteractionRuntime` owns transient input-side state. It emits edit intents that the
//! application routes to `EditorSession`; it never commits map mutations itself.

use super::{gizmo, object_tool, selection, spline_tool};
use crate::camera::FlyCamera;
use crate::pick::{ObjectPick, Pick};
use eframe::egui;
use std::collections::HashSet;
use web_time::Instant;

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
    pub(super) marquee: Option<selection::Marquee>,
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
}
