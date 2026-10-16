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


def metadata(workload="stationary", profile="Hd", vsync=False, *, commit="a" * 40,
             binary="b" * 64, hardware=None, run_seconds=30.0):
    return {
        "commit": commit,
        "working_tree_status": "",
        "tracked_diff_sha256": "0" * 64,
        "binary_sha256": binary,
        "toolchain": "rustc 1.90.0",
        "hardware": hardware or {"platform": "macOS", "machine": "arm64"},
        "machine_state": {"thermal": "nominal"},
        "workload": workload,
        "profile": profile,
        "vsync": vsync,
        "window_logical_points": [1360, 860],
        "base_settings": {"stats_overlay": False, "show_minimap": False, "tile_info_hud": False},
        "workload_settings": {},
        "workload_script": "",
        "fixed_time_seconds": None,
        "viewport_reuse": "on",
        "cache_path": "/cache/b241/cache",
        "run_seconds": run_seconds,
    }


def report(p50=10.0, scene=2.0, samples=None, meta=None):
    dist = {"count": 3, "mean": p50, "stddev": 0, "min": p50, "p50": p50, "p95": p50,
            "p99": p50, "p99_9": p50, "max": p50}
    return {
        "schema_version": 2, "metadata": meta or metadata(), "context": {}, "warmup_seconds": 1,
        "dropped_samples": 0, "measurements": {}, "segments": ["start", "travel"],
        "events_ms": {},
        "summary": {"frame_interval_ms": dist, "pacing": None, "cpu_ms": dist,
                    "unaccounted_ms": dist, "phases_ms": {},
                    "gpu_pass_ms": {"scene": {"p50": scene}}, "counters": {},
                    "events_ms": {}, "process_resident_mib": None,
                    "scene_encoded_ratio": 1.0, "segments": {}},
        "samples": [sample()] if samples is None else samples,
    }


class ReportTests(unittest.TestCase):
    def test_valid_report_has_no_errors(self):
        self.assertEqual(perf_report.validate(report()), [])

    def test_old_schema_missing_sections_and_provenance_are_rejected(self):
        self.assertTrue(perf_report.validate({"schema_version": 1}))
        broken = report()
        del broken["summary"]["cpu_ms"]
        self.assertIn("missing summary key 'cpu_ms'", perf_report.validate(broken))
        self.assertIn("no post-warmup samples", perf_report.validate(report(samples=[])))
        missing_meta = report()
        del missing_meta["metadata"]["binary_sha256"]
        self.assertIn("missing metadata key 'binary_sha256'", perf_report.validate(missing_meta))

    def test_every_sample_is_validated(self):
        broken = sample()
        del broken["camera_hash"]
        errors = perf_report.validate(report(samples=[sample(), broken]))
        self.assertIn("sample 1 missing 'camera_hash'", errors)
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

    def test_compare_allows_source_change_but_rejects_environment_change(self):
        key = ("stationary", "Hd", "uncapped")
        before = {key: [report(meta=metadata(commit="1" * 40, binary="1" * 64))]}
        after = {key: [report(8.0, meta=metadata(commit="2" * 40, binary="2" * 64))]}
        row = next(r for r in perf_report.compare(before, after)
                   if r["metric"] == "frame_interval_p50_ms")
        self.assertAlmostEqual(row["delta_pct"], -20.0)
        incompatible = {key: [report(meta=metadata(
            commit="2" * 40, binary="2" * 64,
            hardware={"platform": "other", "machine": "arm64"}))]}
        with self.assertRaisesRegex(ValueError, "hardware"):
            perf_report.compare(before, incompatible)

    def test_compare_flags_regressions_beyond_noise(self):
        key = ("stationary", "Hd", "uncapped")
        before = {key: [report(10.0), report(10.2), report(9.8)]}
        better = {key: [report(8.0), report(8.1), report(7.9)]}
        worse = {key: [report(12.0), report(12.1), report(11.9)]}
        row = lambda rows: next(r for r in rows if r["metric"] == "frame_interval_p50_ms")
        self.assertAlmostEqual(row(perf_report.compare(before, better))["delta_pct"], -20.0, places=1)
        self.assertFalse(row(perf_report.compare(before, better))["regression"])
        self.assertTrue(row(perf_report.compare(before, worse))["regression"])
        unavailable = next(r for r in perf_report.compare(before, worse) if r["metric"] == "rss_max_mib")
        self.assertIsNone(unavailable["delta_pct"])

    def test_load_runs_keeps_capped_and_uncapped_separate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for vsync, sync in ((False, "uncapped"), (True, "capped")):
                for index in range(2):
                    payload = report(meta=metadata(workload="water", profile="Hd", vsync=vsync))
                    (root / f"water-Hd-{sync}-{index}.json").write_text(json.dumps(payload))
            (root / "water-Hd-capped-0.metadata.json").write_text("{}")
            (root / "water-Hd-capped-0.settings.json").write_text("{}")
            groups = perf_report.load_runs(root)
        self.assertEqual({k: len(v) for k, v in groups.items()}, {
            ("water", "Hd", "capped"): 2,
            ("water", "Hd", "uncapped"): 2,
        })

    def test_load_runs_rejects_filename_metadata_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            payload = report(meta=metadata(workload="water", profile="Hd", vsync=False))
            (root / "water-Reference-uncapped-0.json").write_text(json.dumps(payload))
            with self.assertRaisesRegex(ValueError, "embedded metadata"):
                perf_report.load_runs(root)

    def test_load_runs_rejects_mixed_repetition_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first = report(meta=metadata(workload="water", binary="1" * 64))
            second = report(meta=metadata(workload="water", binary="2" * 64))
            (root / "water-Hd-uncapped-0.json").write_text(json.dumps(first))
            (root / "water-Hd-uncapped-1.json").write_text(json.dumps(second))
            with self.assertRaisesRegex(ValueError, "binary_sha256"):
                perf_report.load_runs(root)


class StallTests(unittest.TestCase):
    def test_frame_gaps_over_a_second_are_reported_as_stalls(self):
        clean = report(samples=[dict(sample(), interval_ms=16.0)])
        stalled = report(samples=[dict(sample(), interval_ms=16.0), dict(sample(), interval_ms=5200.0)])
        self.assertEqual(perf_report.stalls(clean), [])
        self.assertEqual(perf_report.stalls(stalled), [5200.0])
        self.assertEqual(perf_report.stalls(report(samples=[sample()])), [])


class SummaryTests(unittest.TestCase):
    def test_markdown_summary_shows_sync_median_spread_and_unavailable(self):
        groups = {("water", "Hd", "capped"): [report(10.0), report(12.0), report(11.0)]}
        table = perf_report.markdown_summary(groups)
        self.assertIn("| water | Hd | capped | 3 | 11.00 (10.00-12.00) |", table)
        self.assertIn("n/a", table)


class WorkloadTests(unittest.TestCase):
    def test_run_names_keep_sync_modes_distinct(self):
        self.assertEqual(performance.run_name("water", "Hd", False, 0), "water-Hd-uncapped-0")
        self.assertEqual(performance.run_name("water", "Hd", True, 0), "water-Hd-capped-0")

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
