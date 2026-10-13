//! The Spline tool: click control points, preview a fitted OSRS overlay band, press Enter.
//!
//! There is deliberately no obstacle routing or wall mode here. The control line is smoothed with
//! deterministic Catmull-Rom sampling and the resulting band is fitted against the exact OSRS
//! shaped-terrain geometry. The fitted tiles are committed as one semantic patch / one undo step.

use super::EditorApp;
use super::editing::PathTile;
use crate::interact;
use crate::paint::Tool;
use crate::style::ColorKey;
use crate::terrain_path::{self, FittedTile};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

/// A click this close (in tiles) to a control point grabs it instead of adding a new one.
const GRAB_DISTANCE: f32 = 0.9;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(f32, f32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
    /// The editor state the preview should represent. The worker may still be catching up.
    desired_key: Option<u64>,
    /// One background worker. Rapid edits are coalesced to the newest request.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<PlanWorker>,
    ctx: Option<egui::Context>,
}

#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    overlay_id: u16,
    paints: Vec<FittedTile>,
    /// The exact smoothed centre line used by the fitter.
    lines: Vec<Vec<(f32, f32)>>,
}

impl SplineState {
    fn current_preview(&self) -> Option<&Preview> {
        self.preview
            .as_ref()
            .filter(|preview| self.desired_key == Some(preview.key))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct PlanWorker {
    request: std::sync::mpsc::Sender<PlanInput>,
    result: std::sync::mpsc::Receiver<Preview>,
    requested_key: u64,
}

fn fnv(text: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(p, (a.0 + dx * t, a.1 + dy * t))
}

/// Everything a fit is made from, copied off the editor so another thread can make it.
struct PlanInput {
    key: u64,
    points: Vec<(f32, f32)>,
    width: f32,
    edge_weight: f32,
    overlay_id: u16,
}

impl PlanInput {
    fn make(self) -> Preview {
        let line = terrain_path::smooth_catmull_rom(&self.points);
        let paints = terrain_path::fit_path_overlay(&line, self.width, self.edge_weight);
        Preview {
            key: self.key,
            overlay_id: self.overlay_id,
            paints,
            lines: vec![line],
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PlanWorker {
    /// A short quiet period prevents expensive plans for every pointer sample while dragging.
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(24);

    fn new(first: PlanInput, repaint: Option<egui::Context>) -> Self {
        let requested_key = first.key;
        let (request, requests) = std::sync::mpsc::channel::<PlanInput>();
        let (results, result) = std::sync::mpsc::channel::<Preview>();
        let _ = request.send(first);
        std::thread::spawn(move || {
            while let Ok(mut input) = requests.recv() {
                // Wait briefly for the drag stream to settle, always replacing the pending input
                // with the newest one. Only one expensive plan is ever running at a time.
                loop {
                    match requests.recv_timeout(Self::DEBOUNCE) {
                        Ok(newer) => input = newer,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                let preview = input.make();
                if results.send(preview).is_err() {
                    return;
                }
                if let Some(ctx) = &repaint {
                    ctx.request_repaint();
                }
            }
        });
        Self {
            request,
            result,
            requested_key,
        }
    }

    fn request(&mut self, input: PlanInput) -> Option<PlanInput> {
        if input.key == self.requested_key {
            return None;
        }
        let key = input.key;
        match self.request.send(input) {
            Ok(()) => {
                self.requested_key = key;
                None
            }
            Err(error) => Some(error.0),
        }
    }
}

impl EditorApp {
    /// Tiles the current path would change.
    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline
            .current_preview()
            .map_or(0, |preview| preview.paints.len())
    }

    /// The overlay id the path lays, when the brush sets one.
    fn laying_overlay(&self) -> Option<u16> {
        let brush = &self.prefs.brush;
        match brush.overlay.fill {
            crate::paint::Fill::Set(id) if brush.overlay.armed => Some(id),
            _ => None,
        }
    }

    /// Everything the current path is planned from, or `None` when there is nothing to plan.
    fn plan_input(&self) -> Option<PlanInput> {
        if self.spline.points.len() < 2 {
            return None;
        }
        let overlay_id = self.laying_overlay()?;
        let spline = &self.prefs.spline;
        let width = spline.width.clamp(0.5, 24.0);
        let edge_weight = spline.edge_weight.clamp(0.0, 2.0);
        let key = fnv(&format!(
            "{:?}|{}|{}|{}|{}|{}",
            self.spline.points,
            width.to_bits(),
            edge_weight.to_bits(),
            overlay_id,
            self.view_plane,
            spline.snap_points,
        ));
        Some(PlanInput {
            key,
            points: self.spline.points.clone(),
            width,
            edge_weight,
            overlay_id,
        })
    }

    /// Keep the preview in step with the points and options.
    pub(super) fn spline_update(&mut self) {
        let Some(input) = self.plan_input() else {
            self.spline.desired_key = None;
            self.spline.preview = None;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.spline.worker = None;
            }
            return;
        };
        let desired_key = input.key;
        self.spline.desired_key = Some(desired_key);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut disconnected = false;
            if let Some(worker) = &mut self.spline.worker {
                loop {
                    match worker.result.try_recv() {
                        Ok(preview) => {
                            // Results for superseded drag states are harmless. Keep the previous
                            // tile preview until the worker catches up, but never promote stale
                            // warnings/counts or a stale planned centre line.
                            if preview.key == desired_key {
                                self.spline.preview = Some(preview);
                            }
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
            }
            if disconnected {
                self.spline.worker = None;
            }

            if self.spline.worker.is_none() {
                self.spline.worker = Some(PlanWorker::new(input, self.spline.ctx.clone()));
                return;
            }

            let retry = {
                self.spline
                    .worker
                    .as_mut()
                    .and_then(|worker| worker.request(input))
            };
            if let Some(input) = retry {
                self.spline.worker = Some(PlanWorker::new(input, self.spline.ctx.clone()));
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if self.spline.current_preview().is_none() {
                self.spline.preview = Some(input.make());
            }
        }
    }

    /// Plan now, on this thread, so what is laid is what the points and options say.
    fn spline_plan_now(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.spline.worker = None;
        }
        match self.plan_input() {
            Some(input) => {
                self.spline.desired_key = Some(input.key);
                if self.spline.current_preview().is_none() {
                    self.spline.preview = Some(input.make());
                }
            }
            None => {
                self.spline.desired_key = None;
                self.spline.preview = None;
            }
        }
    }

    /// Everything about the current fitted path in one copyable block.
    pub(super) fn spline_report(&self) -> String {
        let spline = &self.prefs.spline;
        let raw: Vec<String> = self
            .spline
            .points
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.0, p.1))
            .collect();
        let mut text = format!(
            "PATH plane={} points={}\nwidth={:.2} edge_weight={:.2} overlay={:?}\n",
            self.view_plane,
            raw.join(";"),
            spline.width,
            spline.edge_weight,
            self.prefs.brush.overlay.fill,
        );
        if let Some(preview) = &self.spline.preview {
            text.push_str(&format!("fitted {} tiles\n", preview.paints.len()));
        }
        text
    }

    /// Add, move, insert and remove control points; Enter lays the path.
    pub(super) fn spline_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.spline.ctx.get_or_insert_with(|| ctx.clone());
        if self.prefs.tool != Tool::Spline {
            self.spline.drag = None;
            return;
        }
        self.spline_update();
        if self.quick_open.is_some() || self.context_menu.is_some() {
            return;
        }
        if !self.keys_blocked(ctx) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.commit_spline();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
                self.spline.points.pop();
            }
        }
        let Some(hit) = self.hover.tile else {
            if !ctx.input(|i| i.pointer.primary_down()) {
                self.spline.drag = None;
            }
            return;
        };
        let at = if self.prefs.spline.snap_points {
            (hit.tile.0 as f32 + 0.5, hit.tile.1 as f32 + 0.5)
        } else {
            hit.at
        };
        let modifiers = ctx.input(|i| i.modifiers);
        let nearest = self
            .spline
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (i, distance(*p, at)))
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if !self.keys_blocked(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            match nearest {
                Some(index) => {
                    self.spline.points.remove(index);
                }
                None => {
                    self.spline.points.pop();
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_pressed()) && response.is_pointer_button_down_on() {
            if modifiers.alt {
                return;
            }
            match nearest {
                Some(index) if modifiers.command => {
                    self.spline.points.remove(index);
                }
                Some(index) => self.spline.drag = Some(index),
                None if modifiers.shift && self.spline.points.len() >= 2 => {
                    // Insert on the stretch of the control polygon nearest the click.
                    let points = &self.spline.points;
                    let mut best = (f32::MAX, 0);
                    for i in 0..points.len() - 1 {
                        let d = segment_distance(at, points[i], points[i + 1]);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                    self.spline.points.insert(best.1 + 1, at);
                    self.spline.drag = Some(best.1 + 1);
                }
                None => {
                    self.spline.points.push(at);
                    self.spline.drag = Some(self.spline.points.len() - 1);
                }
            }
        }
        if ctx.input(|i| i.pointer.primary_down()) {
            if let Some(index) = self.spline.drag
                && let Some(point) = self.spline.points.get_mut(index)
            {
                *point = at;
            }
        } else {
            self.spline.drag = None;
        }
    }

    /// Lay the previewed fitted path as one semantic patch / one undo step.
    pub(super) fn commit_spline(&mut self) {
        if self.laying_overlay().is_none() {
            self.notice = Some((
                "Choose an overlay to lay the path with".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        self.spline_plan_now();
        let Some(preview) = self.spline.preview.take() else {
            return;
        };
        if preview.paints.is_empty() {
            self.spline.preview = Some(preview);
            self.notice = Some((
                "The fitted path has no overlay tiles".to_owned(),
                web_time::Instant::now(),
            ));
            return;
        }
        let tiles: Vec<PathTile> = preview
            .paints
            .iter()
            .map(|paint| PathTile {
                tile: (paint.tile_x, paint.tile_y),
                overlay: Some(preview.overlay_id),
                shape: paint.shape,
                rotation: paint.orientation,
            })
            .collect();
        let changed = self.apply_terrain_path(&tiles);
        self.spline.points.clear();
        self.notice = Some((
            format!("Laid a fitted path over {changed} tiles"),
            web_time::Instant::now(),
        ));
    }

    /// The path preview: the planned tiles, the centre line, a ghost segment to the cursor, the
    /// control points.
    pub(super) fn draw_spline(
        &self,
        painter: &egui::Painter,
        viewport: Rect,
        pixels_per_point: f32,
        width: u32,
        height: u32,
    ) {
        if self.prefs.tool != Tool::Spline {
            return;
        }
        let plane = self.view_plane;
        let project = |p: [f64; 3]| self.screen_point(p, viewport, pixels_per_point, width, height);
        // A point on the ground at tile coordinates, lifted a little.
        let ground = |p: (f32, f32), lift: f64| -> Option<Pos2> {
            let tile = (p.0.floor() as i32, p.1.floor() as i32);
            let corners = interact::tile_corners(&self.infos, tile, plane)?;
            // Bilinear height inside the tile.
            let (u, v) = (f64::from(p.0 - p.0.floor()), f64::from(p.1 - p.1.floor()));
            let y = corners[0][1] * (1.0 - u) * (1.0 - v)
                + corners[1][1] * u * (1.0 - v)
                + corners[2][1] * u * v
                + corners[3][1] * (1.0 - u) * v;
            project([f64::from(p.0) * 128.0, y - lift, f64::from(p.1) * 128.0])
        };
        let cursor = self.hover.tile.map(|pick| pick.at);
        let outline = self.prefs.colors.get(ColorKey::PathLine);
        let ghost = self.prefs.colors.get(ColorKey::PathGhost);

        if self.spline.points.is_empty() {
            if let Some(at) = cursor
                && let Some(position) = ground(at, 8.0)
            {
                painter.circle_stroke(position, 9.0, Stroke::new(2.0, ghost));
                painter.text(
                    position + egui::vec2(14.0, -12.0),
                    egui::Align2::LEFT_CENTER,
                    "Click to start the path",
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
            return;
        }

        // The planned tiles. While a replacement plan is running, retain the last tiles as a
        // faint spatial reference instead of flashing the whole preview off.
        let base_overlay_color = self.prefs.colors.get(ColorKey::PathFill);
        let base_edge_color = self.prefs.colors.get(ColorKey::PathEdge);
        if let Some(preview) = &self.spline.preview {
            let current = self.spline.desired_key == Some(preview.key);
            let fade = |color: Color32| {
                if current {
                    color
                } else {
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a() / 3)
                }
            };
            let overlay_color = fade(base_overlay_color);
            let edge = Stroke::new(0.75, fade(base_edge_color));
            let tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8| {
                let Some(corners) = interact::tile_corners(&self.infos, tile, plane) else {
                    return;
                };
                // Map a tile-local (u, v) in 0..1 to the tile's four corners (bilinear).
                let at = |u: f32, v: f32| -> Option<Pos2> {
                    let (u, v) = (f64::from(u), f64::from(v));
                    let lerp = |a: [f64; 3], b: [f64; 3], t: f64| {
                        [
                            a[0] + (b[0] - a[0]) * t,
                            a[1] + (b[1] - a[1]) * t,
                            a[2] + (b[2] - a[2]) * t,
                        ]
                    };
                    let south = lerp(corners[0], corners[1], u);
                    let north = lerp(corners[3], corners[2], u);
                    project(lerp(south, north, v))
                };
                for triangle in terrain_path::overlay_triangles(shape, rotation) {
                    if let (Some(a), Some(b), Some(c)) = (
                        at(triangle[0][0], triangle[0][1]),
                        at(triangle[1][0], triangle[1][1]),
                        at(triangle[2][0], triangle[2][1]),
                    ) {
                        painter.add(egui::Shape::convex_polygon(
                            vec![a, b, c],
                            overlay_color,
                            edge,
                        ));
                    }
                }
            };
            for paint in &preview.paints {
                tile_polygon((paint.tile_x, paint.tile_y), paint.shape, paint.orientation);
            }
        }

        // The centre line.
        let drawn: Vec<(f32, f32)> = match self.spline.current_preview() {
            Some(preview) if !preview.lines.is_empty() => {
                preview.lines.iter().flatten().copied().collect()
            }
            _ => terrain_path::smooth_catmull_rom(&self.spline.points),
        };
        let screen: Vec<Pos2> = drawn.iter().filter_map(|p| ground(*p, 10.0)).collect();
        if screen.len() >= 2 {
            painter.add(egui::Shape::line(
                screen.clone(),
                Stroke::new(4.5, Color32::from_black_alpha(110)),
            ));
            painter.add(egui::Shape::line(screen, Stroke::new(2.5, outline)));
        }
        // A ghost stretch from the last point to the cursor while no point is being dragged.
        if self.spline.drag.is_none()
            && let (Some(last), Some(at)) = (self.spline.points.last(), cursor)
            && let (Some(a), Some(b)) = (ground(*last, 10.0), ground(at, 10.0))
        {
            painter.add(egui::Shape::dashed_line(
                &[a, b],
                Stroke::new(1.5, ghost),
                6.0,
                5.0,
            ));
            painter.circle_stroke(b, 5.0, Stroke::new(1.5, ghost));
        }
        // The control points.
        for (index, point) in self.spline.points.iter().enumerate() {
            if let Some(position) = ground(*point, 12.0) {
                let active = self.spline.drag == Some(index);
                painter.circle_filled(position, if active { 8.0 } else { 6.5 }, Color32::WHITE);
                painter.circle_stroke(position, 7.5, Stroke::new(2.0, outline));
                painter.text(
                    position + egui::vec2(11.0, -11.0),
                    egui::Align2::LEFT_CENTER,
                    (index + 1).to_string(),
                    egui::FontId::proportional(11.5),
                    Color32::WHITE,
                );
            }
        }
    }
}
