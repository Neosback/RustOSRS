# CI, Property, Fuzz, Failure-Injection, and Benchmark Policy

Status: **Checkpoint 7 verification plan**

This document defines the intended local/CI verification tiers for the future Rust workspace.

The commands are conceptual until implementation exists, but the ownership and merge-gate rules are normative.

## 1. Test runner baseline

Preferred implementation-era tooling:

- `cargo nextest` for normal Rust test execution;
- `proptest` for invariant/state-machine properties;
- `cargo fuzz` / libFuzzer for parser robustness;
- Criterion or equivalent for benchmarks;
- `tracing` spans/metrics for stage attribution;
- a dedicated image comparison tool for P3 goldens;
- wgpu validation enabled in test/dev reference runs where practical.

No tool is semantic authority. A future replacement is allowed if it preserves these contracts.

## 2. CI tiers

### Tier A: formatting/static analysis

Runs on every change.

Future examples:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

Also validate documentation/spec registry consistency where tooling exists.

### Tier B: fast exact unit tests

Runs on every change.

Includes:

- decoder unit tests;
- pure integer math;
- model transformations;
- terrain topology;
- morph selection;
- command apply/revert;
- small fixture tests;
- renderer extraction logic that does not require a GPU.

Target: deterministic and fast enough for ordinary development.

### Tier C: full semantic/differential suite

Merge-gating.

Includes:

- all checked-in source-derived fixtures;
- normals/lighting;
- bridge scenes;
- priority/alpha exact order;
- composed semantic golden scenes;
- randomized history/state-machine suites with bounded seeds/cases;
- project serialization/migration tests.

Ordinary execution consumes checked-in fixtures and does not require Java.

### Tier D: renderer structural/headless suite

Merge-gating once renderer work begins.

Includes:

- extraction and zone compilation;
- static/dynamic/ordered classification;
- material paging;
- stale-generation rejection;
- picking ID mapping;
- render-graph construction;
- device/resource rebuild logic;
- wgpu validation on supported test adapters/backends.

Where software/headless adapters are insufficient, separate logic-level tests from canonical GPU tests.

### Tier E: reference GPU visual suite

Merge-gating for renderer changes on designated canonical runner(s), not necessarily every developer platform.

Includes:

- deterministic Reference profile;
- fixed golden scenes/cameras;
- image tolerance checks;
- reverse-Z/culling/bias;
- alpha/priority composition;
- textures/animation fixed ticks.

The canonical runner records adapter/backend/driver metadata.

Noncanonical platforms may run informational visual tests without being allowed to rewrite expected images.

### Tier F: extended robustness

Scheduled/nightly or pre-release where expensive.

Includes:

- long property/state-machine runs;
- fuzz corpus execution;
- sanitizer/Miri-compatible subsets where practical;
- large project save/recovery tests;
- multi-region stress;
- repeated GPU recreate/device-loss simulations;
- broader backend matrix;
- performance benchmarks.

## 3. Merge gates by changed area

A future CI classifier may avoid running the most expensive unrelated work, but minimum ownership rules remain:

| Changed area | Minimum required tiers |
|---|---|
| `osrs-cache` decode | A, B, C + relevant fuzz smoke |
| `osrs-core` semantic math/model | A, B, C |
| `osrs-scene` placement/planes/normals | A, B, C |
| `osrs-render` extraction only | A-D |
| `osrs-render` shader/raster/ordering | A-E |
| `osrs-editor` command/document | A-C + editor V5 suites |
| project persistence/export | A-C + failure injection |
| source fixtures/goldens | full owning differential/golden suite |
| shared architecture/public semantic API | all materially affected tiers |

A path-based optimization must never skip the actual owning parity suite.

## 4. Property test catalog

### Decoder properties

- arbitrary byte input never triggers unchecked huge allocation from a malformed count;
- decode errors retain archive/file/definition identity where available;
- valid length/count relationships are checked before indexing;
- unsupported revision opcode fails explicitly rather than silently desynchronizing the buffer.

### Model properties

- every face index remains within vertex count after combine/transform/mirror;
- mirroring twice restores geometry/winding where the operation is involutive for the tested representation;
- recolor/retexture never changes geometry;
- instance mutation never changes the shared source model hash;
- transform operations do not change face count unless the owning operation explicitly allows it.

### Terrain properties

- every generated face references an existing generated vertex;
- every shape `0..12` and rotation produces deterministic output;
- rotation values normalize only according to the owning contract;
- no terrain edit can create an implicit fractional semantic height.

### Plane/scene properties

- link-below preserves exactly one linked-below identity;
- game-object plane decrements only under the audited qualification;
- renderer filtering does not mutate source/storage/collision plane values;
- loc footprint anchor remains stable through render rebuild.

### Renderer properties

- semantic texture ID mapping is stable/unique inside one extraction generation;
- stale generation artifacts cannot become current;
- zone dirty expansion includes every zone touched by an object's semantic footprint;
- renderer recreation from the same extraction yields equivalent structural artifacts modulo nonsemantic GPU handles.

### Editor state-machine properties

Generated valid command sequences should verify:

```text
baseline
  -> apply commands
  -> final hash A
  -> undo all
  -> baseline hash
  -> redo all
  -> final hash A
```

Add periodic project serialize/reload boundaries and require the semantic hash to remain equivalent.

## 5. Fuzz target catalog

### `fuzz_model_decode`

Targets model binary format parsers.

Assertions:

- no panic/UB;
- bounded allocations;
- explicit errors;
- no out-of-bounds face/texture indexing accepted as valid output.

### `fuzz_object_definition_decode`

Covers opcode streams, transform tables, model IDs, recolor/retexture lists, sizes, offsets, animation/morph fields.

### `fuzz_floor_definition_decode`

Underlay/overlay definitions, RGB/HSL post-decode boundaries.

### `fuzz_texture_definition_decode`

Texture/material references and animation metadata.

### `fuzz_map_terrain_decode`

Tile heights/settings/underlay/overlay/shape/rotation input.

### `fuzz_loc_stream_decode`

Delta/smart encoded object placements and coordinates.

### `fuzz_project_file`

Project schema parser, unknown fields/versions, truncated/corrupt recovery artifacts.

### `fuzz_export_serializer_state`

Feeds structurally valid but adversarial semantic documents into export prevalidation/serialization and asserts explicit rejection instead of corruption/panic.

## 6. Fuzz corpus policy

- checked-in corpus cases should be small;
- every fixed fuzz bug becomes a regression test when practical;
- seed corpora may include synthetic valid examples from reference fixtures;
- do not commit proprietary cache archives as fuzz corpora;
- crashes retain minimized reproducer inputs as test assets when redistribution is appropriate.

## 7. Failure-injection catalog

### Persistence

Inject:

- temp-file create failure;
- partial/write failure;
- flush failure;
- rename/replace failure;
- stale autosave completion;
- corrupt recovery file;
- schema migration error.

Assert:

- previous explicit save stays intact;
- dirty generation remains correct;
- no failed write is reported as saved;
- recovery remains separable from explicit save.

### Concurrency

Use deterministic barriers to force completion order such as:

```text
start generation N task
start generation N+1 task
finish N+1
finish N
```

Assert generation N is discarded.

Apply to scene builds, zone compiles, validation, autosave, picks, thumbnails, and search/index work.

### Renderer lifecycle

Simulate or abstract:

- resource cache drop;
- pipeline recreation;
- texture page recreation;
- viewport resize/reconfigure;
- device/backend initialization failure;
- adapter capability missing.

Assert semantic/project state is unaffected and diagnostics are explicit.

## 8. Wgpu validation strategy

Development/reference runs should enable available wgpu validation/debug behavior.

Tests should explicitly catch:

- bind-layout mismatch;
- invalid vertex/index ranges;
- buffer bounds errors;
- texture array capability/limit failures;
- sample-count/depth attachment mismatches;
- stale resource use after generation/device recreation.

Do not suppress validation errors to make a renderer test pass.

## 9. Golden-image execution

Canonical visual runner requirements:

- fixed viewport dimensions;
- deterministic Reference profile;
- fixed render tick;
- no editor UI composited into world image unless the test owns UI;
- fixed backend where possible;
- recorded adapter/vendor/device/driver metadata;
- artifact upload on failure.

A second backend may be run informationally to discover portability issues.

## 10. Image comparison output

On failure, comparison tooling should emit:

- expected PNG;
- actual PNG;
- absolute/amplified diff PNG;
- JSON metric summary;
- count/fraction of changed pixels;
- max and percentile channel error;
- scene/camera/profile manifest;
- renderer diagnostic metadata.

Thresholds are calibrated after the reference renderer exists. Until calibration, the blueprint intentionally does not invent a numeric tolerance.

## 11. Project schema migration verification

Every persistent project schema version after v1 must retain fixtures for migration from supported earlier versions.

Tests require:

- old fixture opens;
- migration output is deterministic;
- semantic document hash matches expected post-migration state;
- unknown future version is rejected safely;
- migration never rewrites the source cache.

If backward support is dropped, the supported-version policy must be documented and migration tooling considered.

## 12. Export verification

Once export exists, each supported target profile needs:

- semantic prevalidation fixture;
- output decode/round-trip test where possible;
- exact target-format field/record checks;
- explicit unsupported-state failures;
- project save state unchanged by export success/failure.

Round-trip does not by itself prove semantic correctness if the same buggy encoder/decoder share an error, so source/reference fixtures remain necessary.

## 13. Benchmark catalog

### Decode

- representative model set;
- object/floor/texture definitions;
- map + loc region decode.

### Semantic construction

- model build/transform;
- normal generation;
- cross-model normal reconciliation;
- full region scene materialization;
- morph/dynamic preview update.

### Renderer CPU

- render extraction;
- priority sorting;
- 8x8 zone compilation;
- dirty-zone rebuild;
- material table/page assignment;
- frame preparation.

### Renderer GPU

When stable measurement infrastructure exists:

- representative static scene frame;
- alpha/priority-heavy scene;
- diagnostic overlays off/on;
- Reference vs Enhanced profiles.

### Editor

- large terrain transaction;
- bulk loc transaction;
- undo/redo large change;
- project save/autosave serialization;
- Problems revalidation.

## 14. Benchmark gate policy

Initial benchmark results establish baselines rather than hard merge blockers.

A performance budget becomes merge-gating only when:

1. the workload fixture is stable;
2. variance is measured;
3. hardware/environment is controlled enough;
4. the budget does not incentivize semantic shortcuts;
5. the threshold is documented in the implementation roadmap/release criteria.

Regressions above an agreed threshold require either optimization or an explicit accepted rationale.

## 15. Early performance targets are hypotheses

Historical research suggested targets such as a 60 FPS editor viewport and bounded frame-prep/GPU time.

Checkpoint 7 does **not** elevate those unmeasured numbers into architectural truth.

Checkpoint 8 may use them as provisional implementation targets, clearly labeled as budgets to validate by profiling.

## 16. Test reproducibility

Every nondeterministic test must expose/record its seed.

CI failures from property/state-machine tests report the minimized input/seed.

Tests must not rely on:

- sleep duration for ordering correctness;
- current wall-clock time;
- random OSRS terrain jitter without explicit seed/input;
- arbitrary filesystem ordering;
- unrecorded GPU adapter selection.

## 17. Flaky-test policy

A flaky parity test is a defect.

Do not permanently solve flakiness with broad retries.

Temporary retries may aid diagnosis, but the owner must identify whether nondeterminism originates from:

- test setup;
- async generation races;
- unpinned inputs;
- GPU backend variance;
- actual implementation nondeterminism.

Canonical reference tests must eventually become deterministic on their designated runner.
