# M8 Exit Audit

Status: **PASS, merge gated on exact-head CI and PR verification**

Milestone: **M8 - Morphs, contouring, and animation ownership**

Base `main` at audit start:

`12f0c28011291a0d30b664132f06b297cfb0a72a`

Implementation branch:

`impl/m8-dynamic-model-state`

## Scope decision

M8 remains inside the semantic/runtime ownership boundary. It does not introduce renderer, GPU, editor, terrain-color-builder, or broad skeletal-animation claims.

The milestone closes the verified dynamic-object subset required by the roadmap:

- varbit/varp object transform resolution;
- fallback and semantic-null morphs;
- active-definition footprint and placement-input recomputation;
- exact integer contour-ground behavior within the verified contract;
- deterministic legacy sequence/frame progression;
- legacy animation archive decode and object-model pose application;
- replacement preserve/restart behavior;
- private dynamic working-model ownership;
- pending/live scene replacement distinct from initial static construction.

## Owned specification audit

### MORPH-001

**EXISTING**

Production implementation and exact tests cover:

- varbit selection;
- varp selection when no varbit is configured;
- in-range transform selection;
- out-of-range fallback;
- explicit null transform entries;
- null fallback;
- active-definition footprint changes;
- null morph short-circuit before model lookup.

Primary verification:

- `crates/osrs-core/src/morph.rs`
- `crates/osrs-reference/tests/m8_morph_resolution.rs`
- `crates/osrs-scene/tests/m8_active_morph_placement.rs`
- `crates/osrs-reference/tests/m8_dynamic_model_assembly.rs`

### CONTOUR-001

**EXISTING**

Production implementation and tests cover:

- contour disabled control;
- flat-height fast path;
- unequal-corner bilinear interpolation;
- clip-threshold behavior;
- negative and positive model Y inputs;
- tile-boundary sampling;
- source/base immutability.

Primary verification:

- `crates/osrs-core/src/contour.rs`
- `crates/osrs-reference/tests/m8_contour_ground.rs`
- `crates/osrs-reference/tests/m8_dynamic_model_assembly.rs`

### ANIMATION-001

**EXISTING within the explicitly verified M8 legacy-frame scope**

Production implementation and tests cover:

- legacy skeleton/frame decode;
- deterministic object-model pose;
- orientation-aware application;
- vertex and face-alpha transforms where represented;
- deterministic frame-duration progression;
- looping/restart/finish behavior;
- deterministic preview snap-to-start;
- same-sequence preserve behavior;
- restart and loop-count restart replacement modes.

Primary verification:

- `crates/osrs-cache/src/decode/animation.rs`
- `crates/osrs-core/src/animation.rs`
- `crates/osrs-core/src/animation_pose.rs`
- `crates/osrs-core/src/sequence_replacement.rs`
- `crates/osrs-reference/tests/m8_legacy_animation_decode.rs`
- `crates/osrs-reference/tests/m8_legacy_animation_pose.rs`
- `crates/osrs-reference/tests/m8_sequence_progression.rs`
- `crates/osrs-reference/tests/m8_sequence_replacement.rs`

This milestone does not infer or claim broader skeletal animation execution that is not covered by the verified source/test contract.

### LOC-PLACEMENT-004 runtime side

**EXISTING**

The M7 initial path already proves qualifying `nonFlatShading` objects retain ModelData for scene reconciliation. M8 adds the runtime side and an explicit same-definition cross-path regression:

- initial non-flat path returns scene-local ModelData;
- runtime path obtains an already-lit cached base model;
- pending/live replacement removes and reinserts through the correct scene category;
- replacement timing and requested type/orientation metadata are preserved.

Primary verification:

- `crates/osrs-reference/tests/m8_initial_runtime_model_path.rs`
- `crates/osrs-reference/tests/m8_pending_replacement_contract.rs`
- `crates/osrs-scene/src/pending_replacement.rs`
- `crates/osrs-scene/src/dynamic_model.rs`

### MODEL-BUILD-005 remaining ownership

**EXISTING**

M8 closes the contour/animation ownership portion left partial after M7:

- shared raw source remains immutable;
- cached lit dynamic base remains immutable;
- pose/contour operations receive private runtime state;
- mutating one initial scene-local instance does not alter the runtime cached model or source;
- animation plus contouring occurs on owned runtime state.

Primary verification:

- `crates/osrs-core/src/dynamic_model.rs`
- `crates/osrs-reference/tests/m8_dynamic_model_assembly.rs`
- `crates/osrs-reference/tests/m8_initial_runtime_model_path.rs`
- `crates/osrs-reference/tests/m8_contour_ground.rs`
- `crates/osrs-reference/tests/m8_legacy_animation_pose.rs`

## Roadmap exit-gate audit

| M8 exit gate | Result | Evidence |
|---|---|---|
| morph, contour, and owned animation rows become `EXISTING` | PASS | `docs/verification/PARITY-MATRIX.md` plus exact M8 suites |
| initial-vs-pending replacement fixture passes | PASS | `m8_initial_runtime_model_path.rs`, `m8_pending_replacement_contract.rs` |
| active morph changing footprint recomputes placement inputs correctly | PASS | `m8_active_morph_placement.rs` |
| null morph yields no model | PASS | `m8_morph_resolution.rs`, `m8_dynamic_model_assembly.rs` |
| contouring never mutates shared base model | PASS | `m8_contour_ground.rs`, `m8_dynamic_model_assembly.rs` |
| deterministic preview tick/frame tests exist | PASS | `m8_sequence_progression.rs` |

## Permanent CI gate

`.github/workflows/ci.yml` now contains a dedicated M8 Tier C step that explicitly executes:

- morph resolution;
- contour-ground parity;
- legacy animation decode;
- legacy animation pose;
- deterministic sequence progression;
- sequence replacement semantics;
- dynamic model assembly/ownership;
- pending replacement contract;
- initial-versus-runtime representation regression;
- active morph placement recomputation.

This is in addition to Tier B full-workspace tests.

## Branch scope audit

Relative to the M7 `main` baseline, M8 changes are confined to:

- M8 cache animation decoding support;
- M8 core morph/contour/animation/runtime model state;
- M8 scene dynamic placement/replacement plumbing;
- exact M8 reference/scene tests;
- semantic parity documentation;
- CI semantic-gate wiring;
- this exit audit.

No `osrs-render` or `osrs-editor` implementation is introduced. No renderer face-priority executor, GPU policy, or terrain-color builder is pulled forward.

## Source/evidence posture

The M8 contracts continue to terminate in the canonical specs and pinned/reference sources already recorded by the project, including the audited object-definition, dynamic-object, sequence, animation, and ModelData behavior. The milestone does not promote unsupported behavior merely because a plausible implementation exists.

## Deferred boundaries

The following remain intentionally outside M8:

- full semantic parity closure across all remaining `REQUIRED` rows, owned by M9;
- renderer extraction and face/material runtime policy;
- FACE-001 through FACE-004 renderer-facing closure;
- TEXTURE-001 renderer-facing closure;
- TERRAIN-004, which remains blocked on exact builder evidence;
- GPU and editor milestones;
- animation behavior outside the explicitly verified M8 scope.

## Merge gate

The semantic and scope audit is complete. M8 may merge only after:

1. Tier A passes on the exact final branch head;
2. Tier B passes on that same head;
3. Tier C passes on that same head, including the permanent M8 suite;
4. a single M8 PR is opened against unchanged `main`;
5. the exact PR changed-file list is audited for scope;
6. PR-triggered CI passes on the exact PR head;
7. the PR remains mergeable and `main` has not unexpectedly advanced.

After squash merge, `main` must be verified and work must stop before M9.
