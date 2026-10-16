#!/usr/bin/env python3
"""Pin or verify the supplied 530 asset inputs; this does not decode textures or certify fidelity."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'docs/verification/530-source-manifest.json'
CONSULTED = [
    'README.md',
    'Arios-Client/src/client.java',
    'Arios-Client/src/InputStream_Sub1.java',
    'Arios-Client/src/Class130.java',
    'Arios-Client/src/Class82.java',
    'Arios-Client/src/Class3_Sub28_Sub20.java',
    'Arios-Client/src/Class102.java',
    'Arios-Client/src/Class104.java',
]


def record(path: Path) -> dict:
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return {'bytes': path.stat().st_size, 'sha256': digest.hexdigest()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True, help='Canonical archived 530Cache directory')
    parser.add_argument('--client', type=Path, required=True, help='Arios-530-main root')
    parser.add_argument('--write-manifest', action='store_true')
    args = parser.parse_args()
    cache_files = ['main_file_cache.dat2'] + sorted(p.name for p in args.cache.glob('main_file_cache.idx*'))
    if 'main_file_cache.idx255' not in cache_files:
        raise SystemExit('Missing archive reference index; choose the archived cache')
    current = {
        'version': 1,
        'status': 'source_inventory_only',
        'meaning': 'Exact supplied input bytes, not a cache profile/fingerprint, decoder acceptance, original-client oracle or 2008 HD completion claim.',
        'textures_decoded': False,
        'archived_cache_files': {name: record(args.cache / name) for name in cache_files},
        'consulted_client_sources': {name: record(args.client / name) for name in CONSULTED},
    }
    if args.write_manifest:
        MANIFEST.write_text(json.dumps(current, indent=2) + '\n')
    elif json.loads(MANIFEST.read_text()) != current:
        raise SystemExit('530 input inventory or content changed; review provenance before importing')
    print(f"530 source bytes verified: {len(cache_files)} archive files, {len(CONSULTED)} consulted client files; textures remain undecoded.")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
