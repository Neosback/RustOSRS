"""Tests for performance report validation, aggregation and comparison (no editor, no GPU)."""
import json
import tempfile
import unittest
from pathlib import Path

import perf_report
import performance


def sample(segment=0, camera=1):
    return {"seconds": 1.0, "segment": segment, "camera_hash": camera, "scene_encoded": True,
            "cpu_ms": 1.0, "unaccounted_ms": 0.1, "phases_ms": {}, "counters": {},
            "gpu_pass_samples": {}}


def report(p50=10.0, scene=2.0, samples=None):
    dist = {"count": 3, "mean": p50, "stddev": 0, "min": p50, "p50": p50, "p95": p50,
            "p99": p50, "p99_9": p50, "max": p50}
    return {
        "schema_version": 2, "metadata": {}, "context": {}, "warmup_seconds": 1, "dropped_samples": 0,
        "measurements": {}, "segments": ["start", "travel"], "events_ms": {},
        "summary": {"frame_interval_ms": dist, "pacing": None, "cpu_ms": dist, "unaccounted_ms": dist,
                    "phases_ms": {}, "gpu_pass_ms": {"scene": {"p50": scene}}, "counters": {},
                    "events_ms": {}, "process_resident_mib": None, "scene_encoded_ratio": 1.0,
                    "segments": {}},
        "samples": [sample()] if samples is None else samples,
    }


class ReportTests(unittest.TestCase):
    def test_valid_report_has_no_errors(self):
        self.assertEqual(perf_report.validate(report()), [])

    def test_old_schema_and_missing_sections_are_rejected(self):
        self.assertTrue(perf_report.validate({"schema_version": 1}))
        broken = report()
        del broken["summary"]["cpu_ms"]
        self.assertIn("missing summary key 'cpu_ms'", perf_report.validate(broken))
        self.assertIn("no post-warmup samples", perf_report.validate(report(samples=[])))
        out_of_range = report(samples=[sample(segment=5)])
        self.assertTrue(any("outside the segment table" in e for e in perf_report.validate(out_of_range)))

    def test_camera_drift_counts_extra_hashes_per_segment(self):
        steady = report(samples=[sample(0, 1), sample(0, 1), sample(1, 2)])
        moving = report(samples=[sample(0, 1), sample(0, 2), sample(0, 3)])
        self.assertEqual(perf_report.camera_drift(steady), 0)
        self.assertEqual(perf_report.camera_drift(moving), 2)

    def test_missing_measurements_stay_none_not_zero(self):
        values = perf_report.metrics(report())
        self.assertIsNone(values["rss_max_mib"])
        self.assertIsNone(values["footprint_max_mib"])
        self.assertEqual(values["gpu_scene_p50_ms"], 2.0)

    def test_compare_reports_deltas_and_flags_regressions_beyond_noise(self):
        before = {("stationary", "Hd"): [report(10.0), report(10.2), report(9.8)]}
        better = {("stationary", "Hd"): [report(8.0), report(8.1), report(7.9)]}
        worse = {("stationary", "Hd"): [report(12.0), report(12.1), report(11.9)]}
        row = lambda rows: next(r for r in rows if r["metric"] == "frame_interval_p50_ms")
        self.assertAlmostEqual(row(perf_report.compare(before, better))["delta_pct"], -20.0, places=1)
        self.assertFalse(row(perf_report.compare(before, better))["regression"])
        self.assertTrue(row(perf_report.compare(before, worse))["regression"])
        unavailable = next(r for r in perf_report.compare(before, worse) if r["metric"] == "rss_max_mib")
        self.assertIsNone(unavailable["delta_pct"])

    def test_load_runs_groups_repetitions_and_skips_sidecar_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for index in range(2):
                (root / f"water-Hd-{index}.json").write_text(json.dumps(report()))
            (root / "water-Hd-0.metadata.json").write_text("{}")
            (root / "water-Hd-0.settings.json").write_text("{}")
            groups = perf_report.load_runs(root)
        self.assertEqual({k: len(v) for k, v in groups.items()}, {("water", "Hd"): 2})


class WorkloadTests(unittest.TestCase):
    def test_every_workload_script_uses_only_known_commands_and_fits_its_duration(self):
        known = {"teleport", "pose", "sleep", "mark", "tool", "overlay", "paint", "undo", "redo",
                 "spline", "lay"}
        for name, spec in performance.WORKLOADS.items():
            sleep = 0.0
            for step in filter(None, (s.strip() for s in spec.get("script", "").split(";"))):
                word = step.split()[0]
                self.assertIn(word, known, f"{name}: {step}")
                if word == "sleep":
                    sleep += float(step.split()[1])
            if "seconds" in spec:
                self.assertGreater(spec["seconds"], sleep, f"{name} ends before its script")


if __name__ == "__main__":
    unittest.main()
