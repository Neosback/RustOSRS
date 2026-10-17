# Local verification without repeated full sweeps

Use Python 3.11+ (`/opt/homebrew/bin/python3` on the development Mac) and the pinned Rust toolchain.

## Development loop

Run focused checks for the packages actually changed:

```sh
python3 scripts/verify.py -p osrs-hd -p osrs-editor
```

This runs the Python guards, formatting, strict Clippy and selected package tests. Clippy already
type-checks that selection. This mode deliberately omits the separate workspace check and WASM
gates and is **not full checkpoint verification**. For a small fix, use its existing test directly:

```sh
python3 scripts/run_tests.py -p osrs-hd --test themes
python3 scripts/run_tests.py -p osrs-editor --lib
```

Do not start a full sweep after each small edit, rerun green suites without a relevant change,
or overlap focused/full Cargo builds. Differently selected package feature graphs can also rebuild
shared dependencies. A checkout-wide runner lock rejects overlapping runner invocations; direct
Cargo commands still need coordination with other sessions.

## Checkpoint gate

Once a checkpoint is ready, run all required gates once:

```sh
python3 scripts/verify.py --full
```

This retains strict native Clippy (which type-checks all targets), full workspace/all-target/all-feature
tests and both WASM commands, plus Python guards and formatting. A failure stops the gate. Repair
the failure and rerun the affected check; broaden verification only when changed inputs justify it.

## Parallel test execution

`scripts/run_tests.py` asks Cargo to build the selected tests with `--no-run --message-format=json`,
then executes **every test artifact Cargo reports**, including example harnesses. It does not reuse
old pass/fail results. Four independent binaries may run at once; `--jobs 1` selects serial execution.
`--jobs` does not change Cargo's compile concurrency. Library/integration targets of GPU-capable
editor/render/world packages share one execution slot; their example harnesses can run alongside
CPU work. Ordinary libtest filtering of ignored
tests remains unchanged; this runner does not accept missing external prerequisites as acceptance.

Cargo package working directories, package environment and dynamic-library paths are retained.
The runner currently supports macOS/Linux. Use ordinary Cargo on other hosts. Each binary has a
600-second timeout (overridable); launch failures, timeouts and nonzero exits fail the run. Ctrl+C
cancels queued work and terminates active test process groups.

Reports under ignored `target/verify/` separate build time from per-binary execution time, retain
logs and counts, and record HEAD plus a hash of tracked/untracked source changes. Edits during a
run make it diagnostic rather than a stable checkpoint result. Generated `__pycache__` files are
excluded from that source identity. No report can certify independent client parity or remote CI.

On the development Mac, samples taken on 2026-10-16 caught test executables waiting in dyld before
Rust started, with negligible process CPU time. The serial workspace sweep had already exceeded
30 minutes. Bounded concurrency amortizes independent launch waits; it does not change macOS
security settings or establish the cause of the loader delay.
