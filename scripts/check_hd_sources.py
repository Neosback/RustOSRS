#!/usr/bin/env python3
"""Verify the pinned HD corpus and feature inventory; never regenerate acceptance evidence."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def verify_files(root: Path, expected: dict[str, str], pattern: str = "*") -> list[str]:
    actual = {str(path.relative_to(root)): path for path in root.rglob(pattern) if path.is_file()}
    errors = [f"file inventory differs: {sorted(actual.keys() ^ expected.keys())}"] if actual.keys() != expected.keys() else []
    for name in sorted(actual.keys() & expected.keys()):
        if hashlib.sha256(actual[name].read_bytes()).hexdigest() != expected[name]:
            errors.append(f"source hash changed: {name}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream-java", type=Path, help="Optional exact rs117/hd Java root")
    args = parser.parse_args()
    manifest = json.loads((ROOT / "docs/verification/117hd-source-manifest.json").read_text())
    ledger = json.loads((ROOT / "docs/verification/117hd-feature-ledger.json").read_text())
    errors = verify_files(ROOT / "assets/117hd", manifest["assets"])
    if args.upstream_java:
        errors.extend(verify_files(args.upstream_java, manifest["upstream_java"], "*.java"))
    if ledger["config_sha256"] != manifest["upstream_java"]["HdPluginConfig.java"]:
        errors.append("feature ledger uses a different configuration source")
    keys = [item["key"] for item in ledger["config_items"]]
    if not keys or len(keys) != len(set(keys)):
        errors.append("feature ledger has absent/duplicate configuration keys")
    allowed = {"audit_pending", "implemented", "verified", "equivalent", "not_applicable"}
    for item in ledger["config_items"]:
        if item["status"] not in allowed:
            errors.append(f"unknown status: {item['key']}")
        if item["status"] in {"verified", "equivalent", "not_applicable"} and not item.get("verification"):
            errors.append(f"missing disposition evidence: {item['key']}")
    if errors:
        print("\n".join(errors))
        return 1
    pending = sum(item["status"] == "audit_pending" for item in ledger["config_items"])
    print(f"HD source pins verified; {len(keys)} configuration entries, {pending} awaiting audit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
