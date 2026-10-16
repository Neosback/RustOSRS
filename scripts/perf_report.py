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
REQUIRED_METADATA = (
    "commit", "working_tree_status", "tracked_diff_sha256", "binary_sha256", "toolchain",
    "hardware", "machine_state", "workload", "profile", "vsync", "window_logical_points",
    "base_settings", "workload_settings", "workload_script", "fixed_time_seconds",
    "viewport_reuse", "cache_path", "run_seconds",
)
# These must be identical across repetitions of a workload and between before/after directories.
# Source/binary identity is intentionally excluded from before/after compatibility because that is
# normally what the comparison is measuring. Machine state is recorded for diagnosis, not equality.
COMPARISON_METADATA = (
    "toolchain", "hardware", "cache_path", "viewport_reuse", "window_logical_points",
    "base_settings", "workload_settings", "workload_script", "fixed_time_seconds", "run_seconds",
)
REPETITION_METADATA = (
    "commit", "working_tree_status", "tracked_diff_sha256", "binary_sha256", *COMPARISON_METADATA,
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


def sync_mode(metadata: dict) -> str:
    return "capped" if metadata.get("vsync") is True else "uncapped"


def validate(report: dict) -> list[str]:
    """Return a list of schema/provenance problems; empty means the report is usable."""
    errors: list[str] = []
    if report.get("schema_version") != SCHEMA_VERSION:
        return [f"schema_version {report.get('schema_version')!r} != {SCHEMA_VERSION}"]
    errors += [f"missing top-level key {k!r}" for k in REQUIRED_TOP_LEVEL if k not in report]
    metadata = report.get("metadata")
    if not isinstance(metadata, dict):
        errors.append("metadata is not an object")
    else:
        errors += [f"missing metadata key {k!r}" for k in REQUIRED_METADATA if k not in metadata]
        if "vsync" in metadata and not isinstance(metadata["vsync"], bool):
            errors.append("metadata 'vsync' is not a bool")
        if not isinstance(metadata.get("workload"), str) or not metadata.get("workload"):
            errors.append("metadata 'workload' must be a non-empty string")
        if not isinstance(metadata.get("profile"), str) or not metadata.get("profile"):
            errors.append("metadata 'profile' must be a non-empty string")
    summary = report.get("summary")
    if isinstance(summary, dict):
        errors += [f"missing summary key {k!r}" for k in REQUIRED_SUMMARY if k not in summary]
    samples = report.get("samples")
    if not samples:
        errors.append("no post-warmup samples")
    elif not isinstance(samples, list):
        errors.append("samples is not a list")
    else:
        for index, sample in enumerate(samples):
            if not isinstance(sample, dict):
                errors.append(f"sample {index} is not an object")
                continue
            errors += [f"sample {index} missing {k!r}" for k in REQUIRED_SAMPLE if k not in sample]
    segments = report.get("segments")
    if isinstance(samples, list) and samples and isinstance(segments, list):
        used = {s.get("segment") for s in samples if isinstance(s, dict)}
        errors += [f"sample segment {i} outside the segment table" for i in used
                   if not isinstance(i, int) or i < 0 or i >= len(segments)]
    return errors


STALL_MS = 1000.0


def stalls(report: dict) -> list[float]:
    """Frame intervals above one second indicate a contaminated, throttled/slept run."""
    return [s["interval_ms"] for s in report.get("samples", [])
            if (s.get("interval_ms") or 0.0) > STALL_MS]


def camera_drift(report: dict) -> int:
    """Count distinct camera hashes beyond one per segment."""
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


def _metadata_mismatches(reference: dict, candidate: dict, fields: Iterable[str]) -> list[str]:
    return [field for field in fields if reference.get(field) != candidate.get(field)]


def _validate_group(reports: list[dict], key: tuple[str, str, str]) -> None:
    if not reports:
        return
    reference = reports[0]["metadata"]
    for index, report in enumerate(reports[1:], 1):
        mismatches = _metadata_mismatches(reference, report["metadata"], REPETITION_METADATA)
        if mismatches:
            raise ValueError(
                f"{key[0]}/{key[1]}/{key[2]} repetition {index} has incompatible provenance: "
                + ", ".join(mismatches)
            )


def load_runs(directory: Path) -> dict[tuple[str, str, str], list[dict]]:
    """Group `<workload>-<profile>-<sync>-<repetition>.json` reports with strict provenance."""
    groups: dict[tuple[str, str, str], list[dict]] = {}
    for path in sorted(directory.glob("*.json")):
        if path.name.endswith((".metadata.json", ".settings.json", ".footprint.json")):
            continue
        parts = path.stem.rsplit("-", 3)
        if len(parts) != 4 or parts[2] not in {"capped", "uncapped"} or not parts[3].isdigit():
            continue
        report = json.loads(path.read_text())
        problems = validate(report)
        if problems:
            raise ValueError(f"{path.name}: " + "; ".join(problems))
        metadata = report["metadata"]
        expected = (parts[0], parts[1], parts[2])
        actual = (metadata["workload"], metadata["profile"], sync_mode(metadata))
        if actual != expected:
            raise ValueError(
                f"{path.name}: filename identifies {expected}, embedded metadata identifies {actual}"
            )
        groups.setdefault(expected, []).append(report)
    for key, reports in groups.items():
        _validate_group(reports, key)
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


def _validate_comparison_pair(key: tuple[str, str, str], before: list[dict], after: list[dict]) -> None:
    _validate_group(before, key)
    _validate_group(after, key)
    mismatches = _metadata_mismatches(
        before[0]["metadata"], after[0]["metadata"], COMPARISON_METADATA
    )
    if mismatches:
        raise ValueError(
            f"{key[0]}/{key[1]}/{key[2]} before/after environments differ: "
            + ", ".join(mismatches)
        )


def compare(before: dict, after: dict) -> list[dict]:
    """Rows of per-metric deltas for every compatible workload/profile/sync key."""
    rows = []
    for key in sorted(set(before) & set(after)):
        _validate_comparison_pair(key, before[key], after[key])
        b, a = aggregate(before[key]), aggregate(after[key])
        for name, (_, lower_is_better) in KEY_METRICS.items():
            bm, am = b[name]["median"], a[name]["median"]
            common = {"workload": key[0], "profile": key[1], "sync": key[2], "metric": name}
            if bm is None or am is None:
                rows.append({**common, "before": bm, "after": am, "delta_pct": None,
                             "regression": None, "note": "unavailable in one side"})
                continue
            delta = (am - bm) / bm * 100.0 if bm else None
            spread = (b[name]["max"] - b[name]["min"]) / bm * 100.0 if bm else 0.0
            worse = (am > bm) if lower_is_better else (am < bm)
            rows.append({**common, "before": bm, "after": am, "delta_pct": delta,
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


def markdown_summary(groups: dict[tuple[str, str, str], list[dict]]) -> str:
    """Markdown table of per-workload medians; each cell is median (min-max)."""
    header = ["workload", "profile", "sync", "runs"] + [name for name, _ in SUMMARY_COLUMNS]
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for (workload, profile, sync), reports in sorted(groups.items()):
        agg = aggregate(reports)
        cells = [workload, profile, sync, str(len(reports))]
        for _, key in SUMMARY_COLUMNS:
            metric = agg[key]
            if metric["median"] is None:
                cells.append("n/a")
            else:
                cells.append(
                    f"{metric['median']:.2f} ({metric['min']:.2f}-{metric['max']:.2f})"
                )
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines)
