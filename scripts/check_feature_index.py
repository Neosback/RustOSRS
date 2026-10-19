#!/usr/bin/env python3
"""Fail when the generated human feature index is stale."""
from __future__ import annotations

import feature_index


if __name__ == "__main__":
    raise SystemExit(feature_index.main(["--check"]))
