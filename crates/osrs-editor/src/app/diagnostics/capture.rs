//! Opt-in raw performance capture (report schema 2). Disabled runs allocate no sample storage.
//! GPU timings retain their measurement source; queue latency is never called GPU execution time.
mod summary;

use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use web_time::Instant;

const SCHEMA_VERSION: u32 = 2;
const MAX_SAMPLES: usize = 100_000;
/// Cap per recorded event stream; later events are counted in `dropped_events`.
const MAX_EVENTS: usize = 20_000;
/// Phases that run inside another phase. They are reported, but not subtracted from the frame
/// twice when computing the unaccounted remainder.
const NESTED_PHASES: &[&str] = &["anim-upload", "minimap"];

#[derive(Serialize)]
struct FrameSample {
    seconds: f64,
    /// Index into the report's `segments` table (see `mark` in the script language).
    segment: u16,
    /// FNV-1a hash of the camera state, to detect drift or input contamination within a run.
    camera_hash: u64,
    scene_encoded: bool,
    interval_ms: Option<f64>,
    cpu_ms: f64,
    /// `cpu_ms` minus the top-level named phases.
    unaccounted_ms: f64,
    gpu_pass_samples: BTreeMap<&'static str, (u32, f32)>,
    phases_ms: BTreeMap<&'static str, f64>,
    /// Renderer counters and sampled gauges for this frame.
    counters: BTreeMap<&'static str, u64>,
    gpu_scene_smoothed_ms: Option<f32>,
    gpu_submit_to_done_smoothed_ms: Option<f32>,
    process_resident_mib: Option<f32>,
}

pub(super) struct PerformanceCapture {
    path: PathBuf,
    started: Instant,
    last_frame: Option<Instant>,
    warmup_seconds: f64,
    phases: BTreeMap<&'static str, f64>,
    counters: BTreeMap<&'static str, u64>,
    samples: Vec<FrameSample>,
    dropped: u64,
    context: Value,
    /// Latency and cost events (worker build stages, installs, rebuilds), in milliseconds.
    events: BTreeMap<&'static str, Vec<f32>>,
    dropped_events: u64,
    context_at: Option<Instant>,
    gpu_pass_samples: BTreeMap<&'static str, (u32, f32)>,
    scene_encoded: bool,
    segments: Vec<String>,
    segment: u16,
    camera_hash: u64,
}

impl PerformanceCapture {
    pub(super) fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("RUSTOSRS_PERF_REPORT")?);
        let warmup_seconds = std::env::var("RUSTOSRS_PERF_WARMUP_SECONDS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(5.0);
        Some(Self::new(path, warmup_seconds))
    }

    fn new(path: PathBuf, warmup_seconds: f64) -> Self {
        Self {
            path,
            started: Instant::now(),
            last_frame: None,
            warmup_seconds,
            phases: BTreeMap::new(),
            counters: BTreeMap::new(),
            samples: Vec::new(),
            dropped: 0,
            context: json!({}),
            events: BTreeMap::new(),
            dropped_events: 0,
            context_at: None,
            gpu_pass_samples: BTreeMap::new(),
            scene_encoded: false,
            segments: vec!["start".to_owned()],
            segment: 0,
            camera_hash: 0,
        }
    }

    pub(super) fn scene_encoded(&mut self, encoded: bool) {
        self.scene_encoded = encoded;
    }

    pub(super) fn begin(&mut self) {
        self.scene_encoded = false;
        self.phases.clear();
        self.counters.clear();
    }

    pub(super) fn phase(&mut self, name: &'static str, started: Instant) {
        *self.phases.entry(name).or_default() += started.elapsed().as_secs_f64() * 1000.0;
    }

    pub(super) fn counter(&mut self, name: &'static str, value: u64) {
        self.counters.insert(name, value);
    }

    /// Start a named workload segment; later samples are attributed to it.
    pub(super) fn mark_segment(&mut self, name: &str) {
        let index = self
            .segments
            .iter()
            .position(|existing| existing == name)
            .unwrap_or_else(|| {
                self.segments.push(name.to_owned());
                self.segments.len() - 1
            });
        self.segment = u16::try_from(index).unwrap_or(u16::MAX);
    }

    pub(super) fn camera(&mut self, words: &[u32]) {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in words.iter().flat_map(|word| word.to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        self.camera_hash = hash;
    }

    pub(super) fn context(&mut self, context: Value) {
        self.context = context;
        self.context_at = Some(Instant::now());
    }

    pub(super) fn context_due(&self) -> bool {
        self.context_at
            .is_none_or(|at| at.elapsed().as_secs_f32() >= 1.0)
    }

    pub(super) fn event(&mut self, name: &'static str, ms: f32) {
        if !ms.is_finite() || self.started.elapsed().as_secs_f64() < self.warmup_seconds {
            return;
        }
        let stream = self.events.entry(name).or_default();
        if stream.len() < MAX_EVENTS {
            stream.push(ms);
        } else {
            self.dropped_events += 1;
        }
    }

    pub(super) fn end(&mut self, started: Instant, gpu: Option<f32>, queue_latency: Option<f32>) {
        let now = Instant::now();
        let interval_ms = self
            .last_frame
            .map(|last| (started - last).as_secs_f64() * 1000.0);
        self.last_frame = Some(started);
        let seconds = now.duration_since(self.started).as_secs_f64();
        if seconds < self.warmup_seconds {
            return;
        }
        if self.samples.len() == MAX_SAMPLES {
            self.dropped += 1;
            return;
        }
        let cpu_ms = now.duration_since(started).as_secs_f64() * 1000.0;
        let top_level: f64 = self
            .phases
            .iter()
            .filter(|(name, _)| !NESTED_PHASES.contains(name))
            .map(|(_, ms)| ms)
            .sum();
        self.samples.push(FrameSample {
            seconds,
            segment: self.segment,
            camera_hash: self.camera_hash,
            scene_encoded: self.scene_encoded,
            interval_ms,
            cpu_ms,
            unaccounted_ms: (cpu_ms - top_level).max(0.0),
            phases_ms: self.phases.clone(),
            counters: self.counters.clone(),
            gpu_pass_samples: self.gpu_pass_samples.clone(),
            gpu_scene_smoothed_ms: gpu,
            gpu_submit_to_done_smoothed_ms: queue_latency,
            // Sampling every frame would make measurement a dominant CPU cost.
            process_resident_mib: None,
        });
    }

    pub(super) fn gpu_samples(&mut self, samples: BTreeMap<&'static str, (u32, f32)>) {
        self.gpu_pass_samples = samples;
    }

    pub(super) fn memory(&mut self, mib: Option<f32>) {
        if let Some(sample) = self.samples.last_mut() {
            sample.process_resident_mib = mib;
        }
    }

    fn report(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "metadata": std::env::var("RUSTOSRS_PERF_METADATA").ok()
                .and_then(|value| serde_json::from_str::<Value>(&value).ok()),
            "context": self.context,
            "warmup_seconds": self.warmup_seconds,
            "dropped_samples": self.dropped,
            "dropped_events": self.dropped_events,
            "measurements": {
                "interval_ms": "Time between frame starts. On a presented window it includes waiting for the drawable and the compositor; present wait is not separated from useful work",
                "cpu_ms": "Body of EditorApp::ui only; excludes egui tessellation, queue submission by eframe and present",
                "unaccounted_ms": "cpu_ms minus top-level named phases; nested phases (anim-upload, minimap) are not subtracted twice",
                "gpu_scene_smoothed_ms": "GPU timestamp queries; scene pass only, delayed and smoothed",
                "gpu_submit_to_done_smoothed_ms": "Queue submission-to-callback latency, including waiting; not GPU execution time",
                "gpu_pass_samples": "Per-pass [sequence, raw milliseconds]; delayed results, deduplicated by sequence in the summary",
                "process_resident_mib": "Latest supported process RSS, sampled at most once per second; not the macOS physical footprint",
                "counters": "Per-frame renderer counters (draw calls, vertices, upload bytes since the previous sample, shadow redraws, visible zones) and worker gauges",
                "events_ms": "Worker and install latencies: build.queue_wait/decode/scene/extract/animations/info/compute, build.install, anim.rebuild, outline.latency",
                "allocation_count": null,
                "allocation_count_unavailable_reason": "No allocator profiler attached; unsafe allocator interception is not enabled (use the alloc-profile feature in a dedicated run)",
                "gpu_driver_resident_bytes": null,
                "gpu_driver_resident_bytes_unavailable_reason": "Owned resource sizes exclude driver metadata, alignment and shared eframe resources",
                "memory_accounting": "Process RSS and owned GPU bytes may overlap on unified memory; never sum them"
            },
            "segments": self.segments,
            "summary": summary::summarize(&self.samples, &self.segments, &self.events),
            "events_ms": self.events,
            "samples": self.samples
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn write(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(&self.path)?;
        use std::io::Write;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, &self.report())?;
        writer.flush()?;
        Ok(())
    }
}

impl Drop for PerformanceCapture {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        eprintln!(
            "performance report {}: {}",
            self.path.display(),
            self.report()
        );
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(error) = self.write() {
            eprintln!("performance report {}: {error}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_reset_phases_and_keep_absent_gpu_measurements_absent()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut capture = PerformanceCapture::new(PathBuf::new(), 0.0);
        capture.phase("upload", Instant::now());
        capture.end(Instant::now(), None, None);
        capture.begin();
        capture.end(Instant::now(), Some(2.0), Some(8.0));
        assert!(capture.samples[1].phases_ms.is_empty());
        assert_eq!(capture.samples[0].gpu_scene_smoothed_ms, None);
        assert_eq!(capture.samples[1].gpu_submit_to_done_smoothed_ms, Some(8.0));
        assert!(capture.report()["measurements"]["allocation_count"].is_null());
        assert_eq!(capture.report()["schema_version"], SCHEMA_VERSION);
        // This in-memory test never publishes a file when the capture is dropped.
        capture.path =
            std::env::temp_dir().join(format!("rustosrs-capture-test-{}.json", std::process::id()));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = capture.path.clone();
            drop(capture);
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    #[test]
    fn warmup_frames_and_events_are_not_promoted_to_measurement_samples() {
        let mut capture = PerformanceCapture::new(PathBuf::new(), 1e6);
        capture.end(Instant::now(), None, None);
        capture.event("build.compute", 3.0);
        assert!(capture.samples.is_empty());
        assert!(capture.events.is_empty());
    }

    #[test]
    fn nested_phases_are_not_subtracted_twice_from_the_unaccounted_remainder() {
        let mut capture = PerformanceCapture::new(PathBuf::new(), 0.0);
        let started = Instant::now();
        capture.phases.insert("anim", 1.0e6);
        capture.phases.insert("anim-upload", 5.0e5);
        capture.end(started, None, None);
        let sample = &capture.samples[0];
        // The injected top-level phase dwarfs any real frame time, so the remainder clamps to zero.
        assert_eq!(sample.unaccounted_ms, 0.0);
        assert_eq!(sample.phases_ms["anim-upload"], 5.0e5);
    }

    #[test]
    fn segments_attribute_samples_and_camera_hash_tracks_state() {
        let mut capture = PerformanceCapture::new(PathBuf::new(), 0.0);
        capture.end(Instant::now(), None, None);
        capture.mark_segment("travel");
        capture.mark_segment("return");
        capture.mark_segment("travel");
        capture.camera(&[1, 2, 3]);
        capture.end(Instant::now(), None, None);
        let first = capture.samples[1].camera_hash;
        capture.camera(&[1, 2, 4]);
        capture.end(Instant::now(), None, None);
        assert_eq!(capture.samples[0].segment, 0);
        assert_eq!(capture.samples[1].segment, 1);
        assert_ne!(first, capture.samples[2].camera_hash);
        assert_eq!(capture.segments, ["start", "travel", "return"]);
    }
}
