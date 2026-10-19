#!/usr/bin/env python3
"""Build Cargo's selected test inventory (lib, bin and integration tests), then run it concurrently.

Examples are not launched (they hold no tests); --examples adds them. No tests are cached or omitted. GPU-capable packages share one execution slot. Cargo builds
remain serial with other gate phases; --jobs controls test executables, not compilation.
"""
import argparse
from collections import deque
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor, wait, FIRST_COMPLETED
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading
import time

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
GPU_PACKAGES = {'osrs-render', 'osrs-world', 'osrs-editor'}
RESULT = re.compile(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;')


@contextmanager
def verification_lock():
    """One runner per checkout; nested test invocation inherits the gate parent's lock."""
    if os.environ.get('RUSTOSRS_VERIFY_PARENT') == str(os.getppid()):
        yield
        return
    if os.name != 'posix':
        raise ValueError('This runner supports macOS/Linux; use cargo test directly on this host')
    import fcntl
    directory = ROOT / 'target/verify'
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / 'runner.lock').open('a+') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError('Another verification runner is active in this checkout') from error
        lock.seek(0)
        lock.truncate()
        lock.write(str(os.getpid()))
        lock.flush()
        yield


@dataclass(frozen=True)
class TestBinary:
    package: str
    name: str
    executable: str
    cwd: str
    gpu: bool = False


class TestProcesses:
    def __init__(self):
        self.lock = threading.Lock()
        self.stopped = False
        self.children = set()

    def run(self, command, cwd, env, log, timeout):
        with self.lock:
            if self.stopped:
                return 130
            process = subprocess.Popen(command, cwd=cwd, env=env, stdout=log,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            self.children.add(process)
        try:
            try:
                return process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                log.write(f'\nTest executable timed out after {timeout}s\n'.encode())
                return 124
        finally:
            with self.lock:
                self.children.discard(process)

    def cancel(self):
        with self.lock:
            self.stopped = True
            for process in self.children:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass


def source_identity(root=ROOT):
    """Detect concurrent edits, including new files. Reports never contain source contents."""
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
    digest = hashlib.sha256(head.encode())
    digest.update(subprocess.check_output(
        ['git', 'diff', '--binary', 'HEAD', '--', '.', ':(exclude)**/__pycache__/**'], cwd=root))
    untracked = subprocess.check_output(
        ['git', 'ls-files', '--others', '--exclude-standard', '-z'], cwd=root)
    for name in sorted(untracked.split(b'\0')):
        if name and b'__pycache__' not in name.split(b'/'):
            path = root / os.fsdecode(name)
            digest.update(name + b'\0')
            digest.update(hashlib.sha256(path.read_bytes()).digest())
    return {'head': head, 'working_tree_sha256': digest.hexdigest()}


def selected_binaries(messages, packages):
    binaries = {}
    for row in messages:
        if row.get('reason') != 'compiler-artifact' or not row.get('executable'):
            continue
        if not row['profile']['test']:
            continue
        package = packages[row['package_id']]
        binary = TestBinary(package['name'], row['target']['name'], row['executable'],
                            str(Path(row['manifest_path']).parent),
                            package['name'] in GPU_PACKAGES and 'example' not in row['target']['kind'])
        binaries[binary.executable] = binary
    return sorted(binaries.values(), key=lambda b: (b.package, b.name, b.executable))


def runtime_env(binary, packages, search_paths, target_libdir):
    package = next(p for p in packages.values() if p['name'] == binary.package)
    env = os.environ.copy()
    version = package['version'].split('-', 1)
    parts = version[0].split('.')
    env.update({
        'CARGO_MANIFEST_DIR': binary.cwd,
        'CARGO_MANIFEST_PATH': package['manifest_path'],
        'CARGO_PKG_NAME': package['name'],
        'CARGO_PKG_VERSION': package['version'],
        'CARGO_PKG_VERSION_MAJOR': parts[0],
        'CARGO_PKG_VERSION_MINOR': parts[1],
        'CARGO_PKG_VERSION_PATCH': parts[2],
        'CARGO_PKG_VERSION_PRE': version[1] if len(version) > 1 else '',
        'CARGO_PKG_AUTHORS': ':'.join(package.get('authors', [])),
    })
    for key in ['description', 'homepage', 'repository', 'license', 'license_file', 'readme', 'rust_version']:
        env['CARGO_PKG_' + key.upper()] = package.get(key) or ''
    key = 'DYLD_FALLBACK_LIBRARY_PATH' if sys.platform == 'darwin' else 'LD_LIBRARY_PATH'
    folder = Path(binary.executable).parent
    profile = folder.parent if folder.name in {'deps', 'examples'} else folder
    paths = [*search_paths, str(folder), str(profile / 'deps'), str(profile), target_libdir]
    if sys.platform == 'win32':
        key = 'PATH'
    if env.get(key):
        paths.extend(env[key].split(os.pathsep))
    elif sys.platform == 'darwin':
        paths.extend([str(Path.home() / 'lib'), '/usr/local/lib', '/usr/lib'])
    env[key] = os.pathsep.join(dict.fromkeys(paths))
    return env


def execute_binary(binary, log_path, env, gpu_slot, test_threads=None, processes=None, timeout=600):
    command = [binary.executable]
    if test_threads is not None:
        command.append(f'--test-threads={test_threads}')
    slot = gpu_slot if binary.gpu else threading.Lock()
    with slot:
        started = time.monotonic()
        with log_path.open('wb') as log:
            try:
                code = (processes or TestProcesses()).run(command, binary.cwd, env, log, timeout)
            except OSError as error:
                log.write(str(error).encode())
                code = 127
        counts = [tuple(map(int, row)) for row in RESULT.findall(log_path.read_text(errors='replace'))]
        return {
            'package': binary.package, 'target': binary.name, 'executable': binary.executable,
            'returncode': code, 'seconds': round(time.monotonic() - started, 3),
            'passed': sum(r[0] for r in counts), 'failed': sum(r[1] for r in counts),
            'ignored': sum(r[2] for r in counts), 'log': str(log_path),
        }


def run(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    before = source_identity()
    started = time.monotonic()
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--format-version=1', '--no-deps'], cwd=ROOT))
    packages = {p['id']: p for p in metadata['packages']}
    command = ['cargo', 'test', '--locked', '--all-features', '--no-run', '--message-format=json']
    if args.package:
        for package in args.package:
            command.extend(['-p', package])
    else:
        command.append('--workspace')
    if args.test:
        for target in args.test:
            command.extend(['--test', target])
    elif args.lib:
        command.append('--lib')
    elif args.examples:
        command.append('--all-targets')
    else:
        # Examples are tool binaries with no tests; clippy --all-targets already type-checks them.
        command.extend(['--lib', '--bins', '--tests'])
    print('Build:', ' '.join(command), flush=True)
    with (output / 'build.jsonl').open('w') as log:
        build = subprocess.run(command, cwd=ROOT, stdout=log, check=False)
    if build.returncode:
        return build.returncode
    messages = [json.loads(row) for row in (output / 'build.jsonl').read_text().splitlines() if row.strip()]
    binaries = selected_binaries(messages, packages)
    if not binaries:
        raise ValueError('Cargo produced no test executables; refusing to report success')
    search_paths = []
    for row in messages:
        if row.get('reason') == 'build-script-executed':
            for path in row.get('linked_paths', []):
                search_paths.append(path.split('=', 1)[-1])
    target_libdir = subprocess.check_output(['rustc', '--print', 'target-libdir'], cwd=ROOT).decode().strip()
    built = time.monotonic()
    report = {
        'version': 1, 'scope': 'focused' if args.package or args.test or args.lib else 'workspace',
        'command': command, 'source_before': before, 'test_jobs': args.jobs,
        'build_seconds': round(built - started, 3), 'binaries': [],
    }
    print(f'Run: {len(binaries)} executables; {args.jobs} slots; GPU packages serialized', flush=True)
    gpu_slot = threading.Lock()
    processes = TestProcesses()
    with ThreadPoolExecutor(max_workers=args.jobs) as executor:
        cpu = deque(b for b in binaries if not b.gpu)
        gpu = deque(b for b in binaries if b.gpu)
        pending = {}
        def schedule():
            # Keep one GPU executable alongside CPU work without blocked GPU jobs occupying slots.
            gpu_active = any(binary.gpu for binary in pending.values())
            while len(pending) < args.jobs:
                if gpu and not gpu_active:
                    binary = gpu.popleft()
                    gpu_active = True
                elif cpu:
                    binary = cpu.popleft()
                else:
                    break
                index = binaries.index(binary)
                future = executor.submit(
                    execute_binary, binary, output / f'{index:03}-{binary.package}-{binary.name}.log',
                    runtime_env(binary, packages, search_paths, target_libdir), gpu_slot,
                    args.test_threads, processes, args.timeout)
                pending[future] = binary
        schedule()
        try:
            while pending:
                completed, _ = wait(pending, timeout=15, return_when=FIRST_COMPLETED)
                if not completed:
                    print(f'Waiting: {len(report["binaries"])}/{len(binaries)} complete, '
                          f'{time.monotonic() - built:.0f}s elapsed', flush=True)
                for future in completed:
                    del pending[future]
                    row = future.result()
                    report['binaries'].append(row)
                    status = 'PASS' if row['returncode'] == 0 else 'FAIL'
                    print(f"[{len(report['binaries'])}/{len(binaries)}] {row['package']}::{row['target']} "
                          f"{status} {row['seconds']:.2f}s ({row['passed']} passed, {row['ignored']} ignored)", flush=True)
                    if row['returncode']:
                        print(Path(row['log']).read_text(errors='replace')[-6000:], flush=True)
                schedule()
        except BaseException:
            processes.cancel()
            for future in pending:
                future.cancel()
            raise
    report['source_after'] = source_identity()
    report['source_changed'] = report['source_after'] != before
    report['test_seconds'] = round(time.monotonic() - built, 3)
    report['total_seconds'] = round(time.monotonic() - started, 3)
    report['passed'] = sum(r['passed'] for r in report['binaries'])
    report['failed'] = sum(r['failed'] for r in report['binaries'])
    report['ignored'] = sum(r['ignored'] for r in report['binaries'])
    report['success'] = not report['source_changed'] and all(
        r['returncode'] == 0 and r['failed'] == 0 for r in report['binaries'])
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f"{report['passed']} passed; {report['failed']} failed; {report['ignored']} ignored; "
          f"build {report['build_seconds']:.1f}s + tests {report['test_seconds']:.1f}s", flush=True)
    print(f'Report: {output / "report.json"}', flush=True)
    if report['source_changed']:
        print('Sources changed during this run. Results are diagnostic, not a stable checkpoint gate.', flush=True)
    return 0 if report['success'] else 1


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('-p', '--package', action='append', help='Focus on this package (repeatable)')
    targets = parser.add_mutually_exclusive_group()
    targets.add_argument('--test', action='append', help='Focus on this integration target (repeatable)')
    targets.add_argument('--lib', action='store_true', help='Focus on unit tests')
    targets.add_argument('--examples', action='store_true',
                         help='Also build and launch example harnesses (all targets; they hold no tests)')
    parser.add_argument('-j', '--jobs', type=int, default=4, help='Concurrent test executables (default: 4)')
    parser.add_argument('--test-threads', type=int, help='Override libtest threads per executable')
    parser.add_argument('--timeout', type=float, default=600, help='Seconds per test executable (default: 600)')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/verify' / time.strftime('%Y%m%d-%H%M%S'))
    args = parser.parse_args(argv)
    if args.jobs < 1 or args.test_threads is not None and args.test_threads < 1:
        parser.error('Concurrency must be positive')
    if args.timeout <= 0:
        parser.error('Timeout must be positive')
    try:
        with verification_lock():
            return run(args)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Test runner failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print('Test run cancelled; no success report was recorded.', file=sys.stderr)
        sys.exit(130)
