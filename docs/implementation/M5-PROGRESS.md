# M5 Progress: Reference Fixture Infrastructure

Status: **IN PROGRESS**  
Milestone: `M5 - Reference fixture infrastructure`  
Branch: `impl/m5-reference-fixture-infrastructure`  
Baseline: M4 squash merge `4d3d21dbe449cd345cf46ccbab651bec6366d585`

## Milestone purpose

M5 turns `osrs-reference` into a real development/test-only reference fixture system before scene, normals, lighting, and other high-risk semantic work depends on differential evidence.

Production crates must not depend on `osrs-reference`. Ordinary Rust verification must remain offline and deterministic. Source/oracle execution and fixture regeneration are separate development workflows and must never silently rewrite checked-in expected outputs during CI.

## Checkpoint plan

1. **Fixture loader/comparator/provenance foundation** - COMPLETE
2. Canonical normalized fixture schemas and first migrated source-pinned fixtures - NOT STARTED
3. Exact fixture inventory/runner integration and dedicated M5 CI gates - NOT STARTED
4. Explicit regeneration command and isolated oracle/harness adapters - NOT STARTED
5. Historical `deob_golden.txt` indexing/migration plus additional priority semantic families - NOT STARTED
6. M5 verification closure, parity links, exit audit, and milestone PR - NOT STARTED

The later checkpoint boundaries may be refined if source evidence requires it, but Checkpoint 1 intentionally does not pull those later responsibilities forward.

---

## Checkpoint 1 - fixture loader/comparator/provenance foundation

Status: **COMPLETE**

Implementation validation head: `2c4455a99c4f6bbc76e964d243f2f0b1244596fe`  
CI workflow: `37890081413`

### Scope completed

Checkpoint 1 hardens the M0-era manifest parser into an offline integrity boundary suitable for later M5 fixture families.

Permanent implementation:

- `crates/osrs-reference/src/fixture.rs`
  - validates the supported manifest schema version;
  - requires a non-empty fixture ID;
  - requires at least one non-empty canonical owned spec;
  - rejects duplicate owned specs;
  - validates oracle kind;
  - requires `oracle.repository` and `oracle.commit` to appear together;
  - validates commit IDs as exact 40-character hexadecimal identities;
  - allows blob-pinned oracle files and validates each blob as an exact 40-character hexadecimal identity;
  - rejects duplicate oracle source paths;
  - requires exact oracle provenance through either a pinned commit or blob-pinned source file(s);
  - validates non-empty harness path/revision;
  - validates non-empty input/expected paths;
  - validates `expected_sha256` as an exact 64-character hexadecimal identity.

- `crates/osrs-reference/src/loader.rs`
  - introduces `FixtureRepository` rooted at a canonical fixture directory;
  - resolves manifest, input, and expected files only through root-relative paths;
  - rejects parent, rooted, or platform-prefix traversal before filesystem access;
  - canonicalizes resolved files and rejects symlink/path escape outside the fixture root;
  - loads fixture input and expected output without network access;
  - verifies the exact expected-output SHA-256 before the expected bytes are admitted for comparison;
  - exposes a `LoadedFixture` containing the verified manifest and bytes;
  - reports typed path, I/O, manifest, and expected-hash failures.

- `crates/osrs-reference/src/comparator.rs`
  - introduces exact byte-for-byte comparison;
  - performs no hidden normalization, sorting, tolerance, or absent/present coercion;
  - reports the first differing byte plus both lengths and optional expected/actual bytes;
  - treats length-only differences as exact failures at the common boundary.

- `crates/osrs-reference/src/lib.rs`
  - exposes the manifest, loader, and comparator modules while preserving the development-only crate boundary.

- `crates/osrs-reference/Cargo.toml` / `Cargo.lock`
  - reuse the workspace-pinned `sha2 0.10` dependency for expected-output integrity verification;
  - no dependency version or external package was changed by this checkpoint.

### Exact comparison policy

Checkpoint 1 deliberately provides a raw exact-byte comparator rather than inventing semantic normalization rules.

Later normalized fixture schemas may serialize canonical semantic structures before comparison, but any such normalization must be explicit and fixture-owned. The comparator itself cannot hide:

- ordering differences;
- missing versus present values;
- signedness or integer-value differences;
- extra/truncated output;
- whitespace or serialization differences unless the fixture-producing normalization explicitly owns them before this boundary.

### Filesystem safety policy

Fixture manifests are untrusted development inputs at the loader boundary.

A fixture path must:

1. be non-empty;
2. be relative to the configured fixture root;
3. contain no `..`, root, or platform-prefix component;
4. exist and canonicalize successfully;
5. remain inside the canonical fixture root after resolution.

This prevents both direct traversal and symlink escape from silently reading unrelated files.

### Provenance policy established

A manifest cannot become a trusted reference fixture merely because it parses as YAML.

Checkpoint 1 requires exact oracle identity through:

- a paired repository + 40-hex commit pin; or
- one or more source files with exact 40-hex blob identities.

When source files are listed, duplicate source paths are rejected. The harness path/revision and expected-output SHA-256 are also mandatory identities.

This is schema/integrity validation only. Checkpoint 1 does **not** contact GitHub, execute Java/deob code, or independently verify that a declared commit/blob exists upstream. Oracle execution and regeneration belong to later M5 checkpoints.

### Tests added/expanded

Manifest tests cover:

- valid source-pinned manifest;
- unsupported schema version;
- empty owned-spec set;
- missing exact oracle provenance;
- unpaired repository/commit identity;
- invalid blob identity;
- malformed YAML.

Comparator tests cover:

- exact equality;
- first content mismatch;
- length-only mismatch.

Loader tests cover:

- hash-verified offline fixture loading;
- exact expected/actual comparison;
- rejection of expected-output hash drift;
- rejection of parent traversal before file access.

### Validation

Implementation head `2c4455a99c4f6bbc76e964d243f2f0b1244596fe` passed the normal CI chain in workflow `37890081413`:

- Tier A static quality: PASS
  - architecture dependency boundary
  - architecture guard tests
  - rustfmt
  - locked workspace check
  - strict clippy with warnings denied
- Tier B workspace tests: PASS
- Tier C existing M3/M4 semantic parity: PASS

The checkpoint introduced no CI network dependency and no regeneration behavior.

### Checkpoint 1 diff against M4 baseline

Before this progress document, the implementation head was 10 commits ahead and 0 behind M4 `main` and changed only:

1. `Cargo.lock`
2. `crates/osrs-reference/Cargo.toml`
3. `crates/osrs-reference/src/comparator.rs`
4. `crates/osrs-reference/src/fixture.rs`
5. `crates/osrs-reference/src/lib.rs`
6. `crates/osrs-reference/src/loader.rs`

No `osrs-core`, `osrs-cache`, `osrs-scene`, renderer, editor, runtime semantic, or existing fixture files were modified.

### Explicit non-goals / deferred work

Checkpoint 1 does not implement:

- canonical typed JSON input/output schemas for specific semantic fixture families;
- migration of M4 synthetic inventory entries into full M5 provenance manifests;
- a repository-wide fixture index or exact runner over checked-in fixture families;
- new Tier C M5 fixture commands;
- regeneration commands;
- Java/deob/reference oracle adapters;
- local absolute-path cleanup inside existing deob harness tooling;
- historical `deob_golden.txt` provenance reconstruction or migration;
- normal, lighting, placement, plane, or priority semantics themselves;
- parity-matrix promotion for later semantic rows.

Those remain later M5 work.

---

## Current milestone boundary

M5 Checkpoint 1 is complete once the documentation-complete branch head passes the normal Tier A/B/C chain and the final branch-vs-M4 scope audit confirms no unrelated changes.

No M5 pull request should be opened until the milestone exit checkpoint.
