//! Performance readout (fps, uncapped estimate, memory) and the teleport window.

use eframe::egui;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

const MIB: f32 = 1024.0 * 1024.0;
/// Smoothing factor of the per-frame timings.
const SMOOTHING: f32 = 0.1;

/// Frame pacing and memory statistics.
///
/// `frame_ms` is the interval between frames, so it includes any wait for the display
/// (vsync). `cpu_ms` is the time spent inside the frame callback and `gpu_ms` the time from
/// queue submission until the GPU reports the work done. The slower of the two bounds the frame
/// rate the application could reach with vsync off, which is the *uncapped* estimate.
pub struct FrameStats {
    gpu_bits: Arc<AtomicU32>,
    last_frame: Option<Instant>,
    frame_ms: f32,
    cpu_ms: f32,
    process_mib: f32,
    peak_process_mib: f32,
    memory_sampled: Option<Instant>,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            gpu_bits: Arc::new(AtomicU32::new(0)),
            last_frame: None,
            frame_ms: 0.0,
            cpu_ms: 0.0,
            process_mib: 0.0,
            peak_process_mib: 0.0,
            memory_sampled: None,
        }
    }

    /// Call first thing in the frame; returns the start time for [`Self::end_frame`].
    pub fn begin_frame(&mut self) -> Instant {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            let ms = now.duration_since(last).as_secs_f32() * 1000.0;
            self.frame_ms = if self.frame_ms == 0.0 {
                ms
            } else {
                self.frame_ms + (ms - self.frame_ms) * SMOOTHING
            };
        }
        self.last_frame = Some(now);
        if self
            .memory_sampled
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
        {
            self.memory_sampled = Some(now);
            if let Some(usage) = memory_stats::memory_stats() {
                self.process_mib = usage.physical_mem as f32 / MIB;
                self.peak_process_mib = self.peak_process_mib.max(self.process_mib);
            }
        }
        now
    }

    pub fn end_frame(&mut self, started: Instant) {
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        self.cpu_ms = if self.cpu_ms == 0.0 {
            ms
        } else {
            self.cpu_ms + (ms - self.cpu_ms) * SMOOTHING
        };
    }

    /// Callback to run when the GPU finishes the work submitted just now.
    pub fn gpu_probe(&self) -> impl FnOnce() + Send + 'static {
        let submitted = Instant::now();
        let sink = self.gpu_bits.clone();
        move || {
            let ms = submitted.elapsed().as_secs_f32() * 1000.0;
            let previous = f32::from_bits(sink.load(Ordering::Relaxed));
            let smoothed = if previous == 0.0 {
                ms
            } else {
                previous + (ms - previous) * SMOOTHING
            };
            sink.store(smoothed.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn fps(&self) -> f32 {
        if self.frame_ms > 0.0 {
            1000.0 / self.frame_ms
        } else {
            0.0
        }
    }

    pub fn gpu_ms(&self) -> f32 {
        f32::from_bits(self.gpu_bits.load(Ordering::Relaxed))
    }

    /// Frame rate the CPU and GPU work would allow without a display-rate cap.
    pub fn uncapped_fps(&self) -> f32 {
        let work = self.cpu_ms.max(self.gpu_ms());
        if work > 0.0 { 1000.0 / work } else { 0.0 }
    }

    /// One-line summary for the status bar.
    pub fn performance_text(&self) -> String {
        format!(
            "{:.0} fps ({:.1} ms) | cpu {:.1} ms, gpu {:.1} ms | uncapped est. ~{:.0} fps",
            self.fps(),
            self.frame_ms,
            self.cpu_ms,
            self.gpu_ms(),
            self.uncapped_fps()
        )
    }

    pub fn memory_text(&self, gpu_geometry_bytes: u64) -> String {
        format!(
            "RAM {:.0} MiB (peak {:.0}) | GPU geometry {:.0} MiB",
            self.process_mib,
            self.peak_process_mib,
            gpu_geometry_bytes as f32 / MIB
        )
    }
}

/// A place to move the camera to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// World tile and plane (`z`, 0-3).
    Tile { x: i32, y: i32, plane: u8 },
    /// Centre of a map region.
    Region { x: i32, y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Tile,
    RegionId,
    RegionXy,
}

/// Floating "Teleport" window: tile `x, y, z`, region id, or region `x, y`.
pub struct TeleportWindow {
    pub open: bool,
    mode: Mode,
    x: String,
    y: String,
    z: String,
    region_id: String,
    error: Option<String>,
}

impl TeleportWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            mode: Mode::Tile,
            x: String::new(),
            y: String::new(),
            z: "0".to_owned(),
            region_id: String::new(),
            error: None,
        }
    }

    /// Prefill the fields with the camera's current position when the window opens.
    pub fn prefill(&mut self, tile: (i32, i32), plane: u8) {
        self.x = tile.0.to_string();
        self.y = tile.1.to_string();
        self.z = plane.to_string();
        let region = (tile.0.div_euclid(64), tile.1.div_euclid(64));
        self.region_id = region_id(region.0, region.1).to_string();
        self.error = None;
    }

    /// Draw the window; returns a destination when the user confirms one.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Destination> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut destination = None;
        egui::Window::new("Teleport")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.mode, Mode::Tile, "Tile x, y, z");
                    ui.selectable_value(&mut self.mode, Mode::RegionId, "Region id");
                    ui.selectable_value(&mut self.mode, Mode::RegionXy, "Region x, y");
                });
                ui.separator();
                let mut submit = false;
                egui::Grid::new("teleport_fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        match self.mode {
                            Mode::Tile => {
                                submit |= field(ui, "x", &mut self.x);
                                submit |= field(ui, "y", &mut self.y);
                                submit |= field(ui, "z (plane)", &mut self.z);
                            }
                            Mode::RegionId => {
                                submit |= field(ui, "region id", &mut self.region_id);
                            }
                            Mode::RegionXy => {
                                submit |= field(ui, "region x", &mut self.x);
                                submit |= field(ui, "region y", &mut self.y);
                            }
                        };
                    });
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                if ui.button("Go").clicked() || submit {
                    match self.parse() {
                        Ok(target) => {
                            self.error = None;
                            destination = Some(target);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            });
        self.open = open;
        destination
    }

    fn parse(&self) -> Result<Destination, String> {
        let number = |text: &str, name: &str| -> Result<i32, String> {
            text.trim()
                .parse::<i32>()
                .map_err(|_| format!("{name} must be a whole number"))
        };
        match self.mode {
            Mode::Tile => {
                let x = number(&self.x, "x")?;
                let y = number(&self.y, "y")?;
                let plane = number(&self.z, "z")?;
                if !(0..=3).contains(&plane) {
                    return Err("z (plane) must be 0-3".to_owned());
                }
                if !(0..=16383).contains(&x) || !(0..=16383).contains(&y) {
                    return Err("x and y must be 0-16383".to_owned());
                }
                Ok(Destination::Tile {
                    x,
                    y,
                    plane: plane as u8,
                })
            }
            Mode::RegionId => {
                let id = number(&self.region_id, "region id")?;
                if !(0..=65535).contains(&id) {
                    return Err("region id must be 0-65535".to_owned());
                }
                Ok(Destination::Region {
                    x: id >> 8,
                    y: id & 0xff,
                })
            }
            Mode::RegionXy => {
                let x = number(&self.x, "region x")?;
                let y = number(&self.y, "region y")?;
                if !(0..=255).contains(&x) || !(0..=255).contains(&y) {
                    return Err("region x and y must be 0-255".to_owned());
                }
                Ok(Destination::Region { x, y })
            }
        }
    }
}

/// Region id as the game defines it: `(x << 8) | y`.
pub fn region_id(x: i32, y: i32) -> i32 {
    (x << 8) | y
}

/// One labelled text field; returns true when Enter was pressed in it.
fn field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.label(label);
    let response = ui.add(egui::TextEdit::singleline(value).desired_width(90.0));
    ui.end_row();
    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_id_matches_the_game_definition() {
        // Lumbridge is region (50, 50) = 12850.
        assert_eq!(region_id(50, 50), 12850);
    }

    #[test]
    fn region_id_destination_splits_into_x_and_y() {
        let mut window = TeleportWindow::new();
        window.mode = Mode::RegionId;
        window.region_id = "12850".to_owned();
        assert_eq!(window.parse(), Ok(Destination::Region { x: 50, y: 50 }));
    }

    #[test]
    fn tile_destination_validates_plane() {
        let mut window = TeleportWindow::new();
        window.x = "3222".to_owned();
        window.y = "3218".to_owned();
        window.z = "2".to_owned();
        assert_eq!(
            window.parse(),
            Ok(Destination::Tile {
                x: 3222,
                y: 3218,
                plane: 2
            })
        );
        window.z = "4".to_owned();
        assert!(window.parse().is_err());
    }
}
