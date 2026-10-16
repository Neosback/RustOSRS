"""Evidence-backed bottleneck ranking for certified RustOSRS performance baselines."""
from __future__ import annotations

import hashlib
import json
import statistics
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import perf_report
import performance_baseline

ANALYSIS_SCHEMA = 1
MANIFEST_KIND = "rustosrs-performance-baseline"
REQUIRED_MANIFEST = (
    "schema_version", "kind", "certified", "commit", "binary_sha256", "frozen_binary",
    "profiles", "workloads", "sync_modes", "repetitions", "expected_reports",
    "target_machine_problems",
)
HOTSPOT_FAMILIES = {
    "Frame and CPU": (
        ("frame_interval_p50_ms", "frame p50", "ms"),
        ("frame_interval_p95_ms", "frame p95", "ms"),
        ("frame_interval_p99_ms", "frame p99", "ms"),
        ("cpu_p50_ms", "CPU p50", "ms"),
        ("cpu_p95_ms", "CPU p95", "ms"),
    ),
    "GPU": (
        ("gpu_scene_p50_ms", "scene GPU p50", "ms"),
        ("gpu_shadow_p50_ms", "shadow GPU p50", "ms"),
        ("gpu_post_p50_ms", "post GPU p50", "ms"),
    ),
    "Scene traffic": (
        ("draw_calls_mean", "draw calls/frame", "count"),
        ("vertices_mean", "vertices/frame", "count"),
        ("upload_bytes_mean", "upload bytes/frame", "bytes"),
        ("shadow_redraws_mean", "shadow redraws/frame", "count"),
    ),
    "Projection and rebuild": (
        ("build_compute_p50_ms", "build compute p50", "ms"),
        ("build_queue_wait_p95_ms", "build queue wait p95", "ms"),
        ("build_install_p95_ms", "build install p95", "ms"),
        ("anim_rebuild_p50_ms", "animation rebuild p50", "ms"),
    ),
    "Memory": (
        ("rss_max_mib", "RSS max", "MiB"),
        ("footprint_max_mib", "physical footprint max", "MiB"),
    ),
}
PROFILE_DELTA_METRICS = (
    ("cpu_p50_ms", "CPU"),
    ("gpu_scene_p50_ms", "scene GPU"),
    ("gpu_shadow_p50_ms", "shadow GPU"),
    ("gpu_post_p50_ms", "post GPU"),
    ("draw_calls_mean", "draw calls"),
    ("upload_bytes_mean", "uploads"),
    ("rss_max_mib", "RSS"),
    ("footprint_max_mib", "footprint"),
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_certified_baseline(
    directory: Path, *, allow_non_target_machine: bool = False
) -> tuple[dict, dict[tuple[str, str, str], list[dict]]]:
    directory = directory.resolve()
    path = directory / "baseline-manifest.json"
    if not path.is_file():
        raise ValueError(f"baseline manifest is missing: {path}")
    manifest = json.loads(path.read_text())
    problems = [f"missing manifest key {key!r}" for key in REQUIRED_MANIFEST if key not in manifest]
    if manifest.get("schema_version") != 1:
        problems.append(f"manifest schema_version {manifest.get('schema_version')!r} != 1")
    if manifest.get("kind") != MANIFEST_KIND:
        problems.append(f"manifest kind {manifest.get('kind')!r} != {MANIFEST_KIND!r}")
    if manifest.get("certified") is not True:
        problems.append("baseline manifest is not certified")
    if manifest.get("sync_modes") != ["uncapped", "capped"]:
        problems.append("manifest sync_modes must be ['uncapped', 'capped']")
    if not isinstance(manifest.get("repetitions"), int) or manifest.get("repetitions", 0) < 1:
        problems.append("manifest repetitions must be a positive integer")
    if problems:
        raise ValueError("; ".join(problems))
    if manifest["target_machine_problems"] and not allow_non_target_machine:
        raise ValueError(
            "baseline was captured on a non-target machine: "
            + "; ".join(manifest["target_machine_problems"])
        )

    frozen = directory / manifest["frozen_binary"]
    if not frozen.is_file():
        raise ValueError(f"frozen baseline binary is missing: {frozen}")
    frozen_sha = sha256_file(frozen)
    if frozen_sha != manifest["binary_sha256"]:
        raise ValueError("frozen baseline binary hash differs from the manifest")

    groups, facts = performance_baseline.certify_directory(
        directory, list(manifest["workloads"]), list(manifest["profiles"]),
        manifest["repetitions"], frozen_sha, manifest["commit"],
    )
    expected = len(groups) * manifest["repetitions"]
    if manifest["expected_reports"] != expected or facts["reports"] != expected:
        raise ValueError(
            f"manifest/report count mismatch: manifest={manifest['expected_reports']}, "
            f"actual={facts['reports']}"
        )
    for field in ("commit", "binary_sha256", "toolchain", "hardware", "cache_path"):
        if manifest.get(field) != facts.get(field):
            raise ValueError(f"manifest {field} differs from certified reports")
    return manifest, groups


def rank_metric(
    groups: dict[tuple[str, str, str], list[dict]], metric: str, top: int
) -> dict[str, Any]:
    available = []
    for key, reports in groups.items():
        value = perf_report.aggregate(reports)[metric]["median"]
        if value is not None:
            available.append((float(value), key))
    available.sort(reverse=True)
    matrix_median = statistics.median(value for value, _ in available) if available else None
    rows = []
    for value, key in available[:top]:
        rows.append({
            "workload": key[0], "profile": key[1], "sync": key[2], "value": value,
            "matrix_median": matrix_median,
            "factor_vs_matrix_median":
                value / matrix_median if matrix_median not in (None, 0.0) else None,
        })
    return {"metric": metric, "available_groups": len(available), "rows": rows}


def nested_hotspots(
    groups: dict[tuple[str, str, str], list[dict]], section: str, top: int
) -> list[dict[str, Any]]:
    rows = []
    for key, reports in groups.items():
        names = sorted({
            name for report in reports
            for name in (perf_report.dig(report, ("summary", section)) or {})
        })
        for name in names:
            values = [
                float(value) for report in reports
                if isinstance(
                    (value := perf_report.dig(report, ("summary", section, name, "p50"))),
                    (int, float),
                )
            ]
            if values:
                rows.append({
                    "workload": key[0], "profile": key[1], "sync": key[2],
                    "name": name, "value_ms": statistics.median(values),
                })
    rows.sort(key=lambda row: row["value_ms"], reverse=True)
    return rows[:top]


def profile_deltas(groups: dict[tuple[str, str, str], list[dict]]) -> list[dict[str, Any]]:
    rows = []
    for workload in sorted({key[0] for key in groups}):
        for sync in sorted({key[2] for key in groups}):
            ref_key, hd_key = (workload, "Reference", sync), (workload, "Hd", sync)
            if ref_key not in groups or hd_key not in groups:
                continue
            ref, hd = perf_report.aggregate(groups[ref_key]), perf_report.aggregate(groups[hd_key])
            metrics = {}
            for metric, _ in PROFILE_DELTA_METRICS:
                before, after = ref[metric]["median"], hd[metric]["median"]
                delta = (
                    (after - before) / before * 100.0
                    if before not in (None, 0.0) and after is not None else None
                )
                metrics[metric] = {"reference": before, "hd": after, "delta_pct": delta}
            rows.append({"workload": workload, "sync": sync, "metrics": metrics})
    return rows


def analyze(manifest: dict, groups: dict[tuple[str, str, str], list[dict]], top: int) -> dict:
    return {
        "schema_version": ANALYSIS_SCHEMA,
        "kind": "rustosrs-performance-bottleneck-analysis",
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "baseline": {
            key: manifest[key] for key in (
                "commit", "binary_sha256", "hardware", "cache_path", "profiles",
                "workloads", "sync_modes", "repetitions",
            )
        },
        "method": {
            "synthetic_score": False,
            "ranking": "descending within each original measurement unit",
            "profile_delta": "Hd minus Reference for identical workload and sync mode",
            "memory_accounting": "RSS and physical footprint remain separate and are never added",
        },
        "families": {
            family: [
                {**rank_metric(groups, metric, top), "label": label, "unit": unit}
                for metric, label, unit in specs
            ]
            for family, specs in HOTSPOT_FAMILIES.items()
        },
        "cpu_phase_hotspots": nested_hotspots(groups, "phases_ms", top),
        "gpu_pass_hotspots": nested_hotspots(groups, "gpu_pass_ms", top),
        "profile_deltas": profile_deltas(groups),
    }


def _value(value: float | None, unit: str) -> str:
    if value is None:
        return "n/a"
    if unit == "bytes":
        return f"{value:,.0f}"
    if unit == "count":
        return f"{value:,.2f}"
    return f"{value:.2f}"


def _pct(value: float | None) -> str:
    return "n/a" if value is None else f"{value:+.1f}%"


def markdown(analysis: dict) -> str:
    baseline = analysis["baseline"]
    lines = [
        "# RustOSRS certified baseline bottleneck analysis", "",
        f"- Commit: `{baseline['commit']}`",
        f"- Binary SHA-256: `{baseline['binary_sha256']}`",
        f"- Cache: `{baseline['cache_path']}`",
        f"- Repetitions per matrix cell: {baseline['repetitions']}", "",
        "No cross-unit synthetic score is used. Larger values are listed first within each metric. "
        "This ranks evidence; it does not by itself prove the cause of a hotspot.", "",
    ]
    for family, metrics in analysis["families"].items():
        lines += [f"## {family}", ""]
        for metric in metrics:
            lines += [
                f"### {metric['label']}", "",
                "| workload | profile | sync | value | matrix median | factor |",
                "|---|---|---|---:|---:|---:|",
            ]
            if not metric["rows"]:
                lines.append("| n/a | n/a | n/a | n/a | n/a | n/a |")
            for row in metric["rows"]:
                factor = row["factor_vs_matrix_median"]
                lines.append(
                    f"| {row['workload']} | {row['profile']} | {row['sync']} | "
                    f"{_value(row['value'], metric['unit'])} | "
                    f"{_value(row['matrix_median'], metric['unit'])} | "
                    f"{'n/a' if factor is None else f'{factor:.2f}x'} |"
                )
            lines.append("")
    for title, key in (
        ("CPU phase hotspots", "cpu_phase_hotspots"), ("GPU pass hotspots", "gpu_pass_hotspots")
    ):
        lines += [
            f"## {title}", "",
            "| workload | profile | sync | name | median p50 ms |",
            "|---|---|---|---|---:|",
        ]
        if not analysis[key]:
            lines.append("| n/a | n/a | n/a | n/a | n/a |")
        for row in analysis[key]:
            lines.append(
                f"| {row['workload']} | {row['profile']} | {row['sync']} | "
                f"{row['name']} | {row['value_ms']:.2f} |"
            )
        lines.append("")
    lines += [
        "## HD minus Reference deltas", "",
        "Each row compares identical workload and sync mode. Positive means HD measured higher.", "",
        "| workload | sync | CPU | scene GPU | shadow GPU | post GPU | draw calls | uploads | RSS | footprint |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in analysis["profile_deltas"]:
        cells = [_pct(row["metrics"][metric]["delta_pct"]) for metric, _ in PROFILE_DELTA_METRICS]
        lines.append(f"| {row['workload']} | {row['sync']} | " + " | ".join(cells) + " |")
    if not analysis["profile_deltas"]:
        lines.append("| n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |")
    lines += [
        "", "## Interpretation boundary", "",
        "Choose optimization work only after correlating a ranked hotspot with the corresponding raw "
        "samples/events and code path. RSS and physical footprint remain independent and must not be "
        "summed on unified-memory systems.", "",
    ]
    return "\n".join(lines)


def write_analysis(directory: Path, analysis: dict, prefix: str) -> tuple[Path, Path]:
    json_path, md_path = directory / f"{prefix}.json", directory / f"{prefix}.md"
    json_path.write_text(json.dumps(analysis, indent=2) + "\n")
    md_path.write_text(markdown(analysis))
    return json_path, md_path
