# Local verification without repeated full sweeps

Use Python 3.11+ (`/opt/homebrew/bin/python3` on the development Mac) and the pinned Rust toolchain.
`scripts/verify.py` is the single definition of the gate: local `--full` and GitHub CI run the same steps.

## Development loop

Run focused checks for the packages actually changed:

```sh
python3 scripts/verify.py -p osrs-hd -p osrs-editor
```

This runs the Python guards, formatting, strict Clippy and selected package tests. Clippy already
type-checks that selection (examples included). This mode deliberately omits the WASM gate and is
**not full checkpoint verification**. For a small fix, use its existing test directly:

```sh
python3 scripts/run_tests.py -p osrs-hd --test integration themes::
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

Steps, cheapest first, stopping at the first failure:

1. `quick` stage (no compilation): the Python guards and `cargo fmt --all --check`.
2. `build` stage: strict native Clippy over the workspace, all targets and all features (this also
   type-checks every example and test target); one WASM Clippy command (`wasm32-unknown-unknown`,
   `-D warnings`) over `osrs-editor` and the portable crates it depends on; then the tests.

`--stage quick` or `--stage build` runs half of that (CI runs `quick` before restoring its Rust cache).
The separate `cargo check` commands that used to precede Clippy are gone: Clippy compiles
everything check does, so they only rebuilt the same code. Repair a failure and rerun the affected
check; broaden verification only when changed inputs justify it.

## What the tests run

`scripts/run_tests.py` builds `--lib --bins --tests` (all features) with `--no-run --message-format=json`
and executes every test artifact Cargo reports. It does not reuse old pass/fail results.

- **Examples are not test binaries.** They hold no `#[test]`s, and launching them as harnesses used to
  cost about 59 of the 123 binaries in a full run. Clippy `--all-targets` still type-checks them;
  `--examples` on the runner builds and launches them anyway.
- **One integration binary per crate.** `osrs-cache`, `osrs-core`, `osrs-hd`, `osrs-reference`,
  `osrs-render`, `osrs-scene` and `osrs-world` each declare `autotests = false` and a single
  `[[test]] name = "integration"` (`tests/integration.rs`), which includes the old files as modules
  with `#[path]`. Former `--test themes` is now `--test integration themes::`. New integration test
  files must be added there, or they will silently not run.
- **Cache-dependent tests are `#[ignore]`d**, not skipped at runtime. Run them with the pinned cache
  (`RUSTOSRS_TARGET_CACHE_DIR=~/RustOSRS-cache/b241/cache cargo test --locked -p osrs-world -- --ignored`). GPU tests skip themselves when no adapter exists.
  Ignored tests are not proof of acceptance.

Up to four test executables run at once (`--jobs`; this does not change Cargo's compile concurrency).
Library/integration targets of the GPU-capable editor/render/world packages share one execution slot.
Cargo package working directories, package environment and dynamic-library paths are retained. The
runner supports macOS/Linux; use ordinary Cargo elsewhere. Each binary has a 600-second timeout;
launch failures, timeouts and nonzero exits fail the run. Ctrl+C cancels queued work.

Reports under ignored `target/verify/` separate build time from per-binary execution time, retain
logs and counts, and record HEAD plus a hash of tracked/untracked source changes. Edits during a
run make it diagnostic rather than a stable checkpoint result. No report can certify independent
client parity or remote CI.

## Why binary count matters on macOS

In a 2026-10-18 full run (123 test binaries) libtest reported about 121 s of actual test time, while
the per-binary wall times summed to about 5,030 s. The difference is the first launch of each freshly
linked executable (dyld/Gatekeeper scanning, 20–80 s each under load); a second launch is instant.
Fewer test binaries is therefore the real lever, which is why examples are no longer launched and
integration tests are bundled. Enabling your terminal app under System Settings → Privacy & Security →
Developer Tools typically exempts its child processes from that scan; that is a security setting, so it is left
to you.

## Build settings

`[profile.dev]` uses `debug = "line-tables-only"` (readable backtraces) and no debug info for
dependencies; set `CARGO_PROFILE_DEV_DEBUG=2` when stepping through code in a debugger. CI sets
`CARGO_PROFILE_DEV_DEBUG=0`. Linux stable Rust 1.90+ already links x86_64 with `rust-lld`, so no mold
setup is needed.

## GitHub CI (`.github/workflows/ci.yml`)

One Ubuntu job, `timeout-minutes: 45`. Runs on pushes to `main` and on pull requests (not both for a
branch), skips Markdown-only changes, and cancels superseded runs. Order: toolchain → quick stage →
apt packages → rust-cache (only `main` saves) → build stage. No scheduled, matrix, macOS or Windows
jobs. Cache-dependent tests never run on GitHub because the pinned cache is not available there.
