//! Performance measurement, smoke-test hooks and a bounded edit/rebuild trace. Diagnostics
//! observe runtime transitions; enabling or disabling them cannot change editing correctness.
mod capture;
mod trace;
use crate::{hud::FrameStats, screenshot::ScreenshotHook};
use eframe::egui;
use osrs_core::coords::MapTile;
pub(super) use trace::{DiagnosticSink, TraceEvent};
use web_time::Instant;

pub(super) struct DiagnosticsRuntime {
    stats: FrameStats,
    frames: u64,
    peak_resident: usize,
    screenshot: Option<ScreenshotHook>,
    exit_after_seconds: Option<f32>,
    autofly: Option<(f32, f32)>,
    trace: DiagnosticSink,
    capture: Option<capture::PerformanceCapture>,
    gpu_scene_ms: Option<f32>,
    fixed_time_seconds: Option<f32>,
}

pub(super) struct SmokeSnapshot {
    pub seconds: f32,
    pub regions: usize,
    pub vertex_bytes: u64,
    pub zones: usize,
    pub tile: (i32, i32),
    pub region: (i32, i32),
}

impl DiagnosticsRuntime {
    pub(super) fn new(authoring: bool) -> Self {
        let enabled = std::env::var("RUSTOSRS_TRACE_EDITS")
            .map_or(cfg!(debug_assertions), |value| value != "0");
        Self {
            stats: FrameStats::new(),
            frames: 0,
            peak_resident: 0,
            screenshot: authoring.then(ScreenshotHook::from_env).flatten(),
            exit_after_seconds: authoring
                .then(|| {
                    std::env::var("RUSTOSRS_EXIT_AFTER_SECONDS")
                        .ok()
                        .and_then(|value| value.parse().ok())
                })
                .flatten(),
            autofly: authoring
                .then(|| {
                    std::env::var("RUSTOSRS_AUTOFLY").ok().and_then(|value| {
                        let (x, z) = value.split_once(',')?;
                        Some((x.parse().ok()?, z.parse().ok()?))
                    })
                })
                .flatten(),
            trace: if enabled {
                DiagnosticSink::enabled()
            } else {
                DiagnosticSink::default()
            },
            capture: authoring
                .then(capture::PerformanceCapture::from_env)
                .flatten(),
            gpu_scene_ms: None,
            fixed_time_seconds: std::env::var("RUSTOSRS_FIXED_TIME_SECONDS")
                .ok()
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0),
        }
    }
    pub(super) fn sink(&self) -> DiagnosticSink {
        self.trace.clone()
    }
    pub(super) fn stats(&self) -> &FrameStats {
        &self.stats
    }
    pub(super) fn frames(&self) -> u64 {
        self.frames
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn smoke_active(&self) -> bool {
        self.exit_after_seconds.is_some() || self.screenshot.is_some()
    }
    pub(super) fn timed_exit(&self) -> bool {
        self.exit_after_seconds.is_some()
    }
    pub(super) fn expired(&self, seconds: f32) -> bool {
        self.exit_after_seconds
            .is_some_and(|limit| seconds >= limit)
    }
    pub(super) fn autofly(&self) -> Option<(f32, f32)> {
        self.autofly
    }
    pub(super) fn begin_frame(&mut self) -> Instant {
        if let Some(capture) = &mut self.capture {
            capture.begin();
        }
        self.stats.begin_frame()
    }
    pub(super) fn phase(&mut self, name: &'static str, started: Instant) {
        if let Some(capture) = &mut self.capture {
            capture.phase(name, started);
        }
        self.stats.phase(name, started);
    }
    pub(super) fn end_frame(&mut self, started: Instant) {
        self.stats.end_frame(started);
        if let Some(capture) = &mut self.capture {
            capture.end(started, self.gpu_scene_ms, self.stats.queue_latency_ms());
            capture.memory(self.stats.resident_mib());
        }
    }
    pub(super) fn set_gpu_scene_ms(&mut self, ms: Option<f32>) {
        self.gpu_scene_ms = ms;
        self.stats.set_gpu_scene_ms(ms);
    }
    pub(super) fn record_scene_encoding(&mut self, encoded: bool) {
        if let Some(capture) = &mut self.capture {
            capture.scene_encoded(encoded);
        }
    }
    pub(super) fn capture_gpu_samples(
        &mut self,
        make: impl FnOnce() -> std::collections::BTreeMap<&'static str, (u32, f32)>,
    ) {
        if let Some(capture) = &mut self.capture {
            capture.gpu_samples(make());
        }
    }
    pub(super) fn scene_seconds(&self, real: f32) -> f32 {
        self.fixed_time_seconds.unwrap_or(real)
    }
    pub(super) fn build(&mut self, ms: f32) {
        if let Some(capture) = &mut self.capture {
            capture.build(ms);
        }
    }
    pub(super) fn capture_context(&mut self, make: impl FnOnce() -> serde_json::Value) {
        if let Some(capture) = &mut self.capture
            && capture.context_due()
        {
            capture.context(make());
        }
    }
    pub(super) fn observe_residency(&mut self, regions: usize) {
        self.peak_resident = self.peak_resident.max(regions);
    }
    pub(super) fn finish_frame(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        if let Some(hook) = &mut self.screenshot {
            hook.tick(ctx, self.frames);
        }
    }
    pub(super) fn report_smoke(&self, snapshot: SmokeSnapshot) {
        let SmokeSnapshot {
            seconds,
            regions,
            vertex_bytes,
            zones,
            tile,
            region,
        } = snapshot;
        eprintln!(
            "smoke: {} frames in {:.2}s ({:.1} fps average); {} regions resident (peak {}), {:.0} MiB GPU, {} zones\nsmoke: tile {:?} region {:?}\nsmoke: {} | {} | {}",
            self.frames,
            seconds,
            self.frames as f32 / seconds,
            regions,
            self.peak_resident,
            vertex_bytes as f32 / (1024.0 * 1024.0),
            zones,
            tile,
            region,
            self.stats.performance_text(),
            self.stats.phases_text(),
            self.stats.memory_text(vertex_bytes)
        );
    }
    pub(super) fn tile_inspector(&self, ui: &mut egui::Ui, tile: (i32, i32), plane: u8) {
        if !self.trace.is_enabled() {
            return;
        }
        egui::CollapsingHeader::new("Edit and rebuild trace").show(ui, |ui| {
            let lines = self.trace.explain(MapTile::new(tile.0, tile.1), plane);
            if lines.is_empty() {
                ui.weak("No matching events retained in this session.");
            }
            for line in lines {
                ui.small(line);
            }
        });
    }
}
