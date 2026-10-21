#!/usr/bin/env python3
"""Focused development checks by default; --full runs every required gate once (local and CI)."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from run_tests import ROOT, source_identity, verification_lock

PYTHON_CHECKS = [
    'check_architecture.py', 'check_authority.py', 'check_feature_index.py',
    'test_check_architecture.py', 'test_editor_architecture.py', 'test_authority_audit.py',
    'test_feature_registry.py', 'test_features.py', 'test_feature_index.py',
    'test_reference_regeneration.py', 'test_normal_oracle.py', 'test_deob_golden_index.py', 'check_hd_sources.py',
    'test_hd_sources.py', 'test_audit_hd_config.py', 'test_run_tests.py', 'test_lab_training.py',
]
# Every crate that must build for wasm32. osrs-editor is the web entry point; the rest are its
# portable libraries, listed so their lints are enforced with -D warnings too.
WASM_PACKAGES = ['osrs-editor', 'osrs-core', 'osrs-edit', 'osrs-lab', 'osrs-cache', 'osrs-scene',
                 'osrs-hd', 'osrs-render', 'osrs-world']
STAGES = ('all', 'quick', 'build')


def commands(full, packages, jobs, output, stage='all'):
    """(name, stage, argv) in cheapest-first order; `quick` needs no compilation."""
    result = [(name, 'quick', [sys.executable, f'scripts/{name}']) for name in PYTHON_CHECKS]
    result.append(('format', 'quick', ['cargo', 'fmt', '--all', '--check']))
    selection = ['--workspace'] if full else [arg for package in packages for arg in ['-p', package]]
    # Clippy type-checks every target (examples included), so there is no separate cargo check.
    result.append(('native-clippy', 'build', ['cargo', 'clippy', '--locked', *selection,
                                              '--all-targets', '--all-features', '--', '-D', 'warnings']))
    if full:
        # One wasm command: clippy implies check, so the portable crates are compiled once.
        result.append(('wasm-clippy', 'build', ['cargo', 'clippy', '--locked', '--target', 'wasm32-unknown-unknown',
                                                *[arg for package in WASM_PACKAGES for arg in ['-p', package]],
                                                '--', '-D', 'warnings']))
    tests = [sys.executable, 'scripts/run_tests.py', '--jobs', str(jobs), '--output', str(output / 'tests')]
    if not full:
        tests.extend(selection)
    result.append(('tests', 'build', tests))
    return [(name, command) for name, step_stage, command in result if stage in ('all', step_stage)]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--full', action='store_true', help='All required gates; no package filtering')
    parser.add_argument('-p', '--package', action='append', help='Development package (repeatable)')
    parser.add_argument('-j', '--jobs', type=int, default=4, help='Concurrent test executables')
    parser.add_argument('--stage', choices=STAGES, default='all',
                        help='quick = Python guards + rustfmt (no compilation); build = clippy, wasm, tests')
    args = parser.parse_args(argv)
    if args.jobs < 1:
        parser.error('Concurrency must be positive')
    if args.full and args.package:
        parser.error('--full cannot filter packages')
    if not args.full and not args.package:
        parser.error('Choose development packages with -p, or use --full at a checkpoint')
    with verification_lock():
        return run(args)


def run(args):
    output = ROOT / 'target/verify' / time.strftime('gates-%Y%m%d-%H%M%S')
    output.mkdir(parents=True, exist_ok=False)
    before = source_identity()
    started = time.monotonic()
    report = {'version': 1, 'mode': 'full' if args.full else 'focused', 'stage': args.stage,
              'source_before': before, 'steps': [], 'success': False}
    ci = os.environ.get('GITHUB_ACTIONS') == 'true'
    for name, command in commands(args.full, args.package or [], args.jobs, output, args.stage):
        step_start = time.monotonic()
        print(f'\n{"::group::" if ci else ""}{name}: {" ".join(command)}', flush=True)
        env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1',
               'RUSTOSRS_VERIFY_PARENT': str(os.getpid())}
        code = subprocess.run(command, cwd=ROOT, env=env, check=False).returncode
        row = {'name': name, 'command': command, 'returncode': code,
               'seconds': round(time.monotonic() - step_start, 3)}
        report['steps'].append(row)
        print(f'{name}: {row["seconds"]:.1f}s', flush=True)
        if ci:
            print('::endgroup::', flush=True)
        if code:
            break
    report['source_after'] = source_identity()
    report['source_changed'] = report['source_after'] != before
    report['seconds'] = round(time.monotonic() - started, 3)
    report['success'] = not report['source_changed'] and all(step['returncode'] == 0 for step in report['steps'])
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    if report['source_changed']:
        print('Sources changed during verification; this is not a stable checkpoint gate.', flush=True)
    print(f'\n{"PASS" if report["success"] else "FAIL"} ({report["mode"]}/{args.stage}) in {report["seconds"]:.1f}s; {output / "report.json"}', flush=True)
    return 0 if report['success'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Verification failed: {error}', file=sys.stderr)
        sys.exit(1)
