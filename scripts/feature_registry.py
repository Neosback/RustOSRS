"""Canonical loader for the RustOSRS editor feature registry.

All tooling that consumes ``scripts/editor_features.json`` should load it through this module.
The loader accepts the legacy flat feature map and the versioned wrapper introduced by the
architecture cleanup so migrations do not fork parsing or validation rules.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "scripts" / "editor_features.json"
CURRENT_SCHEMA_VERSION = 1


class RegistryError(ValueError):
    """Raised when the feature registry cannot be read or normalized."""


def normalize_registry(payload: object) -> dict[str, dict[str, Any]]:
    """Return the canonical feature map from a supported registry document."""
    if not isinstance(payload, dict):
        raise RegistryError("feature registry must be a JSON object")

    if "features" in payload:
        version = payload.get("version")
        if version != CURRENT_SCHEMA_VERSION:
            raise RegistryError(
                f"unsupported feature registry version {version!r}; "
                f"expected {CURRENT_SCHEMA_VERSION}"
            )
        features = payload.get("features")
    else:
        # Compatibility path for the pre-versioned registry. CP1B migrates the repository
        # itself to the wrapper, then this branch can be removed in a later cleanup.
        features = payload

    if not isinstance(features, dict):
        raise RegistryError("feature registry must contain a feature object")

    result: dict[str, dict[str, Any]] = {}
    for name, feature in features.items():
        if not isinstance(name, str) or not name.strip():
            raise RegistryError("feature names must be non-empty strings")
        if not isinstance(feature, dict):
            raise RegistryError(f"feature {name!r} must be an object")
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
