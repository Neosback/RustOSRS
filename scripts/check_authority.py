#!/usr/bin/env python3
"""Fail when established semantic checkpoint ownership escapes the EditRuntime boundary."""
from __future__ import annotations

import authority_audit


def main() -> int:
    violations = authority_audit.gateway_violations(authority_audit.scan())
    if not violations:
        print("semantic checkpoint gateway: OK")
        return 0

    print("Semantic checkpoint lifecycle escaped the EditRuntime gateway:")
    for hit in violations:
        print(f"{hit.path}:{hit.line}: {hit.text}")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
