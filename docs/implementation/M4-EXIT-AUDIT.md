# M4 Exit Audit: Model Decode and Exact Construction

Status: **COMPLETE - PR READY**  
Milestone: `M4 - Model decode and exact model construction`  
Branch: `impl/m4-model-decode-construction`  
Baseline: M3 squash merge `6e350afeb73c96f1b1ccd050ef427028e3015987`  
Target: `osrs-live-241-2026-09-30-openrs2-2727`

## Exit decision

M4 is implementation-complete for its owned target-era model decoding and pre-GPU construction scope. The permanent semantic-parity gates pass on implementation closure head `999bbd3d749806786c210fb60d60f34e65d6e704` in workflow run `37881488350`, and the documentation-complete closure head `30dacfcf466f3b1024d8359b0c22c4e423580283` passed the same Tier A/B/C chain in workflow run `37881919532`.

This audit does not claim later scene, normal-reconciliation, lighting, contour, animation, render-extraction, or GPU behavior. Those remain with their owning milestones.

## Target and evidence pins

- OSRS live build: `241`
- OpenRS2 cache: `2727`
- cache fingerprint: `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`
- profile digest: `cfdefa9ef99eff799fcef4fdf0ec78d9fdcd72d8e5be78e1c154d018ab4575b7`
- primary semantic source: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- canonical construction spec: `docs/specs/model-build.md`
- canonical coordinate spec: `docs/specs/coordinates.md`

## Scope reviewed

M4 owns:

1. ModelData decoder substrate and provenance;
2. target-era ModelData binary decode;
3. immutable/revision-aware raw model repository and raw variants;
4. exact typed/untyped object model selection;
5. exact raw mirroring and winding;
6. exact multi-model combination;
7. exact instance transform order and integer arithmetic;
8. permanent M4 fixtures and malformed-input fuzz smoke;
9. implementation-era parity documentation.

M4 does not own:

- scene placement/loc dispatch;
- `nonFlatShading` initial-static-entity representation split and scene normal reconciliation;
- final normal generation/merging or lighting;
- contour execution;
- animation execution;
- renderer face ordering/material extraction;
- GPU mesh construction;
- editor behavior.

## Checkpoint audit

### 1. Decoder substrate: PASS

M4 extends the standard M3 decoder envelope rather than creating a parallel error system.

Verified behavior:

- `DecodeSubject::Model(model_id)` identifies the semantic artifact;
- checked random-access and forked readers support ModelData's parallel streams;
- signed short-smart semantics match the pinned client;
- malformed model failures retain target/cache/archive/file plus model identity provenance.

### 2. Target-era raw ModelData decode: PASS

The selected build-241 target was exhaustively swept through the production model repository path.

Observed target inventory:

- total model groups: `62,043`;
- `FF FD`: `35,103`;
- `FF FE`: `26,940`;
- `FF FF`: `0`;
- legacy: `0`;
- empty model groups: `0`;
- multi-file model groups: `0`.

Every target model decoded successfully.

Canonical decode preserves, where encoded:

- model-local integer vertices;
- face topology;
- face colors;
- render types;
- uniform or per-face priorities;
- signed alpha;
- face textures and texture-face selectors;
- texture triangles and complex mapping inputs;
- vertex skins;
- face skins;
- skeletal vertex inputs;
- authored face bias;
- exact target format identity;
- target provenance.

Historical `FF FF` and legacy families remain explicit typed failures for this target. Their absence from build 241 is target evidence, not a claim that those formats never existed.

### 3. Raw repository and variant identity: PASS

`ModelSourceRepository` binds verified cache transport and target-aware model decode.

Raw cache identity is:

```text
build + TargetProvenance + ModelId + RawModelVariant
```

Verified rules:

- authoritative unmirrored decode is immutable/shared;
- repeated loads reuse the same shared source;
- mirrored and unmirrored variants cannot alias;
- model ID and target provenance must match admission key;
- identical derived re-admission reuses the entry;
- conflicting semantic data cannot replace an existing key;
- mutable instance work is performed on owned construction state.

The exhaustive target sweep loaded all `62,043` target models through this repository boundary.

### 4. MODEL-BUILD-001 selection and combination: PASS

Exact selection semantics:

- untyped model table accepts requested loc type `10` only;
- all untyped model IDs are combined in source order;
- missing/empty IDs produce semantic absence;
- typed model table requires exact requested loc-type match;
- typed miss produces semantic absence;
- no first-model, nearest-type, or type-10 fallback is invented.

Exact multi-model combination preserves the pinned constructor behavior relevant to canonical M4 state:

- lazy vertex admission;
- exact XYZ deduplication;
- deterministic first-seen coordinate ownership;
- face remapping through deduplicated vertices;
- first admitted vertex skin/skeletal metadata wins for duplicate coordinates;
- texture-selector offsets account for prior texture triangles;
- differing uniform priorities materialize per-face priority output;
- optional arrays use audited default/sentinel behavior;
- render-type-0 texture triangles participate in vertex remapping;
- complex texture mapping words for render types `1..=3` are preserved as mapping inputs, not misinterpreted as ordinary model-vertex indices;
- all contributing source identities remain retained.

Parity status: `MODEL-BUILD-001 = EXISTING`.

### 5. MODEL-BUILD-002 mirror semantics: PASS

Typed raw mirror selection is exactly:

```text
isRotated XOR (orientation > 3)
```

Untyped/type-10 selection retains the pinned special branch and therefore uses `isRotated` directly for the accepted path.

Raw mirror operation:

- negates model-local Z using wrapping signed integer behavior;
- swaps face A/C winding;
- leaves the immutable unmirrored source untouched;
- does not substitute renderer-side negative scaling;
- does not rewrite texture-triangle metadata that the pinned mirror method does not rewrite.

Mirrored variants are cached separately and reused.

Parity status: `MODEL-BUILD-002 = EXISTING`.

### 6. MODEL-BUILD-003 and COORD-002 transforms: PASS

Exact instance stage order:

1. for loc type `4` and original orientation `> 3`, rotate by `256` JAU;
2. translate `(45, 0, -45)`;
3. mask with `orientation & 3`;
4. apply ordinary quarter turn for `1`, `2`, or `3`;
5. recolor in authored pair order;
6. retexture in authored pair order;
7. resize using definition X/Y/Z scales and integer `/128` semantics;
8. apply final definition translation.

Exact integer behavior includes:

- pinned sine/cosine table value `46340` at JAU index `256`;
- wrapping Java-int multiplication/addition/subtraction/negation where applicable;
- arithmetic signed right shift;
- signed integer division truncating toward zero;
- no floating-point matrix fusion.

Retexture preserves the raw signed-short `-1` sentinel through canonical `None`/`TextureId` conversion.

Parity status:

- `MODEL-BUILD-003 = EXISTING`;
- `COORD-002 = EXISTING`.

### 7. MODEL-BUILD-005 ownership: PARTIAL by design

M4 proves the ownership rule for its owned operations:

- raw decoded/cache source remains unchanged by mirrored variant construction;
- combining sources does not mutate them;
- recolor/retexture/orientation/resize/translation operate on owned assembled state;
- two independently transformed instances can share one canonical source without contamination.

The full spec also names contouring, animation pose, scene normal accumulation, and matched-face suppression. Those operations do not belong to M4 and have not yet been used to close the complete ownership contract.

Parity status: `MODEL-BUILD-005 = PARTIAL`.

### 8. MODEL-BUILD-004 non-flat-shading path: DEFERRED

`MODEL-BUILD-004` requires the initial static-entity split between immediately lit models and pre-lighting ModelData retained for normal reconciliation. That path depends on later scene/normals/lighting ownership.

M4 intentionally does not invent or prematurely flatten that representation distinction.

Parity status: `MODEL-BUILD-004 = REQUIRED`.

### 9. FACE-001 render-extraction preservation: DEFERRED

M4 preserves optional face/material metadata through decode and semantic construction, but `FACE-001` specifically requires proof through render extraction. No renderer extraction exists in M4.

Parity status: `FACE-001 = REQUIRED`.

## Permanent verification artifacts

### Checked-in semantic fixture manifest

`reference-fixtures/model/m4-p0-p1.txt` pins eight M4 semantic cases to executable artifacts:

1. typed selection;
2. untyped type-10 selection;
3. mirror geometry/winding;
4. multi-model combination;
5. complex texture-mapping preservation;
6. type-4 transform order;
7. ordinary orientation transforms;
8. M4 ownership/source isolation.

`crates/osrs-core/tests/m4_fixture_manifest.rs` gates that inventory so fixture drift is explicit.

### Model decoder fuzz smoke

`crates/osrs-cache/tests/m4_model_fuzz_smoke.rs` is deterministic and CI-stable. It exercises:

- fixed malformed/truncated corpora;
- 512 generated bounded byte sequences;
- arbitrary trailer paths;
- forced `FF FD` target-family dispatch;
- forced `FF FE` target-family dispatch;
- panic containment expectation for every input.

This is a merge-gating malformed-input smoke test, not a claim of exhaustive formal fuzzing.

### Tier C semantic parity

Permanent `.github/workflows/ci.yml` Tier C now gates:

- M3 checked-in P0 decode fixtures;
- M3 deterministic decoder fuzz smoke;
- M4 fixture inventory;
- M4 exact selection/mirror/combine tests;
- M4 exact instance transform tests;
- M4 model decoder fuzz smoke.

No temporary write-capable or target-download workflow is retained.

## Implementation closure CI

Implementation closure head:

`999bbd3d749806786c210fb60d60f34e65d6e704`

Workflow run:

`37881488350`

Result: PASS

- Tier A static quality: PASS
- Tier B full workspace tests: PASS
- Tier C semantic parity: PASS
  - M3 fixture gate: PASS
  - M3 fuzz smoke: PASS
  - M4 fixture inventory: PASS
  - M4 exact construction fixtures: PASS
  - M4 model fuzz smoke: PASS

## Documentation closure CI

Documentation-complete head:

`30dacfcf466f3b1024d8359b0c22c4e423580283`

Workflow run:

`37881919532`

Result: PASS

- Tier A static quality: PASS
- Tier B full workspace tests: PASS
- Tier C semantic parity: PASS

## Parity matrix changes justified at M4 exit

Promoted to `EXISTING`:

- `MODEL-BUILD-001`;
- `MODEL-BUILD-002`;
- `MODEL-BUILD-003`;
- `COORD-002`.

Advanced to `PARTIAL`:

- `MODEL-BUILD-005`.

Deliberately unchanged:

- `MODEL-BUILD-004 = REQUIRED`;
- `FACE-001 = REQUIRED`;
- later normals/lighting, contour, animation, scene, renderer, and editor rows.

## Carryovers

The following are explicit later-milestone work, not M4 exit blockers:

- `MODEL-BUILD-004` non-flat-shading/pre-lighting representation path;
- remainder of `MODEL-BUILD-005` for contour, animation, and scene-normal mutation;
- `FACE-001` end-to-end render extraction preservation;
- normal generation/merging and final lighting;
- contour and animation execution;
- scene placement and collision/occlusion side effects;
- renderer/GPU behavior;
- historical `FF FF`/legacy ModelData compatibility unless a selected target requires it.

## PR readiness

M4 is ready for one milestone PR. The implementation and documentation closure heads have passed the ordinary Tier A/B/C chain. The remaining milestone-boundary action is the final branch-vs-main scope check followed by PR creation only after explicit user continuation.

No M5 or later implementation should begin before that milestone boundary is explicitly accepted.
