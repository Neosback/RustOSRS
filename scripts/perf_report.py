"""Validation and aggregation of editor performance reports (schema 2).

Shared by `performance.py` (runs workloads) and `performance_compare.py` (before/after tables).
Nothing here imposes a timing threshold: deltas are reported with their run-to-run spread so a
reader can judge noise. Missing measurements stay `None`, never zero.
"""
from __future__ import annotations

import json
import statistics
from pathlib import Path
from typing import Any, Iterable

SCHEMA_VERSION = 2
REQUIRED_TOP_LEVEL = (
    "schema_version", "metadata", "context", "warmup_seconds", "dropped_samples",
    "measurements", "segments", "summary", "events_ms", "samples",
)
REQUIRED_SUMMARY = (
    "frame_interval_ms", "pacing", "cpu_ms", "unaccounted_ms", "phases_ms", "gpu_pass_ms",
    "counters", "events_ms", "process_resident_mib", "scene_encoded_ratio", "segments",
)
REQUIRED_SAMPLE = (
    "seconds", "segment", "camera_hash", "scene_encoded", "cpu_ms", "unaccounted_ms",
    "phases_ms", "counters", "gpu_pass_samples",
)

# name -> (path into the report, lower_is_better)
KEY_METRICS: dict[str, tuple[tuple[str, ...], bool]] = {
    "frame_interval_p50_ms": (("summary", "frame_interval_ms", "p50"), True),
    "frame_interval_p95_ms": (("summary", "frame_interval_ms", "p95"), True),
    "frame_interval_p99_ms": (("summary", "frame_interval_ms", "p99"), True),
    "cpu_p50_ms": (("summary", "cpu_ms", "p50"), True),
    "cpu_p95_ms": (("summary", "cpu_ms", "p95"), True),
    "gpu_scene_p50_ms": (("summary", "gpu_pass_ms", "scene", "p50"), True),
    "gpu_shadow_p50_ms": (("summary", "gpu_pass_ms", "shadow", "p50"), True),
    "gpu_post_p50_ms": (("summary", "gpu_pass_ms", "post", "p50"), True),
    "scene_encoded_ratio": (("summary", "scene_encoded_ratio"), True),
    "draw_calls_mean": (("summary", "counters", "draw_calls", "mean"), True),
    "vertices_mean": (("summary", "counters", "vertices", "mean"), True),
    "upload_bytes_mean": (("summary", "counters", "upload_bytes", "mean"), True),
    "shadow_redraws_mean": (("summary", "counters", "shadow_redraws", "mean"), True),
    "build_compute_p50_ms": (("summary", "events_ms", "build.compute", "p50"), True),
    "build_queue_wait_p95_ms": (("summary", "events_ms", "build.queue_wait", "p95"), True),
    "build_install_p95_ms": (("summary", "events_ms", "build.install", "p95"), True),
    "anim_rebuild_p50_ms": (("summary", "events_ms", "anim.rebuild", "p50"), True),
    "rss_max_mib": (("summary", "process_resident_mib", "max"), True),
    "footprint_max_mib": (("external", "footprint_mib", "max"), True),
}


def dig(value: Any, path: Iterable[str]) -> Any:
    for key in path:
        if not isinstance(value, dict) or key not in value:
            return None
        value = value[key]
    return value


def validate(report: dict) -> list[str]:
    """Return a list of schema problems; empty means the report is usable."""
    errors: list[str] = []
    if report.get("schema_version") != SCHEMA_VERSION:
        return [f"schema_version {report.get('schema_version')!r} != {SCHEMA_VERSION}"]
    errors += [f"missing top-level key {k!r}" for k in REQUIRED_TOP_LEVEL if k not in report]
    summary = report.get("summary")
    if isinstance(summary, dict):
        errors += [f"missing summary key {k!r}" for k in REQUIRED_SUMMARY if k not in summary]
    samples = report.get("samples")
    if not samples:
        errors.append("no post-warmup samples")
    else:
        first = samples[0]
        errors += [f"sample missing {k!r}" for k in REQUIRED_SAMPLE if k not in first]
    segments = report.get("segments")
    if samples and isinstance(segments, list):
        used = {s.get("segment") for s in samples}
        errors += [f"sample segment {i} outside the segment table" for i in used
                   if not isinstance(i, int) or i >= len(segments)]
    return errors


STALL_MS = 1000.0


def stalls(report: dict) -> list[float]:
    """Frame intervals above one second. They mean the window or process was throttled or the
    machine slept, not that the editor is slow: such a run is contaminated and must be repeated."""
    return [s["interval_ms"] for s in report.get("samples", [])
            if (s.get("interval_ms") or 0.0) > STALL_MS]


def camera_drift(report: dict) -> int:
    """Number of distinct camera hashes beyond one per segment: nonzero in a stationary workload
    means the camera moved (input contamination)."""
    per_segment: dict[int, set] = {}
    for sample in report.get("samples", []):
        per_segment.setdefault(sample["segment"], set()).add(sample["camera_hash"])
    return sum(len(hashes) - 1 for hashes in per_segment.values())


def metrics(report: dict) -> dict[str, float | None]:
    result: dict[str, float | None] = {}
    for name, (path, _) in KEY_METRICS.items():
        value = dig(report, path)
        result[name] = float(value) if isinstance(value, (int, float)) else None
    return result


def load_runs(directory: Path) -> dict[tuple[str, str], list[dict]]:
    """Group `<workload>-<profile>-<repetition>.json` reports by (workload, profile)."""
    groups: dict[tuple[str, str], list[dict]] = {}
    for path in sorted(directory.glob("*.json")):
        if path.name.endswith((".metadata.json", ".settings.json", ".footprint.json")):
            continue
        parts = path.stem.rsplit("-", 2)
        if len(parts) != 3 or not parts[2].isdigit():
            continue
        report = json.loads(path.read_text())
        if report.get("schema_version") != SCHEMA_VERSION:
            continue
        groups.setdefault((parts[0], parts[1]), []).append(report)
    return groups


def aggregate(reports: list[dict]) -> dict[str, dict[str, float | None]]:
    """Median across repetitions per metric, with min/max spread; absent stays None."""
    out: dict[str, dict[str, float | None]] = {}
    per_run = [metrics(r) for r in reports]
    for name in KEY_METRICS:
        values = [m[name] for m in per_run if m[name] is not None]
        out[name] = {
            "median": statistics.median(values) if values else None,
            "min": min(values) if values else None,
            "max": max(values) if values else None,
            "runs": len(values),
        }
    return out


def compare(before: dict, after: dict) -> list[dict]:
    """Rows of per-metric deltas for every (workload, profile) present in both directories.

    `regression` is informational (worse than the run-to-run spread of `before`); it is not a gate.
    """
    rows = []
    for key in sorted(set(before) & set(after)):
        b, a = aggregate(before[key]), aggregate(after[key])
        for name, (_, lower_is_better) in KEY_METRICS.items():
            bm, am = b[name]["median"], a[name]["median"]
            if bm is None or am is None:
                rows.append({"workload": key[0], "profile": key[1], "metric": name,
                             "before": bm, "after": am, "delta_pct": None, "regression": None,
                             "note": "unavailable in one side"})
                continue
            delta = (am - bm) / bm * 100.0 if bm else None
            spread = (b[name]["max"] - b[name]["min"]) / bm * 100.0 if bm else 0.0
            worse = (am > bm) if lower_is_better else (am < bm)
            rows.append({"workload": key[0], "profile": key[1], "metric": name,
                         "before": bm, "after": am, "delta_pct": delta,
                         "before_spread_pct": spread,
                         "regression": bool(delta is not None and worse and abs(delta) > max(spread, 2.0)),
                         "note": ""})
    return rows


SUMMARY_COLUMNS = (
    ("frame p50 ms", "frame_interval_p50_ms"), ("p99 ms", "frame_interval_p99_ms"),
    ("cpu p50 ms", "cpu_p50_ms"), ("gpu scene ms", "gpu_scene_p50_ms"),
    ("draws", "draw_calls_mean"), ("upload B/frame", "upload_bytes_mean"),
    ("shadow redraw/frame", "shadow_redraws_mean"), ("RSS MiB", "rss_max_mib"),
    ("footprint MiB", "footprint_max_mib"),
)


def markdown_summary(groups: dict[tuple[str, str], list[dict]]) -> str:
    """Markdown table of per-workload medians; each cell is `median (min-max)` over repetitions."""
    header = ["workload", "profile", "runs"] + [name for name, _ in SUMMARY_COLUMNS]
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for (workload, profile), reports in sorted(groups.items()):
        agg = aggregate(reports)
        cells = [workload, profile, str(len(reports))]
        for _, key in SUMMARY_COLUMNS:
            m = agg[key]
            if m["median"] is None:
                cells.append("n/a")
            else:
                cells.append(f"{m['median']:.2f} ({m['min']:.2f}-{m['max']:.2f})")
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines)
