# RustOSRS Verification Architecture

Status: **Checkpoint 7 normative verification blueprint**  
Decision class: `PROJECT_DECISION`

This document defines how RustOSRS proves decode, semantic, render-structural, visual, editor, persistence, and concurrency correctness.

The governing rule is:

> Test the earliest layer that owns the behavior, and use the strongest comparison that layer permits.

A visual golden may corroborate a semantic behavior, but it may not replace an exact semantic test. A passing GPU image never excuses a wrong loc type, transform order, normal merge, bridge plane, face priority order, or integer contour result.

## 1. Verification layers

RustOSRS verification is organized around the parity levels established in `08-PARITY-MODEL.md`.

### V0. Decode verification

Owns P0 decode parity.

Examples:

- opcode/default decoding;
- model fields;
- floor definitions;
- map/loc records;
- texture/material inputs;
- revision-gated widths and sentinels.

Preferred comparison: exact decoded values.

### V1. Semantic verification

Owns P1 semantic parity.

Examples:

- loc dispatch;
- model selection;
- mirroring/winding;
- transform order;
- footprints;
- morphs;
- contouring;
- base/merged normals;
- bridge relinking;
- exact priority emission order oracle.

Preferred comparison: exact values, exact arrays, exact identities, exact ordering.

### V2. Render-structural verification

Owns P2.

Examples:

- render extraction retains face metadata;
- static/dynamic classification preserves required semantics;
- material mapping preserves semantic texture identity;
- ordered path consumes exact priority/alpha/bias inputs;
- zone compilation does not erase provenance or semantic pick identity.

Preferred comparison: exact extracted artifacts and deterministic command lists.

### V3. Reference visual verification

Owns P3.

Examples:

- fixed-camera reference render;
- reverse-Z/culling interaction;
- authored bias presentation;
- transparency composition;
- texture mapping;
- bridge/roof visibility;
- complete composed scene appearance.

Preferred comparison: screenshot/image metric with explicit tolerance, only after V0-V2 have passed.

### V4. Enhanced presentation verification

Owns P4.

Examples:

- MSAA path;
- anisotropic filtering;
- enhanced lighting overlays;
- fog/presentation options;
- editor-only diagnostics.

These tests prove intentional product behavior, not OSRS semantic parity.

### V5. Editor/document verification

Owns project behavior rather than parity.

Examples:

- command apply/revert;
- transactions;
- undo/redo;
- cross-region edits;
- selection identity;
- save/autosave/recovery;
- export validation;
- stale-generation rejection;
- device/resource rebuild isolation.

## 2. Verification dependency rule

Tests form an evidence ladder:

```text
pinned source / executable oracle
            |
            v
       exact fixture
            |
            v
    Rust unit/differential
            |
            v
 semantic composition scene
            |
            v
 render-structural assertion
            |
            v
 reference screenshot
```

A higher step does not replace a lower step.

When a P3 screenshot fails:

1. first run/inspect the associated V0-V2 tests;
2. if an exact test fails, fix that earlier layer;
3. only investigate raster/presentation after semantic and structural output is proven correct.

## 3. Source oracle policy

`osrs-reference` is development/test infrastructure for executing, normalizing, or comparing reference behavior.

Permitted oracles include:

- the pinned public melxin/deob snapshot;
- the local deob harness only where its exact source identity is recorded or the specific behavior is independently pinned;
- imported RuneLite source for explicitly classified renderer/reference behavior;
- exact checked-in source-derived tables and fixtures;
- independent implementations such as FileStore only as corroboration unless separately promoted.

Reference executors never become production runtime dependencies.

## 4. Fixture identity

Every new reference fixture must have a machine-readable or adjacent manifest containing at minimum:

```text
fixture_id
fixture_schema_version
owned_spec_ids
parity_level
oracle_kind
oracle_repository
oracle_commit/tree/blob pins
oracle_source_paths/symbols
harness_revision
input_description
expected_output_hash
normalization_rules
created_at / provenance note
```

When a fixture depends on cache data, also record:

```text
cache fingerprint/revision
decoder schema version
region/object/model ids used
XTEA/input identity where relevant
```

A fixture without reproducible provenance is evidence for investigation, not merge-gating canonical truth.

## 5. Fixture format policy

The existing monolithic `reference-fixtures/deob_golden.txt` remains historical executable evidence, but new fixture work should move toward one fixture family per semantic contract.

Preferred structure:

```text
reference-fixtures/
  manifest/
  model/
  normals/
  placement/
  morph/
  contour/
  terrain/
  planes/
  priority/
  alpha/
  textures/
  scenes/
```

Formats should be deterministic, diffable, and easy to consume from Rust.

Recommended formats:

- JSON/TOML/YAML for structured small fixtures;
- compact binary only for large arrays where accompanied by a readable manifest and checksum;
- PNG for image goldens;
- text/JSON summaries beside images for camera/profile/state/provenance.

Do not serialize opaque Java object graphs as the canonical interchange format.

## 6. Exact comparison policy

Exact comparison is mandatory for contracts whose semantic domain is exact.

Use exact equality for:

- ids;
- opcodes/default values;
- integer vertices;
- triangle indices;
- orientation/type/plane;
- footprints;
- integer HSL/helper outputs;
- normal components and magnitudes;
- merged-normal components/magnitudes;
- face render type/priority/alpha/texture/bias metadata;
- transform output;
- contour output;
- terrain topology;
- morph branch result;
- bridge tile identity/relinking;
- deterministic face emission order;
- command/transaction semantic state;
- serialized project schema fields where deterministic.

Do not use epsilon comparison merely because the Rust implementation eventually feeds a GPU.

## 7. Floating/tolerance policy

Tolerance is allowed only where the owning contract is floating, rasterized, or adapter-sensitive.

Examples:

- final reference-profile pixels;
- depth edge coverage;
- floating projection matrices when renderer policy specifies float math;
- GPU interpolated texture samples;
- anti-aliasing output.

Every tolerant test must state:

- why exact equality is inappropriate;
- per-channel/per-pixel or metric threshold;
- allowed changed-pixel fraction if used;
- ignored mask/edge regions if any;
- reference adapter/backend assumptions;
- whether the tolerance is merge-gating on every platform or only on a canonical renderer runner.

Broad undocumented thresholds are prohibited.

## 8. Differential test pattern

For high-risk semantics, prefer a direct differential pattern:

```text
same normalized input
      |                 |
      v                 v
reference oracle      Rust implementation
      |                 |
      +------ exact -----+
             compare
```

The Rust test should be able to consume a checked-in expected fixture without requiring Java at ordinary test time.

Regeneration is a separate explicit developer action.

## 9. Required semantic fixture families

Checkpoint 7 defines the minimum fixture families required before the corresponding implementation milestone can claim parity.

### Placement

- loc types `0..22` and representative `>=12`;
- orientations `0..3`;
- wall decoration types `4..8`;
- default and non-default wall displacement;
- square/non-square footprints;
- initial vs pending-replacement construction;
- floor-decoration exact height/no-lift.

### Model build

- typed model hit/miss;
- type-10 untyped combine;
- mirror matrix across `isRotated` and orientation ranges;
- winding verification;
- all ordinary rotations;
- type-4 `orientation > 3` recenter;
- recolor/retexture/resize/translate combined-order case;
- shared-source immutability.

### Normals and lighting

- base flat/smooth normals;
- mirrored-winding normal case;
- positive cross-model merge;
- zero-match negative control;
- translated merge;
- merge with/without matched-face suppression;
- dual-arm wall scene merge;
- wall/game neighbor merge;
- floor-decoration matched-face merge;
- plane-above neighbor;
- merged-normal final-lighting delta;
- lighting ambient/contrast boundary cases.

### Morph, animation, contour

- varbit selector;
- varp selector;
- fallback transform;
- null transform;
- footprint-changing transform;
- one deterministic animation frame/pose;
- animation replacement preserve/restart rules;
- contour full-warp;
- nonzero clip thresholds;
- fast paths;
- tile-boundary interpolation;
- source immutability.

### Terrain

- all `13 x 4` shape/rotation outputs;
- flat tile diagonal split;
- sentinel hidden surface;
- underlay weighted-HSL boundaries;
- overlay defaults/opcodes/secondary color;
- complete terrain-color builder only after `TERRAIN-004` provenance gate closes.

### Planes and bridges

- encoded planes `0..3`, bridge bit on/off;
- collision-plane adjustment;
- full four-plane `setLinkBelow` synthetic column;
- tagged game-object plane changes;
- linked-below identity;
- renderer roof grouping disabled/enabled without semantic state mutation.

### Face/material behavior

- all priorities `0..11`;
- depth values around `avg12`, `avg34`, `avg68` thresholds;
- priority 10/11 interleave;
- alpha `0`, ordinary values, and sentinel case;
- model-level plus face-level transparency;
- authored face-bias coplanar ordering;
- texture-triangle UV reconstruction;
- no-texture-face canonical UVs;
- deterministic texture-animation direction/speed/tick.

## 10. Golden semantic scenes

A golden scene is a composed semantic fixture, not only a screenshot.

Every golden scene must include a deterministic scene-state artifact containing:

- target profile/cache identity;
- region/tile inputs or synthetic scene definition;
- semantic object placements;
- explicit varbit/varp state;
- animation/tick state;
- camera only when the scene also owns a visual golden;
- expected semantic scene summary;
- expected render extraction summary where applicable.

This allows the same scene to drive P1, P2, and P3 verification.

## 11. Reference screenshot policy

Reference images must be generated only from the deterministic Reference render profile.

Each image manifest records:

- golden scene id;
- viewport width/height;
- camera position/target/yaw/pitch/projection parameters;
- render tick;
- plane/roof visibility;
- render profile and profile version;
- GPU backend/adapter/driver family when relevant;
- expected image hash;
- comparison metric/tolerance.

Reference image tests must not depend on editor docking layout or unrelated UI chrome.

## 12. Editor command verification

Every command type requires at least:

1. apply expected-state test;
2. revert exact-before-state test;
3. apply -> revert -> apply determinism test;
4. invalid-command no-mutation test;
5. `ChangeSet` scope test.

Transaction tests additionally cover:

- cancel restores exact initial semantic state;
- one user gesture creates one history entry;
- cross-region transaction is atomic;
- terrain stroke result is independent of UI frame count;
- redo is invalidated correctly after divergent edit history.

## 13. Randomized history/state-machine testing

Use property/state-machine tests to generate valid sequences of editor operations.

For a generated command sequence:

- apply all commands;
- record final canonical semantic hash;
- undo to baseline and require exact baseline hash;
- redo all and require the original final hash;
- periodically serialize/reload the project and require equivalent semantic hash.

The generator should eventually include loc, terrain, region-boundary, and selection-independent editing operations.

## 14. Persistence and recovery failure injection

Required tests include:

- save to temporary path succeeds then atomic replacement succeeds;
- write failure leaves previous project intact;
- serialization failure never marks generation saved;
- autosave generation N cannot overwrite newer autosave N+1;
- recovery newer than explicit save is detected;
- corrupt recovery artifact is rejected without damaging explicit save;
- project schema migration preserves semantic content;
- source-cache fingerprint mismatch is surfaced explicitly;
- export failure does not alter project save state.

Filesystem-specific atomicity guarantees must be documented per supported platform during implementation.

## 15. Generation/concurrency verification

All asynchronous systems consuming immutable snapshots require stale-result tests.

At minimum:

- scene build generation N finishes after N+1 and is rejected;
- zone compile N is rejected after a newer semantic edit;
- stale pick result cannot change selection;
- stale validation result cannot clear newer Problems entries;
- stale autosave cannot replace newer recovery state;
- stale thumbnail/search result cannot corrupt authoritative data.

Race tests should use deterministic barriers/channels rather than relying only on timing/sleeps.

## 16. GPU lifecycle verification

Renderer tests must cover:

- GPU resource rebuild from extraction after simulated device/resource loss;
- semantic scene remains unchanged across renderer destruction/recreation;
- picking generation resets safely;
- zone resources rebuild only from current generation;
- reference profile state is restored deterministically;
- unsupported adapter capability produces an explicit diagnostic instead of silent material loss.

Where CI lacks real GPU coverage, use both logic-level renderer tests and a designated GPU runner for reference render tests.

## 17. Property and fuzz testing

### Property tests

Use `proptest` or equivalent for invariants such as:

- model decode counts never cause unchecked allocation overflow;
- transform/mirror operations preserve index validity;
- terrain generated faces reference valid vertices;
- bridge relinking preserves tile ownership invariants;
- apply/revert is identity for generated commands;
- texture/material ID mapping is one-to-one within an extraction generation;
- zone dirty propagation includes every semantically affected zone.

### Fuzz targets

`cargo-fuzz` targets should include:

- model decoder;
- object-definition decoder;
- floor/texture definitions;
- map/terrain decoder;
- loc stream decoder;
- project file parser/migration;
- export serializers where malformed semantic input can reach them.

Fuzzing is for robustness. It does not replace reference parity fixtures.

## 18. Performance regression verification

Performance tests are informative until a milestone explicitly promotes a budget to a release gate.

Track at minimum:

- cache decode throughput;
- scene materialization;
- normal reconciliation hotspots;
- priority sort;
- zone compilation;
- GPU upload volume;
- frame preparation;
- picking latency;
- project serialization;
- large undo/redo transactions.

Use Criterion or equivalent stable benchmark harnesses. Store benchmark scenario identity and hardware/environment metadata with results.

Do not sacrifice semantic correctness to pass a benchmark. Optimize only after retaining equivalent verification coverage.

## 19. Test naming

Tests should expose the owning contract.

Preferred examples:

```text
model_build_001_typed_miss_returns_none
normals_002_translated_coincident_vertices_accumulate
planes_003_link_below_moves_tagged_game_object
face_002_priority_10_interleaves_at_avg34
editor_move_loc_apply_revert_identity
renderer_stale_zone_generation_is_discarded
```

This makes CI failures traceable directly to specs/ADRs.

## 20. Merge-gate philosophy

A change may not be merged merely because the editor launches or a screenshot looks plausible.

The implementation roadmap must assign each milestone the minimum verification tier required for completion.

At a minimum:

- decoder work requires V0;
- semantic construction requires V0+V1;
- renderer extraction requires V0+V1+V2;
- reference renderer milestones require V0-V3;
- editor mutation/persistence milestones require their V5 suites;
- enhanced presentation requires lower parity suites plus its own V4 checks.

Checkpoint 8 will convert these requirements into implementation milestone exit criteria.
