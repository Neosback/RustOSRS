//! The viewer: the same world, NPCs and player without the editor's chrome, for the mobile demo
//! page (`index.html?view=mobile`). The page draws the phone-style game frame around the canvas;
//! this owner only prepares the scene once and turns touch gestures into camera moves.
//!
//! Nothing here edits a map or writes settings: the viewer frame returns before the editor's
//! autosave, and the browser build forces its demo player and spawns in memory only.

use super::EditorApp;
use crate::{camera::CameraMode, hud::Destination};
use eframe::egui;
#[cfg(target_arch = "wasm32")]
use {crate::platform::web, std::time::Duration};

/// Where the demo player stands when the page gives no `?at=`: Lumbridge, in front of the castle.
const DEFAULT_STAND: (i32, i32, u8) = (3222, 3218, 0);
/// Camera distance from the player, in client units (about seven tiles).
const DISTANCE: f32 = 900.0;
const PITCH: f32 = 0.75;

#[derive(Default)]
pub(super) struct Viewer {
    enabled: bool,
    prepared: bool,
    #[cfg(target_arch = "wasm32")]
    npcs: Option<web::SharedText>,
}

impl Viewer {
    /// Whether this run is the viewer: `?view=mobile` in the browser, `RUSTOSRS_VIEWER` natively
    /// (to look at the same frame without a browser).
    pub(super) fn from_environment() -> Self {
        #[cfg(target_arch = "wasm32")]
        let enabled = web::viewer_requested();
        #[cfg(not(target_arch = "wasm32"))]
        let enabled = std::env::var_os("RUSTOSRS_VIEWER").is_some();
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }
}

/// Once per frame before the NPC and player owners run: prepare the scene when the world is up,
/// and hand the fetched spawn list to the NPC layer.
pub(super) fn update(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.viewer.enabled {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    if app.npcs.auto == super::npc_runtime::AutoLoad::Idle {
        // The browser cannot read an OpenRune folder; the page names a spawn file instead.
        app.npcs.auto = super::npc_runtime::AutoLoad::Loaded;
    }
    if !app.viewer.prepared && app.world.is_ready() && app.npcs.source().is_some() {
        app.viewer.prepared = true;
        prepare(app, ctx);
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(shared) = &app.viewer.npcs
        && app.npcs.source().is_some()
    {
        let finished = shared.borrow_mut().take();
        if let Some(result) = finished {
            app.viewer.npcs = None;
            match result {
                Ok(text) => super::npc_panel::load_sidecar(app, "the spawn file", &text),
                Err(error) => app.npcs.notice = Some(error),
            }
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn prepare(app: &mut EditorApp, ctx: &egui::Context) {
    #[cfg(target_arch = "wasm32")]
    {
        let (x, y, plane) = web::at_param().unwrap_or(DEFAULT_STAND);
        demo_player(app, x, y, plane);
        let repaint = ctx.clone();
        app.viewer.npcs = web::npcs_url_param()
            .map(|url| web::start_fetch_text(url, move || repaint.request_repaint()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ctx;
    let stand = app
        .settings
        .prefs
        .player
        .stand
        .map_or(DEFAULT_STAND, |stand| (stand.x, stand.y, stand.plane));
    app.teleport_to(Destination::Tile {
        x: stand.0,
        y: stand.1,
        plane: stand.2,
    });
    app.interaction.set_camera_mode(CameraMode::Orbit);
    app.interaction.set_camera_pose(0.0, PITCH, None);
    let camera = app.interaction.camera();
    let target = [camera.x, -80.0, camera.z];
    app.interaction.focus_camera(target, DISTANCE);
}

/// The browser demo's player: a fixed look in rune, standing at the given tile. In memory only.
#[cfg(target_arch = "wasm32")]
fn demo_player(app: &mut EditorApp, x: i32, y: i32, plane: u8) {
    let player = &mut app.settings.prefs.player;
    player.stand = Some(crate::player::Stand { x, y, plane });
    player.show_model = true;
    player.equipment = [
        (0, 1163),  // rune full helm
        (3, 1333),  // rune scimitar
        (4, 1127),  // rune platebody
        (5, 1201),  // rune kiteshield
        (7, 1079),  // rune platelegs
        (10, 4131), // rune boots
    ]
    .into_iter()
    .collect();
    app.settings.prefs.npc.spawns = true;
    app.settings.prefs.npc.models = true;
}

/// One frame of the viewer: the world filling the canvas, orbited by touch or mouse.
pub(super) fn frame(app: &mut EditorApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
            let pixels_per_point = ctx.pixels_per_point() * app.settings.prefs.render_scale;
            let width = ((rect.width() * pixels_per_point) as u32).max(16);
            let height = ((rect.height() * pixels_per_point) as u32).max(16);
            camera_input(app, ctx, &response);
            app.render_viewport_image(ui, rect, width, height);
        });
}

/// One finger (or the mouse) drags the orbit; two fingers pinch to zoom; the wheel zooms too.
fn camera_input(app: &mut EditorApp, ctx: &egui::Context, response: &egui::Response) {
    let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta() - 1.0));
    let delta = response.drag_delta();
    app.interaction
        .apply_camera_input(super::interaction_runtime::CameraInput {
            delta: [delta.x, delta.y],
            secondary_drag: response.dragged_by(egui::PointerButton::Primary),
            scroll,
            pinch,
            hovered: true,
            ..Default::default()
        });
}
