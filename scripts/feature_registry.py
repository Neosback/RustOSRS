"""Canonical loader for the RustOSRS editor feature registry.

All tooling that consumes ``scripts/editor_features.json`` must load it through this module. The
registry is explicitly versioned so schema changes are deliberate and every consumer interprets the
same machine-readable source of truth.
"""
from __future__ import annotations

import json
from pathlib import Path
import re
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "scripts" / "editor_features.json"
CURRENT_SCHEMA_VERSION = 2
FEATURE_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
FEATURE_STATUSES = frozenset({"implemented", "experimental", "planned", "deprecated"})


class RegistryError(ValueError):
    """Raised when the feature registry cannot be read or normalized."""


def _non_empty_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise RegistryError(f"{label} must be a non-empty string")
    return value


def _validate_feature(name: str, feature: dict[str, Any], seen_ids: set[str]) -> None:
    feature_id = _non_empty_string(feature.get("id"), f"feature {name!r} id")
    if not FEATURE_ID.fullmatch(feature_id):
        raise RegistryError(f"feature {name!r} id must be a lowercase kebab-case identifier")
    if feature_id in seen_ids:
        raise RegistryError(f"duplicate feature id {feature_id!r}")
    seen_ids.add(feature_id)

    aliases = feature.get("aliases")
    if not isinstance(aliases, list) or any(
        not isinstance(alias, str) or not alias.strip() for alias in aliases
    ):
        raise RegistryError(f"feature {name!r} aliases must be a list of non-empty strings")
    folded_aliases = [alias.casefold() for alias in aliases]
    if len(folded_aliases) != len(set(folded_aliases)):
        raise RegistryError(f"feature {name!r} aliases must be unique ignoring case")

    owner = feature.get("owner")
    if not isinstance(owner, dict):
        raise RegistryError(f"feature {name!r} owner must be an object")
    _non_empty_string(owner.get("crate"), f"feature {name!r} owner.crate")
    _non_empty_string(owner.get("area"), f"feature {name!r} owner.area")

    status = _non_empty_string(feature.get("status"), f"feature {name!r} status")
    if status not in FEATURE_STATUSES:
        allowed = ", ".join(sorted(FEATURE_STATUSES))
        raise RegistryError(f"feature {name!r} status must be one of: {allowed}")


def _register_lookup_terms(
    name: str,
    feature: dict[str, Any],
    lookup_owners: dict[str, str],
) -> None:
    terms = [name, feature["id"], *feature["aliases"]]
    for term in terms:
        folded = term.casefold()
        previous = lookup_owners.get(folded)
        if previous is not None and previous != name:
            raise RegistryError(
                f"feature lookup term {term!r} is shared by {previous!r} and {name!r}"
            )
        lookup_owners[folded] = name


def normalize_registry(payload: object) -> dict[str, dict[str, Any]]:
    """Return the canonical feature map from the current registry document."""
    if not isinstance(payload, dict):
        raise RegistryError("feature registry must be a JSON object")

    version = payload.get("version")
    if version != CURRENT_SCHEMA_VERSION:
        raise RegistryError(
            f"unsupported feature registry version {version!r}; "
            f"expected {CURRENT_SCHEMA_VERSION}"
        )
    features = payload.get("features")
    if not isinstance(features, dict):
        raise RegistryError("feature registry must contain a feature object")

    result: dict[str, dict[str, Any]] = {}
    seen_ids: set[str] = set()
    lookup_owners: dict[str, str] = {}
    for name, feature in features.items():
        if not isinstance(name, str) or not name.strip():
            raise RegistryError("feature names must be non-empty strings")
        if not isinstance(feature, dict):
            raise RegistryError(f"feature {name!r} must be an object")
        _validate_feature(name, feature, seen_ids)
        _register_lookup_terms(name, feature, lookup_owners)
        result[name] = feature
    return result


def load_registry(path: Path = REGISTRY_PATH) -> dict[str, dict[str, Any]]:
    """Read and normalize an editor feature registry from disk."""
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise RegistryError(f"cannot read {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise RegistryError(f"invalid JSON in {path}: {exc}") from exc
    return normalize_registry(payload)
