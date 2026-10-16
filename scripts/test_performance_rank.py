"""Tests for certified performance bottleneck ranking (no editor or GPU required)."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import perf_rank


COMMIT = "a" * 40
TOOLCHAIN = "rustc test"
HARDWARE = {"platform": "macOS", "machine": "arm64", "cpu": "Apple M2",
            "physical_memory_bytes": 8 * 1024**3}
CACHE = "/Users/test/RustOSRS-cache/b241/cache"


def dist(value):
    return {
        "count": 3, "mean": value, "stddev": 0.0, "min": value, "p50": value,
        "p95": value, "p99": value, "p99_9": value, "max": value,
    }


def metadata(workload, profile, vsync, binary):
    return {
        "commit": COMMIT,
        "working_tree_status": "",
        "tracked_diff_sha256": "0" * 64,
        "binary_sha256": binary,
        "toolchain": TOOLCHAIN,
        "hardware": HARDWARE,
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
        "cache_path": CACHE,
        "run_seconds": 30.0,
    }


def report(workload, profile, vsync, binary, scale):
    cpu = 5.0 * scale
    scene = 1.0 * scale
    shadow = 0.5 * scale
    post = 0.25 * scale
    return {
        "schema_version": 2,
        "metadata": metadata(workload, profile, vsync, binary),
        "context": {},
        "warmup_seconds": 8.0,
        "dropped_samples": 0,
        "measurements": {},
        "segments": ["start"],
        "events_ms": {},
        "summary": {
            "frame_interval_ms": dist(10.0 * scale),
            "pacing": None,
            "cpu_ms": dist(cpu),
            "unaccounted_ms": dist(0.1),
            "phases_ms": {"ui": dist(1.5 * scale), "render": dist(3.0 * scale)},
            "gpu_pass_ms": {
                "scene": dist(scene), "shadow": dist(shadow), "post": dist(post),
            },
            "counters": {
                "draw_calls": {"mean": 100.0 * scale},
                "vertices": {"mean": 1000.0 * scale},
                "upload_bytes": {"mean": 200.0 * scale},
                "shadow_redraws": {"mean": 1.0 * scale},
            },
            "events_ms": {
                "build.compute": dist(2.0 * scale),
                "build.queue_wait": dist(0.4 * scale),
                "build.install": dist(0.3 * scale),
                "anim.rebuild": dist(0.2 * scale),
            },
            "process_resident_mib": {"max": 500.0 * scale},
            "scene_encoded_ratio": 1.0,
            "segments": {},
        },
        "external": {"footprint_mib": {"max": 700.0 * scale}},
        "samples": [{
            "seconds": 9.0,
            "segment": 0,
            "camera_hash": 1,
            "scene_encoded": True,
            "cpu_ms": cpu,
            "unaccounted_ms": 0.1,
            "phases_ms": {},
            "counters": {},
            "gpu_pass_samples": {},
            "interval_ms": 10.0 * scale,
        }],
    }


def make_baseline(root: Path, repetitions=2):
    frozen = root / "frozen" / "osrs-editor-test"
    frozen.parent.mkdir(parents=True)
    frozen.write_bytes(b"frozen-binary")
    binary = hashlib.sha256(frozen.read_bytes()).hexdigest()
    for profile, scale in (("Reference", 1.0), ("Hd", 2.0)):
        for sync, vsync in (("uncapped", False), ("capped", True)):
            for index in range(repetitions):
                payload = report("stationary", profile, vsync, binary, scale)
                (root / f"stationary-{profile}-{sync}-{index}.json").write_text(
                    json.dumps(payload)
                )
    manifest = {
        "schema_version": 1,
        "kind": "rustosrs-performance-baseline",
        "certified": True,
        "created_utc": "2026-10-16T00:00:00+00:00",
        "commit": COMMIT,
        "tracked_diff_sha256": "0" * 64,
        "binary_sha256": binary,
        "toolchain": TOOLCHAIN,
        "hardware": HARDWARE,
        "cache_path": CACHE,
        "viewport_reuse": "on",
        "window_logical_points": [1360, 860],
        "base_settings": {"stats_overlay": False, "show_minimap": False, "tile_info_hud": False},
        "reports": repetitions * 4,
        "groups": 4,
        "target_machine_problems": [],
        "frozen_binary": "frozen/osrs-editor-test",
        "profiles": ["Reference", "Hd"],
        "workloads": ["stationary"],
        "sync_modes": ["uncapped", "capped"],
        "repetitions": repetitions,
        "expected_reports": repetitions * 4,
        "summary": "baseline-summary.md",
    }
    (root / "baseline-manifest.json").write_text(json.dumps(manifest))
    return manifest


class RankTests(unittest.TestCase):
    def test_load_recertifies_complete_target_baseline(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = make_baseline(root)
            loaded, groups = perf_rank.load_certified_baseline(root)
        self.assertEqual(loaded["commit"], manifest["commit"])
        self.assertEqual(len(groups), 4)

    def test_loader_rejects_non_target_and_tampered_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = make_baseline(root)
            manifest["target_machine_problems"] = ["CPU is not Apple M2"]
            (root / "baseline-manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "non-target machine"):
                perf_rank.load_certified_baseline(root)
            perf_rank.load_certified_baseline(root, allow_non_target_machine=True)
            (root / manifest["frozen_binary"]).write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "binary hash"):
                perf_rank.load_certified_baseline(root, allow_non_target_machine=True)

    def test_metric_ranking_preserves_units_and_orders_hotter_group_first(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(root)
            _, groups = perf_rank.load_certified_baseline(root)
            ranked = perf_rank.rank_metric(groups, "cpu_p50_ms", 4)
        self.assertEqual(ranked["rows"][0]["profile"], "Hd")
        self.assertEqual(ranked["rows"][-1]["profile"], "Reference")
        self.assertEqual(ranked["available_groups"], 4)

    def test_analysis_reports_profile_deltas_and_nested_hotspots(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = make_baseline(root)
            _, groups = perf_rank.load_certified_baseline(root)
            analysis = perf_rank.analyze(manifest, groups, 5)
        self.assertFalse(analysis["method"]["synthetic_score"])
        self.assertIn("never added", analysis["method"]["memory_accounting"])
        delta = analysis["profile_deltas"][0]["metrics"]["cpu_p50_ms"]["delta_pct"]
        self.assertAlmostEqual(delta, 100.0)
        self.assertEqual(analysis["cpu_phase_hotspots"][0]["name"], "render")
        self.assertEqual(analysis["gpu_pass_hotspots"][0]["name"], "scene")

    def test_markdown_keeps_memory_independent_and_exposes_hd_delta(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = make_baseline(root)
            _, groups = perf_rank.load_certified_baseline(root)
            text = perf_rank.markdown(perf_rank.analyze(manifest, groups, 3))
        self.assertIn("No cross-unit synthetic score", text)
        self.assertIn("HD minus Reference deltas", text)
        self.assertIn("RSS and physical footprint remain independent", text)
        self.assertIn("+100.0%", text)

    def test_write_analysis_emits_json_and_markdown(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = make_baseline(root)
            _, groups = perf_rank.load_certified_baseline(root)
            analysis = perf_rank.analyze(manifest, groups, 2)
            json_path, md_path = perf_rank.write_analysis(root, analysis, "rank")
            payload = json.loads(json_path.read_text())
        self.assertTrue(md_path.name.endswith(".md"))
        self.assertEqual(payload["kind"], "rustosrs-performance-bottleneck-analysis")


if __name__ == "__main__":
    unittest.main()
