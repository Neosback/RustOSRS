# RustOSRS contributor entry point

## Read before editing

1. Read `ARCHITECTURE.md` for ownership and dependency rules.
2. Read `DEV_HANDOFF.md`, then verify `git status --short`, the live branch, and `git rev-parse HEAD`.
3. Run `python3 scripts/features.py find "<concept>"` before creating an implementation, then use `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` to inspect the existing controller, planner, gateway and tests. Inspect callers before adding another abstraction.
4. Read the owning OSRS specification in `docs/specs/` and relevant accepted ADRs in `docs/adr/`.

`index.md` defines the documentation authority order. Chat history, research documents and archived handoffs cannot override current code or verified repository state. If status documents disagree with Git, reconcile the facts before implementation; never invent CI results or a remote PR state.

## Change boundaries

- `MapDocument` / `EditorSession` owns edited semantics. Commit, undo and redo go through `EditRuntime`; derived caches and worker mirrors never become editing authority.
- Plan pure semantic changes in `osrs-edit` against `MapQuery`. Use canonical `osrs-core` coordinates and IDs at boundaries.
- Put behavior on its owner with private state and narrow operations. `EditorApp` only composes owners and coordinates lifecycle/effects. Its explicit method/field surface is ratcheted by `scripts/editor_app_surface.json`.
- Preserve FIFO worker assumptions, dependency proofs and conservative rebuild fallbacks. GPU resources stay in rendering; UI code emits actions for product changes.
- Keep UI-local widget state in `UiRuntime`; preferences/key bindings in `SettingsRuntime`; diagnostics never decide whether an edit or worker result is valid.
- Search before creating a type, planner, history, worker message or cache. Reuse existing concepts and names. Do not introduce catch-all helper modules or broad mutable accessors.
- Stay on `main` unless the user explicitly asks for a branch. Work in focused, reviewable checkpoints and continue through an authorized sequence. Do not create worktrees or PRs by default. Publishing still requires authorization.
- New tools must update the feature registry with real planning, cancellation, commit and undo tests. When removing `EditorApp` behavior, remove its obsolete baseline entries; never widen the baseline just to bypass a guard.
- Temporary adapters must name their authoritative source, allowed callers, and removal condition. The terrain preview/history-routing adapter's condition is documented on `TerrainController` and in the feature map.

## Verification

Use the pinned Rust toolchain in `rust-toolchain.toml` and Python **3.11+**. On this Mac, `/opt/homebrew/bin/python3` satisfies that requirement; `/usr/bin/python3` does not provide `tomllib`.

During implementation, use focused checks (`python3 scripts/verify.py -p <changed-crate>`) or the
specific existing tests. Run the full gate once for a ready checkpoint, not after every small edit.
Do not overlap focused/full Cargo runs or repeat green suites without changed inputs.
`python3 scripts/verify.py --full` is the single definition of the gate; GitHub CI runs the same
script (`--stage quick`, then `--stage build`). See `docs/LOCAL-VERIFICATION.md`. In order, it runs:

```sh
# quick stage (no compilation)
python3 scripts/check_architecture.py          # plus the other Python guards listed in verify.py
cargo fmt --all --check
# build stage
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo clippy --locked --target wasm32-unknown-unknown -p osrs-editor -p osrs-core -p osrs-edit -p osrs-lab -p osrs-cache -p osrs-scene -p osrs-hd -p osrs-render -p osrs-world -- -D warnings
python3 scripts/run_tests.py                   # cargo test --lib --bins --tests, all features, bounded concurrency
```

Clippy replaces the separate `cargo check` commands (it compiles everything they do). Examples are
type-checked by Clippy but not launched as test binaries. Each crate's integration tests are bundled
into one `tests/integration.rs` binary: a new `tests/*.rs` file must be added as a module there.

The full workspace tests include the permanent M3–M9 parity suites (they run in CI's single `verify` job). External pinned-cache and independent visual-oracle tests retain their prerequisites; ignored tests are not proof of acceptance.

Update the current handoff when ownership changes. Report checks actually run, their results, the exact baseline SHA, and whether changes remain uncommitted. A local check does not certify remote CI or a later commit.
