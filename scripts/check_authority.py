#!/usr/bin/env python3
"""Fail when established semantic-edit ownership or history authority regresses."""
from __future__ import annotations

import authority_audit


def print_violations(title: str, violations: list[authority_audit.Hit]) -> None:
    print(title)
    for hit in violations:
        print(f"{hit.path}:{hit.line}: {hit.text}")


def main() -> int:
    hits = authority_audit.scan()
    gateway = authority_audit.gateway_violations(hits)
    legacy = authority_audit.legacy_history_violations(hits)
    application = authority_audit.application_history_violations(hits)

    if gateway:
        print_violations("Semantic checkpoint lifecycle escaped the EditRuntime gateway:", gateway)
    else:
        print("semantic checkpoint gateway: OK")

    if legacy:
        print_violations("Legacy EditStep payload authority grew beyond the CP2 baseline:", legacy)
    else:
        print("legacy history authority ratchet: OK")

    if application:
        print_violations(
            "Application-level chronology or retired history routing was reintroduced:",
            application,
        )
    else:
        print("application history authority ratchet: OK")

    return 1 if gateway or legacy or application else 0


if __name__ == "__main__":
    raise SystemExit(main())
