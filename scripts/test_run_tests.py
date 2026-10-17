#!/usr/bin/env python3
"""Test runner coverage and failure semantics with small subprocess fixtures."""
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import unittest

sys.dont_write_bytecode = True
from run_tests import TestBinary, TestProcesses, execute_binary, runtime_env, selected_binaries, source_identity
from verify import commands


class RunnerTests(unittest.TestCase):
    def package(self, root):
        return {'id': 'id', 'name': 'sample', 'version': '1.2.3-beta', 'authors': ['A', 'B'],
                'manifest_path': str(root / 'Cargo.toml')}

    def test_inventory_keeps_example_harnesses_and_deduplicates_executables(self):
        package = self.package(Path('/project'))
        def artifact(path, test=True, target_test=True):
            return {'reason': 'compiler-artifact', 'package_id': 'id', 'executable': path,
                    'manifest_path': '/project/Cargo.toml', 'profile': {'test': test},
                    'target': {'name': 'demo', 'test': target_test, 'kind': ['example'] if not target_test else ['test']}}
        rows = [artifact('/unit'), artifact('/unit'), artifact('/example', target_test=False),
                artifact('/normal', test=False), {'reason': 'build-finished'}]
        binaries = selected_binaries(rows, {'id': package})
        self.assertEqual([b.executable for b in binaries], ['/example', '/unit'])

    def test_examples_do_not_take_the_gpu_slot(self):
        package = {**self.package(Path('/project')), 'name': 'osrs-world'}
        def row(path, kind):
            return {'reason': 'compiler-artifact', 'package_id': 'id', 'executable': path,
                    'manifest_path': '/project/Cargo.toml', 'profile': {'test': True},
                    'target': {'name': path, 'kind': [kind]}}
        binaries = selected_binaries([row('/example', 'example'), row('/gpu', 'test')], {'id': package})
        self.assertEqual([(b.name, b.gpu) for b in binaries], [('/example', False), ('/gpu', True)])

    def test_failures_and_ignored_counts_are_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            script = root / 'failing-test'
            script.write_text(f'#!{sys.executable}\nprint("test result: FAILED. 2 passed; 1 failed; 3 ignored;")\nraise SystemExit(101)\n')
            script.chmod(0o755)
            binary = TestBinary('sample', 'failing', str(script), temp)
            row = execute_binary(binary, root / 'log', os.environ.copy(), threading.Lock())
            self.assertEqual((row['returncode'], row['passed'], row['failed'], row['ignored']), (101, 2, 1, 3))

    def test_missing_executable_is_a_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            row = execute_binary(TestBinary('sample', 'missing', temp + '/missing', temp),
                                 Path(temp) / 'log', os.environ.copy(), threading.Lock())
            self.assertEqual(row['returncode'], 127)

    def test_cancelled_queue_does_not_start_another_executable(self):
        processes = TestProcesses()
        processes.cancel()
        with tempfile.TemporaryDirectory() as temp:
            row = execute_binary(TestBinary('sample', 'cancelled', '/never-run', temp),
                                 Path(temp) / 'log', os.environ.copy(), threading.Lock(),
                                 processes=processes)
            self.assertEqual(row['returncode'], 130)

    def test_runtime_environment_keeps_cargo_package_and_library_context(self):
        package = self.package(Path('/project'))
        env = runtime_env(TestBinary('sample', 'unit', '/target/debug/deps/unit', '/project'),
                          {'id': package}, ['/native/lib'], '/toolchain/lib')
        self.assertEqual(env['CARGO_MANIFEST_DIR'], '/project')
        self.assertEqual(env['CARGO_PKG_VERSION_PRE'], 'beta')
        self.assertEqual(env['CARGO_PKG_AUTHORS'], 'A:B')
        key = 'DYLD_FALLBACK_LIBRARY_PATH' if sys.platform == 'darwin' else 'LD_LIBRARY_PATH'
        if sys.platform == 'win32':
            key = 'PATH'
        self.assertIn('/native/lib', env[key])

    def test_full_gate_retains_every_required_scope_once(self):
        steps = dict(commands(True, [], 4, Path('/output')))
        self.assertNotIn('native-check', steps)
        self.assertIn('--workspace', steps['native-clippy'])
        self.assertIn('wasm-check', steps)
        self.assertIn('wasm-clippy', steps)
        self.assertNotIn('-p', steps['tests'])
        self.assertEqual(sum(name == 'tests' for name, _ in commands(True, [], 4, Path('/output'))), 1)
        focused = dict(commands(False, ['osrs-hd'], 4, Path('/output')))
        self.assertNotIn('native-check', focused)
        self.assertNotIn('wasm-clippy', focused)
        self.assertIn('osrs-hd', focused['tests'])

    def test_source_identity_changes_for_untracked_and_tracked_edits(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            (root / 'source').write_text('first')
            subprocess.run(['git', 'add', '.'], cwd=root, check=True)
            subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                            '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture'], cwd=root, check=True)
            first = source_identity(root)
            (root / 'new-source').write_text('new')
            self.assertNotEqual(source_identity(root), first)
            (root / 'new-source').unlink()
            self.assertEqual(source_identity(root), first)
            (root / 'source').write_text('second')
            self.assertNotEqual(source_identity(root), first)


if __name__ == '__main__':
    unittest.main()
