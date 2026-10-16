#!/usr/bin/env python3
"""Regenerate clipping fixtures from exact external Java sources, never a translated helper."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FIXTURE = ROOT / 'crates/osrs-scene/tests/fixtures/collision-deob.txt'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runelite-root', required=True, type=Path)
    parser.add_argument('--check', action='store_true', help='Compare without modifying the fixture')
    args = parser.parse_args()
    manifest = json.loads((HERE / 'sources.json').read_text())
    sources = []
    for entry in manifest.get('context_sources', []):
        path = args.runelite_root / entry['path']
        if hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
            raise SystemExit(f'Context source pin mismatch: {path}')
    for entry in manifest['sources']:
        path = args.runelite_root / entry['path']
        if hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
            raise SystemExit(f'Source pin mismatch: {path}')
        sources.append(str(path))
    with tempfile.TemporaryDirectory(prefix='rustosrs-collision-') as output:
        subprocess.run(['javac', '--release', '17', '-d', output, *sources, str(HERE / 'CollisionOracle.java')], check=True)
        result = subprocess.check_output(['java', '-cp', output, 'CollisionOracle'])
    if args.check:
        if result != FIXTURE.read_bytes():
            raise SystemExit('Collision fixture differs from the pinned deob oracle')
        print(f'Collision oracle MATCH: {len(result.splitlines()) - 1} cases')
    else:
        FIXTURE.write_bytes(result)
        print(f'Wrote {FIXTURE}: {len(result.splitlines()) - 1} cases')


if __name__ == '__main__':
    main()
