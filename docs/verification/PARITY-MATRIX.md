# Semantic Parity Verification Matrix

Status: **Current through M10 Checkpoint 5 plus M10 foundation audit**

This matrix records current production/verification state. Historical milestone conclusions that were later corrected remain documented in their milestone audit files, but this file represents the current authoritative disposition.

Status vocabulary:

- `EXISTING`: owning production behavior plus required exact verification exists.
- `EXISTING-PARTIAL`: a declared subset is implemented and tested; remaining scope is explicit.
- `SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED`: exact source contract is pinned, but owning Rust implementation/fixture is not complete.
- `STRUCTURAL_EXISTING / GPU_PENDING`: semantic/render-structural contract exists, physical GPU realization remains later.
- `DEFERRED`: deliberately belongs to a later milestone.
- `OPEN-ORACLE`: implementation/regression work may exist, but an independent parity oracle is still missing.

## 1. Target/cache/decode foundation

| Family | Current evidence | Status |
|---|---|---|
| target cache identity | OpenRS2 2727, build 241, fingerprint `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38` | EXISTING |
| target profile identity | `rustosrs-target-profile/v1`, current digest `203bb13fc48b56e88257d1a4bc96dfca9afd66fcd9216276bb7f83c97038726f` | EXISTING |
| cache transport | `osrs-cache` transport tests, exact group/fingerprint safeguards | EXISTING |
| object definitions | M3 decode fixtures/unit tests, full-width model IDs, opcode-92 fallback/null | EXISTING |
| floor underlay/overlay | M3 fixtures plus `osrs-core::floor_color` exact HSL tests | EXISTING |
| map terrain/loc streams | M3 fixtures and decoder tests | EXISTING |
| texture definitions | build-241 M3 fixtures | EXISTING |
| sequence/varbit/varp inputs | M3 fixtures/tests | EXISTING |
| malformed/random decoder input | decoder unit/fuzz-smoke suites | EXISTING |

## 2. Model construction

| Spec/family | Current evidence | Status |
|---|---|---|
| model decode | build-241 `FF FD`/`FF FE` decoder sweep and M4 tests | EXISTING |
| `MODEL-BUILD-001` selection/combine | M4 normalized fixture + production tests | EXISTING |
| `MODEL-BUILD-002` mirroring/winding | M4 normalized fixture/tests | EXISTING |
| `MODEL-BUILD-003` transform order | M4 exact transform tests | EXISTING |
| `MODEL-BUILD-004` non-flat/static lifecycle | M7 scene-local ModelData/finalization tests | EXISTING |
| `MODEL-BUILD-005` runtime ownership | M8 private runtime model/contour/animation tests | EXISTING |

## 3. Placement and scene semantics

| Spec | Current evidence | Status |
|---|---|---|
| `LOC-PLACEMENT-001` type dispatch/storage | M6 insertion tests + historical orientation matrices | EXISTING |
| `LOC-PLACEMENT-002` wall-decor offsets | M6 placement tests + pinned initial placement source | EXISTING |
| `LOC-PLACEMENT-003` footprints/centers/heights | M6 placement-height and golden-scene tests | EXISTING |
| `LOC-PLACEMENT-004` initial vs runtime representation | M7/M8 lifecycle tests | EXISTING |
| `LOC-PLACEMENT-005` side-effect ownership/regeneration inputs | M6 planner tests + M9 ownership regression | EXISTING at ownership/regeneration contract |
| private collision/occlusion/shadow bit-grid formulas beyond current contracts | not all promoted as independent narrow specs | DEFERRED until owning specs/fixtures |

### Placement-shadow correction

Pinned `FriendSystem.addObjects(...)` proves `Tiles.Tiles_underlays2` is placement-derived shadow state. Clipped walls write orientation-specific value `50`; clipped game objects may write model-height-derived values capped at `30`. This input is now part of the future `TERRAIN-004` implementation contract.

## 4. Terrain

| Spec | Current evidence | Status |
|---|---|---|
| `TERRAIN-001` shaped topology | 13x4 exact gallery, pinned `SceneTileModel` | EXISTING |
| `TERRAIN-002` flat/shaped surface contract | M6 terrain tests, flat diagonal/sentinel cases | EXISTING |
| `TERRAIN-003` floor-definition HSL | floor decode/post-decode exact tests | EXISTING |
| `TERRAIN-004` complete terrain builder | pinned `class470.method9712(WorldView)` + `FriendSystem.addObjects(...)` shadow writer | SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED |

### `TERRAIN-004` current truth

The old `BLOCKED because class470 is unrelated` conclusion is superseded. The exact public method is now pinned.

Verified source behavior includes:

- slope lighting;
- placement-derived shadow subtraction;
- separable radius-5/11x11 underlay blending;
- weighted hue/saturation/lightness accumulation;
- overlay texture/magenta/secondary branches;
- 3D terrain corner colors based on non-jittered HSL;
- separately jittered palette/minimap-style RGB;
- hue wrap with `& 255`;
- `minPlane` assignment;
- normal finalization and link-below sequencing.

Production Rust builder + executable differential fixture are still required before promotion to `EXISTING`.

## 5. Planes and bridges

| Spec | Current evidence | Status |
|---|---|---|
| `PLANES-001` typed plane separation | `osrs-core` plane types and scene tests | EXISTING |
| `PLANES-002` collision-plane bridge adjustment | pinned client + M6/M9 tests | EXISTING |
| `PLANES-003` structural `setLinkBelow` | exact four-plane fixture + production scene tests | EXISTING |
| `PLANES-004` `minPlane`/`originalPlane` + renderer grouping boundary | pinned `Tile`, `Scene`, `class470`, imported `SceneUploader` | SOURCE_VERIFIED / IMPLEMENTATION_PARTIAL |

Current correction: `Tile.minPlane` and `Tile.originalPlane` are target client scene state. Imported RuneLite `maplevel` changes settings/roof lookup; it does not relocate `tiles[level]` geometry into another semantic storage plane.

Renderer grouping tests and target `minPlane/originalPlane` representation remain required before this row becomes fully `EXISTING` end to end.

## 6. Normals and lighting

| Spec | Current evidence | Status |
|---|---|---|
| `NORMALS-001` base normals | M7 exact smooth/flat/winding tests | EXISTING |
| `NORMALS-002` cross-model merge | M7 positive/negative/translated/hide tests | EXISTING |
| `NORMALS-003` scene neighbor reconciliation | M7 wall/game/floor/plane-above tests | EXISTING |
| `NORMALS-004` merged-normal lighting precedence | M7 exact lighting delta tests | EXISTING |
| `LIGHTING-001` object lighting | M7 exact lighting tests + pinned opcode decode | EXISTING |

Corrections now explicit:

- Scene does reconcile normals across separate wall/object ModelData; this is not mesh welding.
- same-plane and plane-above reconciliation do not share one universal hide flag.
- opcode-39 object contrast is already decoded as signed byte `* 25`; loc lighting then adds `768`.
- raw alpha `-1/-2` affects render type before final face lighting; suppressed faces use baked `c == -2`.

## 7. Morph, animation, contouring

| Family | Current evidence | Status |
|---|---|---|
| varbit/varp/fallback/null morph | M8 exact tests | EXISTING |
| active morph footprint/null model | M8 scene tests | EXISTING |
| contouring | M8 exact flat/slope/threshold/border tests | EXISTING |
| legacy animation decode/pose/progression/replacement | M8 exact suites | EXISTING within declared legacy scope |
| broader skeletal execution | not claimed by M8 | DEFERRED |

## 8. Coordinates and deterministic scene identity

| Spec/family | Current evidence | Status |
|---|---|---|
| `COORD-001` world/region/local composition | M6/M9 exact tests | EXISTING |
| semantic scene hash | `rustosrs-semantic-scene-v1` golden hashes | EXISTING |
| renderer rebase side of coordinate boundary | M10 CP1 exact tests | EXISTING |

Pinned semantic-scene hashes remain:

- composed scene: `50974f0232192dbd97c623d31bdbe208432ef64a13121c2852230afcc262f4a6`
- four-plane linked-below: `bf7a7876864142f75a29c47836f331da6cfb97b512d500274fd3c8035c3a296c`

## 9. Face behavior

| Spec | Current evidence | Status |
|---|---|---|
| `FACE-001` metadata preservation | M10 CP1 `RenderMesh` extraction + optional-array regression | EXISTING |
| `FACE-002` software/client priority preparation | M10 CP2 exact M5 threshold fixture execution | STRUCTURAL_EXISTING / GPU_PENDING |
| imported RuneLite GPU render-mode sorting | source audited, no dedicated Rust comparison profile yet | SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED |
| `FACE-003` semantic alpha/sentinel state | M7 lighting + M10 CP2 draw-alpha preparation | STRUCTURAL_EXISTING / GPU_PENDING |
| suppressed `c == -2` face admission | semantic output exists; explicit renderer draw exclusion fixture still required | IMPLEMENTATION_REQUIRED before Reference GPU draw |
| `FACE-004` authored bias metadata | M10 extraction preserves input; GPU realization pending | STRUCTURAL_EXISTING / GPU_PENDING |

### Priority provenance rule

RustOSRS M10 priority preparation targets pinned software/client `Model.method5946`.

Imported RuneLite GPU only executes its full priority queue where `prioritySort=true`, notably `SORTED_NO_DEPTH`; ordinary dynamic/static work is not universally priority-sorted. These are separate reference profiles, not contradictory implementations.

## 10. Textures/materials

| Spec/family | Current evidence | Status |
|---|---|---|
| `TEXTURE-001` full-width IDs/material handles | M10 CP4 | EXISTING CPU handoff |
| canonical/explicit/projected model UV | M10 CP4 exact tests | EXISTING CPU preparation |
| texture animation vector/tick preparation | M10 CP4 | EXISTING CPU preparation |
| 128x128 imported RuneLite pixel construction | source audited only | SOURCE_VERIFIED / GPU IMPLEMENTATION_REQUIRED |
| RGB-zero transparency + level0 cutout | source/shader audited only | SOURCE_VERIFIED / GPU IMPLEMENTATION_REQUIRED |
| Reference filtering/wrap behavior | source audited only | SOURCE_VERIFIED / GPU IMPLEMENTATION_REQUIRED |

Imported RuneLite filtering is not simply "nearest": magnification is nearest; minification is nearest at filter level 0 and `NEAREST_MIPMAP_LINEAR` at level >=1; imported config defaults to level 1. S wrap is clamp-to-edge and T remains default repeat in the audited setup.

## 11. Renderer structural M10 progress

| Checkpoint | Contract | Status |
|---|---|---|
| CP1 | render crate, immutable snapshot/generation, integer rebase, metadata handoff | EXISTING |
| CP2 | software priority + draw-alpha preparation | EXISTING |
| CP3 | static/dynamic/ordered classification + CPU draw plan | EXISTING |
| CP4 | material table, full-width texture handles, UV/animation preparation | EXISTING |
| CP5 | 8x8 storage-plane zones, dirty state, stale ticket rejection | EXISTING |
| picking/diagnostics/final M10 exit | later M10 | DEFERRED |

## 12. Raster/GPU Reference policy

| Contract | Current state |
|---|---|
| reverse-Z clear 0 | PROJECT_DECISION + imported evidence |
| strict `Greater` Reference depth compare | corrected ADR-0004, GPU implementation pending |
| no-far projection equivalence | source verified, implementation/fixture pending |
| authored bias distance dependence | source verified, fixture pending |
| Reference alpha depth-write/no-depth behavior | source verified at imported renderer level, implementation fixture pending |
| CCW/backface convention | project decision, GPU fixture pending |

`GreaterEqual` is no longer the Reference baseline.

## 13. Visual verification

| Verification class | Current state |
|---|---|
| RustOSRS self-generated Reference screenshots | planned/usable as internal regression evidence (`V3R`) |
| independent external RuneLite/client visual oracle | OPEN-ORACLE |
| measured external "1:1" visual parity claim | NOT YET PERMITTED |

A self-generated golden proves regression stability, not external parity. External V3 requires a separately generated, provenance-complete comparison artifact.

## 14. Foundation audit gates before first Reference viewport is considered solid

Required before/while crossing M10 -> M11/M12:

1. implement `TERRAIN-004` from the now-pinned builder and add an exact differential fixture;
2. represent/test `minPlane` and `originalPlane` where renderer visibility owns them;
3. add explicit suppressed-face admission test so `c == -2` cannot reach a Reference draw;
4. implement strict-`Greater` reverse-Z/no-far projection tests;
5. add near/far authored-bias fixture;
6. encode Reference alpha depth-write/no-depth behavior explicitly;
7. implement/reference-test texture pixel construction, cutout, filtering, and wrap behavior;
8. establish at least one external visual oracle before claiming measured 1:1 rendering.

No known P0/P1 semantic defect found by the M10 foundation audit is being intentionally pushed into renderer compensation.
