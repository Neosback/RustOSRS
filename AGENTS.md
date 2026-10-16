# RustOSRS contributor entry point

## Read before editing

1. Read `ARCHITECTURE.md` for ownership and dependency rules.
2. Read `DEV_HANDOFF.md`, then verify `git status --short`, the live branch, and `git rev-parse HEAD`.
3. Use `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` to find the existing controller, planner, gateway and tests. Inspect callers before adding another abstraction.
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

```sh
python3 scripts/check_architecture.py
python3 scripts/test_check_architecture.py
python3 scripts/test_editor_architecture.py
python3 scripts/test_reference_regeneration.py
python3 scripts/test_deob_golden_index.py
python3 scripts/test_performance.py
python3 scripts/test_performance_baseline.py
python3 scripts/test_performance_rank.py
cargo fmt --all --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo check --locked --target wasm32-unknown-unknown -p osrs-core -p osrs-edit -p osrs-cache -p osrs-scene -p osrs-hd -p osrs-render -p osrs-world
cargo clippy --locked --target wasm32-unknown-unknown -p osrs-editor -- -D warnings
```

The full workspace tests include the permanent M3–M9 parity suites. `.github/workflows/ci.yml` separately names their Tier C gates. External pinned-cache and independent visual-oracle tests retain their prerequisites; ignored tests are not proof of acceptance.

Update the current handoff when ownership changes. Report checks actually run, their results, the exact baseline SHA, and whether changes remain uncommitted. A local check does not certify remote CI or a later commit.