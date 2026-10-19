#!/usr/bin/env python3
"""Print reproducible architecture metrics from RustOSRS canonical tooling.

This dashboard is intentionally read-only. It derives every value from the feature registry,
authority audit, editor surface ratchet, and explicit architecture-debt file rather than introducing
another architecture source of truth. Violation counts are gating metrics; inventory counts are
reported so later checkpoints can ratchet them only when the meaning remains reliable.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

import authority_audit
from check_architecture import load_editor_debt
from editor_architecture import app_fields, app_surface, check_features
from feature_registry import ROOT, RegistryError, load_registry

VIOLATION_METRICS = (
    "feature_contract_violations",
    "authority_gateway_violations",
    "legacy_history_violations",
    "application_history_violations",
)


class DashboardError(RuntimeError):
    """Raised when a canonical dashboard input cannot be read or interpreted."""


def collect(repo: Path = ROOT) -> dict[str, int]:
    """Collect deterministic architecture metrics for ``repo``."""
    try:
        registry = load_registry(repo / "scripts/editor_features.json")
        editor_source = repo / "crates/osrs-editor/src"
        hits = authority_audit.scan(repo)
        feature_violations = check_features(repo, registry)
        debt = load_editor_debt(repo / "scripts/architecture_debt.json")
        fields = app_fields(editor_source)
        surface = app_surface(editor_source)
    except (OSError, ValueError, TypeError, RegistryError) as error:
        raise DashboardError(str(error)) from error

    return {
        "registered_features": len(registry),
        "registered_mutating_features": sum(
            bool(feature.get("mutates_document")) for feature in registry.values()
        ),
        "feature_contract_violations": len(feature_violations),
        "authority_gateway_violations": len(authority_audit.gateway_violations(hits)),
        "legacy_history_violations": len(authority_audit.legacy_history_violations(hits)),
        "application_history_violations": len(
            authority_audit.application_history_violations(hits)
        ),
        "tracked_lifecycle_write_sites": sum(
            hit.category == "lifecycle-write" for hit in hits
        ),
        "explicit_editor_debt_modules": len(debt),
        "editor_app_fields": len(fields),
        "editor_app_methods": sum(len(methods) for methods in surface.values()),
    }


def render(metrics: dict[str, int]) -> str:
    """Render a compact, stable human-readable dashboard."""
    rows = [
        ("registered features", metrics["registered_features"]),
        ("registered mutating features", metrics["registered_mutating_features"]),
        ("feature contract violations", metrics["feature_contract_violations"]),
        ("authority gateway violations", metrics["authority_gateway_violations"]),
        ("legacy history violations", metrics["legacy_history_violations"]),
        ("application history violations", metrics["application_history_violations"]),
        ("tracked lifecycle write sites", metrics["tracked_lifecycle_write_sites"]),
        ("explicit editor debt modules", metrics["explicit_editor_debt_modules"]),
        ("EditorApp fields", metrics["editor_app_fields"]),
        ("EditorApp methods", metrics["editor_app_methods"]),
    ]
    width = max(len(label) for label, _ in rows)
    return "Architecture dashboard\n" + "\n".join(
        f"  {label:<{width}} : {value}" for label, value in rows
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)

    try:
        metrics = collect()
    except DashboardError as error:
        print(f"architecture dashboard error: {error}", file=sys.stderr)
        return 2

    if args.json:
        print(json.dumps({"version": 1, "metrics": metrics}, indent=2, sort_keys=True))
    else:
        print(render(metrics))

    failed = [name for name in VIOLATION_METRICS if metrics[name] != 0]
    if failed:
        print("architecture dashboard violations: " + ", ".join(failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
