//! Opt-in raw performance capture. Disabled runs allocate no sample storage. GPU timings
//! retain their measurement source; queue latency is never called GPU execution time.
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use web_time::Instant;

const MAX_SAMPLES: usize = 100_000;

#[derive(Serialize)]
struct FrameSample {
    seconds: f64,
    scene_encoded: bool,
    interval_ms: Option<f64>,
    cpu_ms: f64,
    gpu_pass_samples: BTreeMap<&'static str, (u32, f32)>,
    phases_ms: BTreeMap<&'static str, f64>,
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
    samples: Vec<FrameSample>,
    dropped: u64,
    context: Value,
    build_ms: Vec<f32>,
    context_at: Option<Instant>,
    gpu_pass_samples: BTreeMap<&'static str, (u32, f32)>,
    scene_encoded: bool,
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
            samples: Vec::new(),
            dropped: 0,
            context: json!({}),
            build_ms: Vec::new(),
            context_at: None,
            gpu_pass_samples: BTreeMap::new(),
            scene_encoded: false,
        }
    }

    pub(super) fn scene_encoded(&mut self, encoded: bool) {
        self.scene_encoded = encoded;
    }

    pub(super) fn begin(&mut self) {
        self.scene_encoded = false;
        self.phases.clear();
    }

    pub(super) fn phase(&mut self, name: &'static str, started: Instant) {
        *self.phases.entry(name).or_default() += started.elapsed().as_secs_f64() * 1000.0;
    }

    pub(super) fn context(&mut self, context: Value) {
        self.context = context;
        self.context_at = Some(Instant::now());
    }

    pub(super) fn context_due(&self) -> bool {
        self.context_at
            .is_none_or(|at| at.elapsed().as_secs_f32() >= 1.0)
    }

    pub(super) fn build(&mut self, ms: f32) {
        if self.build_ms.len() < MAX_SAMPLES && ms.is_finite() {
            self.build_ms.push(ms);
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
        self.samples.push(FrameSample {
            seconds,
            scene_encoded: self.scene_encoded,
            interval_ms,
            cpu_ms: now.duration_since(started).as_secs_f64() * 1000.0,
            phases_ms: self.phases.clone(),
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
            "schema_version": 1,
            "metadata": std::env::var("RUSTOSRS_PERF_METADATA").ok()
                .and_then(|value| serde_json::from_str::<Value>(&value).ok()),
            "context": self.context,
            "warmup_seconds": self.warmup_seconds,
            "dropped_samples": self.dropped,
            "measurements": {
                "gpu_scene_smoothed_ms": "GPU timestamp queries; scene pass only, delayed and smoothed",
                "gpu_submit_to_done_smoothed_ms": "Queue submission-to-callback latency, including waiting; not GPU execution time",
                "gpu_pass_samples": "Per-pass [sequence, raw milliseconds]; delayed results, deduplicate by sequence within each pass",
                "process_resident_mib": "Latest supported process RSS, sampled at most once per second",
                "allocation_count": null,
                "allocation_count_unavailable_reason": "No allocator profiler attached; unsafe allocator interception is not enabled",
                "gpu_driver_resident_bytes": null,
                "gpu_driver_resident_bytes_unavailable_reason": "Owned resource sizes exclude driver metadata, alignment and shared eframe resources",
                "memory_accounting": "Process RSS and owned GPU bytes may overlap on unified memory; never sum them"
            },
            "summary": {
                "frame_interval_ms": distribution(self.samples.iter().filter_map(|s| s.interval_ms)),
                "cpu_ms": distribution(self.samples.iter().map(|s| s.cpu_ms)),
                "build_ms": distribution(self.build_ms.iter().map(|ms| f64::from(*ms)))
            },
            "build_ms": self.build_ms,
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

fn distribution(values: impl Iterator<Item = f64>) -> Value {
    let mut values: Vec<_> = values.filter(|value| value.is_finite()).collect();
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return Value::Null;
    }
    let percentile = |fraction: f64| {
        let index = ((values.len() - 1) as f64 * fraction).ceil() as usize;
        values[index]
    };
    json!({"count": values.len(), "p50": percentile(0.5), "p95": percentile(0.95),
        "p99": percentile(0.99), "max": values.last()})
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
    fn warmup_frames_are_not_promoted_to_measurement_samples() {
        let mut capture = PerformanceCapture::new(PathBuf::new(), 1e6);
        capture.end(Instant::now(), None, None);
        assert!(capture.samples.is_empty());
        // Drop still verifies error handling without panicking for an invalid output path.
    }

    #[test]
    fn distributions_handle_absent_and_non_finite_samples() {
        assert_eq!(distribution([f64::NAN].into_iter()), Value::Null);
        let result = distribution([4.0, 1.0, 3.0, 2.0].into_iter());
        assert_eq!(result["p50"], 3.0);
        assert_eq!(result["p95"], 4.0);
        assert_eq!(result["count"], 4);
    }
}
