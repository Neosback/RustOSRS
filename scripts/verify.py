#!/usr/bin/env python3
"""Focused development checks by default; --full runs every required local gate once."""
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
    'check_architecture.py', 'test_check_architecture.py', 'test_editor_architecture.py',
    'test_reference_regeneration.py', 'test_deob_golden_index.py', 'check_hd_sources.py',
    'test_hd_sources.py', 'test_audit_hd_config.py', 'test_run_tests.py',
]


def commands(full, packages, jobs, output):
    result = [(name, [sys.executable, f'scripts/{name}']) for name in PYTHON_CHECKS]
    result.append(('format', ['cargo', 'fmt', '--all', '--check']))
    selection = ['--workspace'] if full else [arg for package in packages for arg in ['-p', package]]
    if full:
        result.append(('native-check', ['cargo', 'check', '--locked', *selection, '--all-targets', '--all-features']))
    # Clippy type-checks the focused packages too; only full mode repeats the explicit check gate.
    result.append(('native-clippy', ['cargo', 'clippy', '--locked', *selection,
                                     '--all-targets', '--all-features', '--', '-D', 'warnings']))
    tests = [sys.executable, 'scripts/run_tests.py', '--jobs', str(jobs), '--output', str(output / 'tests')]
    if not full:
        tests.extend(selection)
    result.append(('tests', tests))
    if full:
        portable = ['osrs-core', 'osrs-edit', 'osrs-cache', 'osrs-scene', 'osrs-hd', 'osrs-render', 'osrs-world']
        result.append(('wasm-check', ['cargo', 'check', '--locked', '--target', 'wasm32-unknown-unknown',
                                     *[arg for package in portable for arg in ['-p', package]]]))
        result.append(('wasm-clippy', ['cargo', 'clippy', '--locked', '--target', 'wasm32-unknown-unknown',
                                      '-p', 'osrs-editor', '--', '-D', 'warnings']))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--full', action='store_true', help='All required gates; no package filtering')
    parser.add_argument('-p', '--package', action='append', help='Development package (repeatable)')
    parser.add_argument('-j', '--jobs', type=int, default=4, help='Concurrent test executables')
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
    report = {'version': 1, 'mode': 'full' if args.full else 'focused',
              'source_before': before, 'steps': [], 'success': False}
    for name, command in commands(args.full, args.package or [], args.jobs, output):
        step_start = time.monotonic()
        print(f'\n{name}: {" ".join(command)}', flush=True)
        env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1',
               'RUSTOSRS_VERIFY_PARENT': str(os.getpid())}
        code = subprocess.run(command, cwd=ROOT, env=env, check=False).returncode
        row = {'name': name, 'command': command, 'returncode': code,
               'seconds': round(time.monotonic() - step_start, 3)}
        report['steps'].append(row)
        print(f'{name}: {row["seconds"]:.1f}s', flush=True)
        if code:
            break
    report['source_after'] = source_identity()
    report['source_changed'] = report['source_after'] != before
    report['seconds'] = round(time.monotonic() - started, 3)
    report['success'] = not report['source_changed'] and all(step['returncode'] == 0 for step in report['steps'])
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    if report['source_changed']:
        print('Sources changed during verification; this is not a stable checkpoint gate.', flush=True)
    print(f'\n{"PASS" if report["success"] else "FAIL"} ({report["mode"]}) in {report["seconds"]:.1f}s; {output / "report.json"}', flush=True)
    return 0 if report['success'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Verification failed: {error}', file=sys.stderr)
        sys.exit(1)
