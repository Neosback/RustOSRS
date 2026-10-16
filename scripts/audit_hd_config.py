#!/usr/bin/env python3
"""Read the pinned RLHD config defaults without promoting inventory into acceptance evidence."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def method_body(text: str, start: int) -> str:
    """Find the matching Java brace, ignoring strings, character literals and comments."""
    tokens = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//[^\n]*|/\*.*?\*/|[{}]', re.S)
    depth = 1
    for token in tokens.finditer(text, start):
        if token.group() == '{':
            depth += 1
        elif token.group() == '}':
            depth -= 1
            if depth == 0:
                return text[start:token.start()].strip()
    raise ValueError('Unclosed configuration method')


def inventory(text: str) -> dict:
    constants = dict(re.findall(r'String\s+(KEY_\w+)\s*=\s*"([^"]+)"', text))
    result = {}
    for method in re.finditer(r'default\s+([\w<>\[\]]+)\s+(\w+)\s*\(\s*\)\s*\{', text):
        annotation = text[text.rfind('@ConfigItem(', 0, method.start()):method.start()]
        key = re.search(r'keyName\s*=\s*("[^"]+"|\w+)', annotation)
        if not key:
            continue
        token = key.group(1)
        key = token.strip('"') if token.startswith('"') else constants[token]
        if key in result:
            raise ValueError(f"Unrecognized/duplicate config method: {key}")
        result[key] = {
            "file": "HdPluginConfig.java",
            "line": text.count('\n', 0, method.start()) + 1,
            "method": method.group(2),
            "default_body": method_body(text, method.end()),
        }
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('config', type=Path)
    parser.add_argument('--write-source-anchors', action='store_true')
    args = parser.parse_args()
    path = ROOT / 'docs/verification/117hd-feature-ledger.json'
    ledger = json.loads(path.read_text())
    data = args.config.read_bytes()
    if hashlib.sha256(data).hexdigest() != ledger['config_sha256']:
        raise SystemExit('Configuration does not match the pinned source')
    defaults = inventory(data.decode())
    if defaults.keys() != {item['key'] for item in ledger['config_items']}:
        raise SystemExit('Configuration inventory differs from the ledger')
    if args.write_source_anchors:
        for item in ledger['config_items']:
            item['source'] = defaults[item['key']]
        path.write_text(json.dumps(ledger, indent=2) + '\n')
    else:
        for item in ledger['config_items']:
            if item.get('source') != defaults[item['key']]:
                raise SystemExit(f"Source anchor/default differs: {item['key']}")
    print(f"Verified {len(defaults)} source defaults; implementation dispositions are unchanged.")


if __name__ == '__main__':
    main()
