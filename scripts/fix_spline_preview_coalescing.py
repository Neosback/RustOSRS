from pathlib import Path

path = Path('crates/osrs-editor/src/app/spline_tool.rs')
text = path.read_text()

old = '''#[derive(Debug, Default)]
pub struct SplineState {
    /// Control points in world tile coordinates.
    pub points: Vec<(f32, f32)>,
    drag: Option<usize>,
    preview: Option<Preview>,
    /// The plan being made on another thread, with the key it is for.
    job: Option<(u64, std::sync::mpsc::Receiver<Preview>)>,
    ctx: Option<egui::Context>,
}
'''
new = '''#[derive(Debug, Default)]
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
'''
assert old in text
text = text.replace(old, new, 1)

old = '''#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    paints: Vec<TilePaint>,
    warnings: Vec<PathWarning>,
    /// The centre line as planned (or the smoothed drawn line when there is no plan).
    lines: Vec<Vec<(f32, f32)>>,
    /// Walls mode: the band's tiles and their tangents.
    wall_tiles: Vec<(i32, i32)>,
    tangents: HashMap<(i32, i32), (f32, f32)>,
}
'''
new = '''#[derive(Debug, Default, Clone)]
struct Preview {
    key: u64,
    paints: Vec<TilePaint>,
    warnings: Vec<PathWarning>,
    /// The centre line as planned (or the smoothed drawn line when there is no plan).
    lines: Vec<Vec<(f32, f32)>>,
    /// Walls mode: the band's tiles and their tangents.
    wall_tiles: Vec<(i32, i32)>,
    tangents: HashMap<(i32, i32), (f32, f32)>,
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
'''
assert old in text
text = text.replace(old, new, 1)

old = '''impl PlanInput {
    fn make(self) -> Preview {
        let points = to_points(&self.points);
        match self.mode {
            PathMode::Terrain => {
                let plan = PathPlanner.plan_stroke(
                    &points,
                    self.plane,
                    &self.profile,
                    &self.settings,
                    &self.env,
                );
                Preview {
                    key: self.key,
                    paints: plan.paints,
                    warnings: plan.warnings,
                    lines: plan.lines.iter().map(|l| to_line(l)).collect(),
                    ..Preview::default()
                }
            }
            PathMode::Walls => {
                let line = terrain_path::smooth_line(&points, self.settings.smoothness, STEP);
                let line = to_line(&line);
                let WallPlan { tiles, tangents } =
                    generate_walls(&line, &self.points, self.wall_thickness, self.tower_size);
                Preview {
                    key: self.key,
                    lines: vec![line],
                    wall_tiles: tiles,
                    tangents,
                    ..Preview::default()
                }
            }
        }
    }
}
'''
new = '''impl PlanInput {
    fn make(self) -> Preview {
        let points = to_points(&self.points);
        match self.mode {
            PathMode::Terrain => {
                let plan = PathPlanner.plan_stroke(
                    &points,
                    self.plane,
                    &self.profile,
                    &self.settings,
                    &self.env,
                );
                Preview {
                    key: self.key,
                    paints: plan.paints,
                    warnings: plan.warnings,
                    lines: plan.lines.iter().map(|l| to_line(l)).collect(),
                    ..Preview::default()
                }
            }
            PathMode::Walls => {
                let line = terrain_path::smooth_line(&points, self.settings.smoothness, STEP);
                let line = to_line(&line);
                let WallPlan { tiles, tangents } =
                    generate_walls(&line, &self.points, self.wall_thickness, self.tower_size);
                Preview {
                    key: self.key,
                    lines: vec![line],
                    wall_tiles: tiles,
                    tangents,
                    ..Preview::default()
                }
            }
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

    fn request(&mut self, input: PlanInput) -> Result<(), PlanInput> {
        if input.key == self.requested_key {
            return Ok(());
        }
        let key = input.key;
        match self.request.send(input) {
            Ok(()) => {
                self.requested_key = key;
                Ok(())
            }
            Err(error) => Err(error.0),
        }
    }
}
'''
assert old in text
text = text.replace(old, new, 1)

old = '''    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline
            .preview
            .as_ref()
            .map_or(0, |p| p.paints.len().max(p.wall_tiles.len()))
    }
'''
new = '''    pub(super) fn spline_tile_count(&self) -> usize {
        self.spline
            .current_preview()
            .map_or(0, |p| p.paints.len().max(p.wall_tiles.len()))
    }
'''
assert old in text
text = text.replace(old, new, 1)

old = '''        let Some(preview) = &self.spline.preview else {
            return Vec::new();
        };
'''
new = '''        let Some(preview) = self.spline.current_preview() else {
            return Vec::new();
        };
'''
assert old in text
text = text.replace(old, new, 1)

start = text.index('    /// Keep the preview in step with the points and options.\n    pub(super) fn spline_update(&mut self) {')
end = text.index('\n    /// Plan now, on this thread, so what is laid is what the points and options say.', start)
old = text[start:end]
new = '''    /// Keep the preview in step with the points and options.
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
                let worker = self.spline.worker.as_mut().expect("worker checked above");
                worker.request(input).err()
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
'''
text = text[:start] + new + text[end:]

old = '''    fn spline_plan_now(&mut self) {
        self.spline.job = None;
        match self.plan_input() {
            Some(input) => {
                if self
                    .spline
                    .preview
                    .as_ref()
                    .is_none_or(|p| p.key != input.key)
                {
                    self.spline.preview = Some(input.make());
                }
            }
            None => self.spline.preview = None,
        }
    }
'''
new = '''    fn spline_plan_now(&mut self) {
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
'''
assert old in text
text = text.replace(old, new, 1)

old = '''        // The planned tiles.
        let overlay_color = self.prefs.colors.get(ColorKey::PathFill);
        let edge = Stroke::new(0.75, self.prefs.colors.get(ColorKey::PathEdge));
        if let Some(preview) = &self.spline.preview {
            let tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8| {
'''
new = '''        // The planned tiles. While a replacement plan is running, retain the last tiles as a
        // faint spatial reference instead of flashing the whole preview off.
        let base_overlay_color = self.prefs.colors.get(ColorKey::PathFill);
        let base_edge_color = self.prefs.colors.get(ColorKey::PathEdge);
        if let Some(preview) = &self.spline.preview {
            let current = self.spline.desired_key == Some(preview.key);
            let fade = |color: Color32| {
                if current {
                    color
                } else {
                    Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        color.a().saturating_div(3),
                    )
                }
            };
            let overlay_color = fade(base_overlay_color);
            let edge = Stroke::new(0.75, fade(base_edge_color));
            let tile_polygon = |tile: (i32, i32), shape: u8, rotation: u8| {
'''
assert old in text
text = text.replace(old, new, 1)

old = '''        let drawn: Vec<(f32, f32)> = match &self.spline.preview {
            Some(preview) if !preview.lines.is_empty() => {
                preview.lines.iter().flatten().copied().collect()
            }
            _ => to_line(&terrain_path::smooth_line(
                &to_points(&self.spline.points),
                self.prefs.spline.planner.smoothness,
                STEP,
            )),
        };
'''
new = '''        let drawn: Vec<(f32, f32)> = match self.spline.current_preview() {
            Some(preview) if !preview.lines.is_empty() => {
                preview.lines.iter().flatten().copied().collect()
            }
            _ => to_line(&terrain_path::smooth_line(
                &to_points(&self.spline.points),
                self.prefs.spline.planner.smoothness,
                STEP,
            )),
        };
'''
assert old in text
text = text.replace(old, new, 1)

# Strict Clippy forbids expect in this repository. Avoid it in the generated patch.
text = text.replace(
    '                let worker = self.spline.worker.as_mut().expect("worker checked above");\n                worker.request(input).err()\n',
    '                self.spline\n                    .worker\n                    .as_mut()\n                    .and_then(|worker| worker.request(input).err())\n',
    1,
)

path.write_text(text)
