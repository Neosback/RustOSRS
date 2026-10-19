#!/usr/bin/env python3
"""Search the canonical RustOSRS editor feature registry.

This command is intentionally dependency-free and read-only. It helps contributors and
coding agents find an existing feature owner before introducing another implementation.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any, Iterable

from feature_registry import REGISTRY_PATH, RegistryError, load_registry

REFERENCE_ORDER = (
    "controller",
    "planner",
    "gateway",
    "planning_test",
    "cancellation_test",
    "commit_test",
    "undo_test",
)


def iter_strings(value: Any) -> Iterable[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for key, child in value.items():
            if isinstance(key, str):
                yield key
            yield from iter_strings(child)
    elif isinstance(value, list):
        for child in value:
            yield from iter_strings(child)


def feature_haystack(name: str, feature: dict[str, Any]) -> str:
    return "\n".join([name, *iter_strings(feature)]).casefold()


def score_feature(name: str, feature: dict[str, Any], query: str) -> int:
    needle = query.strip().casefold()
    if not needle:
        return 0

    folded_name = name.casefold()
    if needle == folded_name:
        return 1000
    if folded_name.startswith(needle):
        return 800

    haystack = feature_haystack(name, feature)
    if needle in folded_name:
        return 700
    if needle in haystack:
        return 500

    terms = [term for term in needle.split() if term]
    if terms and all(term in haystack for term in terms):
        return 300 + sum(haystack.count(term) for term in terms)

    return 0


def find_features(
    features: dict[str, dict[str, Any]], query: str
) -> list[tuple[str, dict[str, Any], int]]:
    matches = [
        (name, feature, score_feature(name, feature, query))
        for name, feature in features.items()
    ]
    return sorted(
        (match for match in matches if match[2] > 0),
        key=lambda match: (-match[2], match[0].casefold()),
    )


def describe_reference(reference: Any) -> str | None:
    if not isinstance(reference, dict):
        return None
    file = reference.get("file")
    symbol = reference.get("symbol")
    if isinstance(file, str) and isinstance(symbol, str):
        return f"{file}::{symbol}"
    return None


def print_feature(name: str, feature: dict[str, Any]) -> None:
    kind = feature.get("kind", "tool")
    mutates = bool(feature.get("mutates_document"))
    print(name)
    print(f"  kind: {kind}")
    print(f"  mutates document: {'yes' if mutates else 'no'}")

    aliases = feature.get("aliases")
    if isinstance(aliases, list) and all(isinstance(alias, str) for alias in aliases):
        print(f"  aliases: {', '.join(aliases)}")

    owner = feature.get("owner")
    if isinstance(owner, str):
        print(f"  owner: {owner}")

    for role in REFERENCE_ORDER:
        if rendered := describe_reference(feature.get(role)):
            print(f"  {role.replace('_', ' ')}: {rendered}")


def command_list(features: dict[str, dict[str, Any]]) -> int:
    for name in sorted(features, key=str.casefold):
        feature = features[name]
        kind = feature.get("kind", "tool")
        mutation = "mutating" if feature.get("mutates_document") else "read-only"
        print(f"{name}\t{kind}\t{mutation}")
    return 0


def command_find(features: dict[str, dict[str, Any]], query: str) -> int:
    matches = find_features(features, query)
    if not matches:
        print(f"No registered feature matches {query!r}.", file=sys.stderr)
        return 1

    for index, (name, feature, _) in enumerate(matches):
        if index:
            print()
        print_feature(name, feature)
    return 0


def command_show(features: dict[str, dict[str, Any]], name: str) -> int:
    exact = next(
        (
            (feature_name, feature)
            for feature_name, feature in features.items()
            if feature_name.casefold() == name.casefold()
        ),
        None,
    )
    if exact is not None:
        print_feature(*exact)
        return 0

    matches = find_features(features, name)
    if len(matches) == 1:
        print_feature(matches[0][0], matches[0][1])
        return 0

    if not matches:
        print(f"No registered feature matches {name!r}.", file=sys.stderr)
    else:
        print(
            f"{name!r} is ambiguous. Matches: "
            + ", ".join(match[0] for match in matches),
            file=sys.stderr,
        )
    return 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Navigate the canonical RustOSRS editor feature registry."
    )
    parser.add_argument(
        "--registry",
        type=Path,
        default=REGISTRY_PATH,
        help="alternate editor_features.json path (primarily for tests/tools)",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("list", help="list registered features")

    find_parser = subparsers.add_parser(
        "find", help="search feature names, paths, symbols, aliases and metadata"
    )
    find_parser.add_argument("query", nargs="+", help="concept to search for")

    show_parser = subparsers.add_parser(
        "show", help="show one feature by name (or one unambiguous search match)"
    )
    show_parser.add_argument("name", nargs="+", help="feature name")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        features = load_registry(args.registry)
    except RegistryError as exc:
        print(f"feature registry error: {exc}", file=sys.stderr)
        return 2

    if args.command == "list":
        return command_list(features)
    if args.command == "find":
        return command_find(features, " ".join(args.query))
    if args.command == "show":
        return command_show(features, " ".join(args.name))
    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
