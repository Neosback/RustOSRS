//! Aggregates raw capture samples into distributions. Pure functions over recorded data so the
//! arithmetic can be tested without a window or a GPU.
use super::FrameSample;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// Frames longer than this are counted as visible hitches regardless of the median.
const HITCH_MS: f64 = 33.4;

/// Distribution of finite values: count, mean, spread and the tail percentiles that matter for
/// frame pacing. `None` when there are no finite values.
pub(super) fn distribution(values: impl Iterator<Item = f64>) -> Value {
    let mut values: Vec<_> = values.filter(|value| value.is_finite()).collect();
    values.sort_by(f64::total_cmp);
    let Some(&max) = values.last() else {
        return Value::Null;
    };
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count;
    let percentile = |fraction: f64| {
        let index = ((values.len() - 1) as f64 * fraction).ceil() as usize;
        values[index]
    };
    json!({
        "count": values.len(), "mean": mean, "stddev": variance.sqrt(), "min": values[0],
        "p50": percentile(0.5), "p95": percentile(0.95), "p99": percentile(0.99),
        "p99_9": percentile(0.999), "max": max
    })
}

/// Frame pacing statistics derived from the frame-interval distribution.
fn pacing(intervals: &[f64]) -> Value {
    let mut sorted: Vec<_> = intervals
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return Value::Null;
    }
    let median = sorted[(sorted.len() - 1) / 2];
    let tail = |fraction: f64| sorted[((sorted.len() - 1) as f64 * fraction).ceil() as usize];
    let fps = |interval: f64| {
        if interval > 0.0 {
            1000.0 / interval
        } else {
            0.0
        }
    };
    json!({
        "hitches_over_33_4_ms": sorted.iter().filter(|v| **v > HITCH_MS).count(),
        "hitches_over_2x_median": sorted.iter().filter(|v| **v > 2.0 * median).count(),
        "fps_at_median": fps(median),
        "one_percent_low_fps": fps(tail(0.99)),
        "point_one_percent_low_fps": fps(tail(0.999)),
    })
}

fn named(groups: BTreeMap<String, Vec<f64>>) -> Value {
    Value::Object(
        groups
            .into_iter()
            .map(|(name, values)| (name, distribution(values.into_iter())))
            .collect::<Map<_, _>>(),
    )
}

/// Full summary of a capture. `segments` maps a sample's segment index to its name.
pub(super) fn summarize(
    samples: &[FrameSample],
    segments: &[String],
    events: &BTreeMap<&'static str, Vec<f32>>,
) -> Value {
    let intervals: Vec<f64> = samples.iter().filter_map(|s| s.interval_ms).collect();
    let mut phases: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut counters: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    // GPU results arrive delayed and repeat until the next one lands: count each sequence once.
    let mut passes: BTreeMap<String, BTreeMap<u32, f64>> = BTreeMap::new();
    for sample in samples {
        for (name, ms) in &sample.phases_ms {
            phases.entry((*name).to_owned()).or_default().push(*ms);
        }
        for (name, value) in &sample.counters {
            counters
                .entry((*name).to_owned())
                .or_default()
                .push(*value as f64);
        }
        for (name, (sequence, ms)) in &sample.gpu_pass_samples {
            passes
                .entry((*name).to_owned())
                .or_default()
                .insert(*sequence, f64::from(*ms));
        }
    }
    let gpu_passes = passes
        .into_iter()
        .map(|(name, by_sequence)| (name, by_sequence.into_values().collect()))
        .collect();
    let mut per_segment = Map::new();
    for (index, name) in segments.iter().enumerate() {
        let mine: Vec<_> = samples
            .iter()
            .filter(|s| usize::from(s.segment) == index)
            .collect();
        if mine.is_empty() {
            continue;
        }
        per_segment.insert(
            name.clone(),
            json!({
                "frame_interval_ms": distribution(mine.iter().filter_map(|s| s.interval_ms)),
                "cpu_ms": distribution(mine.iter().map(|s| s.cpu_ms)),
            }),
        );
    }
    let encoded = samples.iter().filter(|s| s.scene_encoded).count();
    json!({
        "frame_interval_ms": distribution(intervals.iter().copied()),
        "pacing": pacing(&intervals),
        "cpu_ms": distribution(samples.iter().map(|s| s.cpu_ms)),
        "unaccounted_ms": distribution(samples.iter().map(|s| s.unaccounted_ms)),
        "phases_ms": named(phases),
        "gpu_pass_ms": named(gpu_passes),
        "counters": named(counters),
        "events_ms": named(events.iter()
            .map(|(name, values)| ((*name).to_owned(), values.iter().map(|v| f64::from(*v)).collect()))
            .collect()),
        "process_resident_mib": distribution(samples.iter().filter_map(|s| s.process_resident_mib.map(f64::from))),
        "scene_encoded_ratio": (!samples.is_empty()).then(|| encoded as f64 / samples.len() as f64),
        "segments": per_segment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(segment: u16, interval: f64, encoded: bool) -> FrameSample {
        FrameSample {
            seconds: 0.0,
            segment,
            camera_hash: 0,
            scene_encoded: encoded,
            interval_ms: Some(interval),
            cpu_ms: 1.0,
            unaccounted_ms: 0.25,
            gpu_pass_samples: BTreeMap::from([("scene", (7, 2.0))]),
            phases_ms: BTreeMap::from([("stream", 0.5)]),
            counters: BTreeMap::from([("draw_calls", 10)]),
            gpu_scene_smoothed_ms: None,
            gpu_submit_to_done_smoothed_ms: None,
            process_resident_mib: Some(100.0),
        }
    }

    #[test]
    fn distributions_report_mean_spread_and_tails() {
        let result = distribution([4.0, 1.0, 3.0, 2.0].into_iter());
        assert_eq!(result["count"], 4);
        assert_eq!(result["mean"], 2.5);
        assert_eq!(result["min"], 1.0);
        assert_eq!(result["p50"], 3.0);
        assert_eq!(result["max"], 4.0);
        assert_eq!(distribution([f64::NAN].into_iter()), Value::Null);
    }

    #[test]
    fn pacing_counts_hitches_and_low_percentile_rates() {
        let mut intervals = vec![10.0; 995];
        intervals.extend([50.0; 5]);
        let result = pacing(&intervals);
        assert_eq!(result["hitches_over_33_4_ms"], 5);
        assert_eq!(result["hitches_over_2x_median"], 5);
        assert_eq!(result["fps_at_median"], 100.0);
        assert_eq!(result["one_percent_low_fps"], 100.0);
        assert_eq!(result["point_one_percent_low_fps"], 20.0);
    }

    #[test]
    fn gpu_pass_results_are_deduplicated_by_sequence_and_segments_are_split() {
        let samples = [
            sample(0, 10.0, true),
            sample(0, 10.0, false),
            sample(1, 20.0, true),
        ];
        let segments = ["warm".to_owned(), "travel".to_owned()];
        let events = BTreeMap::from([("build.compute", vec![4.0_f32, 6.0])]);
        let result = summarize(&samples, &segments, &events);
        assert_eq!(result["gpu_pass_ms"]["scene"]["count"], 1);
        assert_eq!(
            result["segments"]["travel"]["frame_interval_ms"]["p50"],
            20.0
        );
        assert_eq!(result["segments"]["warm"]["frame_interval_ms"]["count"], 2);
        assert_eq!(result["events_ms"]["build.compute"]["mean"], 5.0);
        assert_eq!(result["counters"]["draw_calls"]["max"], 10.0);
        assert!((result["scene_encoded_ratio"].as_f64().unwrap_or(0.0) - 2.0 / 3.0).abs() < 1e-9);
    }
}
