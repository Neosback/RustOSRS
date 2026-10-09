#!/usr/bin/env python3
"""Offline validation for the historical deob_golden index."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDEX_PATH = ROOT / "reference-fixtures/historical/deob_golden.index.json"


def require_hex(value: str, length: int, label: str) -> None:
    assert len(value) == length and all(ch in "0123456789abcdefABCDEF" for ch in value), (
        f"{label} must be {length} hex characters"
    )


def main() -> None:
    index = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    assert index["schema_version"] == 1

    source = index["source"]
    harness = index["harness"]
    require_hex(source["git_blob"], 40, "historical source git_blob")
    require_hex(harness["git_blob"], 40, "historical harness git_blob")

    source_path = ROOT / source["path"]
    harness_path = ROOT / harness["path"]
    assert source_path.is_file(), f"missing historical source {source_path}"
    assert harness_path.is_file(), f"missing historical harness {harness_path}"

    lines = source_path.read_text(encoding="utf-8").splitlines()
    fixture_ids: set[str] = set()

    for entry in index["entries"]:
        fixture_id = entry["fixture_id"]
        assert fixture_id not in fixture_ids, f"duplicate historical fixture id {fixture_id}"
        fixture_ids.add(fixture_id)
        assert entry["owned_specs"], f"{fixture_id} must own at least one spec"

        oracle = entry["public_oracle"]
        require_hex(oracle["commit"], 40, f"{fixture_id} oracle commit")
        require_hex(oracle["blob"], 40, f"{fixture_id} oracle blob")
        assert oracle["repository"] == "melxin/runelite"
        assert oracle["path"]

        has_prefix = "prefix" in entry
        has_exact = "exact_lines" in entry
        assert has_prefix != has_exact, (
            f"{fixture_id} must use exactly one historical selector form"
        )

        if has_prefix:
            matches = [line for line in lines if line.startswith(entry["prefix"])]
            assert len(matches) == entry["expected_count"], (
                f"{fixture_id}: expected {entry['expected_count']} lines with prefix "
                f"{entry['prefix']!r}, found {len(matches)}"
            )
        else:
            assert entry["exact_lines"], f"{fixture_id} exact_lines must not be empty"
            for expected in entry["exact_lines"]:
                assert lines.count(expected) == 1, (
                    f"{fixture_id}: expected historical line exactly once: {expected!r}"
                )

    required_ids = {
        "terrain.shape_gallery.all_13x4",
        "contour.synthetic.flat_slope",
        "lighting.synthetic_triangle.loc_rig",
        "placement.wall_types.orientation_matrix",
        "placement.decor_types.orientation_matrix",
        "placement.floor_type22.storage",
        "placement.game_object.footprint_and_capacity",
    }
    assert fixture_ids == required_ids


if __name__ == "__main__":
    main()
