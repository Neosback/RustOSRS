//! The eframe application: scene loading, the wgpu viewport, and fly-camera input.

mod cache_picker;
mod camera_ops;
mod context_menu;
mod drawer;
mod editing;
mod floating;
mod gizmo;
mod hd;
mod inspector;
mod map_icons;
mod menu;
mod minimap;
pub(crate) mod modal;
mod object_tool;
mod overlay;
#[allow(dead_code)]
mod paint_commit;
mod path_commit;
mod plugins;
mod script;
mod selection;
mod settings_io;
mod settings_window;
mod shelf;
mod spline_tool;
mod status;
mod streaming;
mod tool_strip;
mod toolbar;
mod user_tools;
mod viewport;
mod widgets;
mod workspace;
mod worldmap;

use crate::animation_smoothing::AnimationSmoothingPlugin;
use crate::assets::Assets;
use crate::camera::{CameraMode, FlyCamera};
use crate::grid::GridPlugin;
use crate::hud::{Destination, FrameStats, TeleportWindow};
use crate::interact;
use crate::keymap::Keymap;
use crate::pick::{self, Infos, ObjectPick, Pick};
use crate::plugin::EditorPlugin;
use crate::session::EditorSession;
use crate::settings::ProfileChoice;
use crate::settings::{self, Settings};
use crate::style::ColorKey;
use eframe::{egui, egui_wgpu, wgpu};
use osrs_core::coords::RegionCoord;
use osrs_render::gpu::{COLOR_FORMAT, DEPTH_FORMAT, FrameParams, SceneRenderer};
use osrs_world::{AnimationWorker, RegionStreamer, StreamEvent, TerrainPresentation};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use web_time::Instant;

/// Local units per map region (64 tiles of 128).
const REGION_UNITS: i32 = 64 * 128;
/// Rebase the render origin once the camera is this far (local units) from it.
const REBASE_DISTANCE: f32 = 90_000.0;
/// Regions being built at once.
const MAX_IN_FLIGHT: usize = 2;

struct ViewportTargets {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    texture_id: egui::TextureId,
}

#[derive(Clone, Copy)]
struct RegionStats {
    vertices: usize,
}

/// `(region, loc index)` of a picked object.
type OutlineKey = ((i32, i32), usize);

/// Renderer key of the per-frame animated-loc geometry.
const ANIMATED_KEY: (i32, i32) = (i32::MIN, i32::MIN);
/// Animated locs farther than this many tiles from the camera are not drawn.
const ANIMATION_RADIUS_TILES: i32 = 80;
/// Animated geometry is rebuilt at most once per this many game cycles (20 ms each).
const ANIMATION_STRIDE_CYCLES: u64 = 2;
