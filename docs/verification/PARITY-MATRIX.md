# Semantic and Reference Parity Verification Matrix

Status: **Current through the 2026-10-15 implementation audit**

This matrix records proof state, not merely feature existence. A working renderer/editor path is not automatically external parity evidence.

For current project completion scope read `docs/PROJECT-STATUS.md`. For exact branch/SHA/CI state read `DEV_HANDOFF.md`.

## 1. Status vocabulary

- `EXISTING`: owning production behavior plus required exact verification exists for the declared scope.
- `EXISTING-PARTIAL`: an explicit subset is implemented/tested and remaining scope is named.
- `IMPLEMENTED / PARITY-OPEN`: production implementation exists, but release-level external/exact parity proof remains incomplete.
- `OPEN-ORACLE`: internal implementation/regression work exists, but an independent parity oracle is missing.
- `DEFERRED`: deliberately outside the currently claimed scope.

## 2. Target/cache/decode foundation

| Family | Current evidence | Status |
|---|---|---|
| target cache/profile identity | pinned build-241 profile/fingerprint safeguards | EXISTING |
| cache transport | `osrs-cache` transport/fingerprint tests | EXISTING |
| object/floor/map/loc/texture/sequence/var decode | M3 fixtures and decoder suites | EXISTING |
| malformed/random decoder input | unit + deterministic fuzz-smoke suites | EXISTING |
| map terrain/location encoding | strict inverse encoders with real-cache byte-exact coverage for representable data | EXISTING-PARTIAL |

## 3. Model construction and scene semantics

| Family | Current evidence | Status |
|---|---|---|
| model decode/selection/combine | M4 fixtures/tests | EXISTING |
| mirroring/winding/transforms | exact construction tests | EXISTING |
| static/non-flat/runtime ownership | M7/M8 lifecycle tests | EXISTING |
| loc placement/storage/footprints/heights | M6+ scene/placement suites | EXISTING |
| bridge/link-below and typed planes | semantic scene tests and golden fixtures | EXISTING |
| normals and cross-model reconciliation | M7 exact suites | EXISTING |
| target object lighting | M7 exact lighting suites | EXISTING |
| morph/contour/declared legacy animation | M8 suites | EXISTING within declared scope |
| broader skeletal execution | not claimed by current M8 scope | DEFERRED |

## 4. Terrain

| Contract | Current evidence | Status |
|---|---|---|
| shaped topology | exact shape gallery + pinned source | EXISTING |
| flat/shaped surface contract | terrain tests/golden scenes | EXISTING |
| floor HSL semantics | decode/post-decode exact tests | EXISTING |
| `TERRAIN-004` complete terrain builder | production port + real pinned-client differential oracle | EXISTING |

`TERRAIN-004` is no longer an implementation blocker. Commit `4e1757bded5952a18e51aef67981273dbf066c7c` ported the terrain loader/builder and reproduced real Lumbridge heights/settings/tile outputs/minimum planes/link-below state against the pinned client.

Placement-derived clipped shadow writes are integrated into cache-backed world construction before terrain-color finalization.

## 5. Plane/visibility semantics

| Contract | Current evidence | Status |
|---|---|---|
| source/storage/collision plane separation | core types + scene tests | EXISTING |
| bridge collision adjustment | pinned source + M6/M9 tests | EXISTING |
| structural link-below | exact four-plane fixture + scene tests | EXISTING |
| minimum-plane output | terrain builder oracle + renderer grouping | EXISTING |
| `originalPlane` / roof / visibility edge behavior | implementation exists in surrounding scene/render path, exhaustive release parity fixtures remain incomplete | IMPLEMENTED / PARITY-OPEN |

Do not describe min-plane grouping as unimplemented. Remaining work is edge-case parity around visibility/roof/original-plane semantics.

## 6. Deterministic identity and projection safety

| Family | Current evidence | Status |
|---|---|---|
| world/region/local coordinates | M6/M9 exact tests | EXISTING |
| semantic scene hash | pinned semantic golden hashes | EXISTING |
| render-origin rebase | renderer extraction tests | EXISTING |
| stale async build rejection | request/projection/presentation identity tests | EXISTING |
| pure terrain zone invalidation | exact 8x8 invalidation tests | EXISTING |
| Height invalidation | accepted dependency proof + conservative fallback | EXISTING |
| Object invalidation | accepted producer-output proof + conservative fallback | EXISTING |

## 7. Reference face/material/render preparation

| Contract | Current evidence | Status |
|---|---|---|
| face metadata preservation | renderer extraction regressions | EXISTING |
| software/client priority preparation | exact preparation tests | EXISTING |
| alpha/sentinel preparation | renderer preparation + draw-path coverage | EXISTING |
| suppressed `c == -2` admission | packed Reference geometry excludes suppressed faces | EXISTING |
| authored bias metadata | carried into Reference path; full distance-sensitive parity fixture remains | IMPLEMENTED / PARITY-OPEN |
| zone/level/min-plane grouping | Reference geometry/render path | EXISTING |

RustOSRS Reference priority behavior must continue to name the selected pinned software/client contract. Imported RuneLite GPU sorting behavior is a separate comparison path where relevant.

## 8. Reference textures/materials

| Contract | Current evidence | Status |
|---|---|---|
| full-width semantic texture IDs/material handles | renderer material preparation | EXISTING |
| canonical/explicit/projected UV preparation | exact renderer tests | EXISTING |
| cache sprite decode and 128x128 texture construction | production texture path | EXISTING |
| mipmapped texture array | production GPU path | EXISTING |
| cutout/transparency handling | Reference texture path + regressions | EXISTING |
| texture animation | deterministic render tick/path | EXISTING |
| Reference sampler state | production implementation exists; external/exact parity proof still needs release closure | IMPLEMENTED / PARITY-OPEN |

The old `GPU IMPLEMENTATION_REQUIRED` texture rows are obsolete. Commit `a3141d3edc6f7ce855c5a9f1378cf944f6c850e5` introduced the real cache texture construction/upload/sampler/cutout/animation path.

## 9. Reference GPU renderer

| Contract | Current state |
|---|---|
| headless/offscreen wgpu Reference renderer | EXISTING |
| reverse-Z path | EXISTING implementation; final parity fixtures remain part of M17 |
| opaque terrain/model rendering | EXISTING |
| alpha path | EXISTING implementation; complete mode/depth-write parity remains open |
| editor Reference viewport | EXISTING |
| internal Reference pixel regression | EXISTING |
| exhaustive depth/raster/bias parity | IMPLEMENTED / PARITY-OPEN |
| independent external visual oracle | OPEN-ORACLE |
| measured broad external 1:1 claim | NOT YET PERMITTED |

Commit `210d279e42bf94bcaa87da7982f3a90951abaf81` established the packed zone geometry and headless reverse-Z wgpu Reference renderer. Subsequent commits added the native editor viewport and real texture path.

## 10. Editor semantic authority

| Contract | Current state |
|---|---|
| semantic edit gateway | EXISTING |
| Paint/Height/Paste/Path/Object mutations | EXISTING through semantic authority |
| semantic Undo/Redo | EXISTING |
| one gesture <= one semantic history entry | EXISTING contract/tests |
| projection invalidation from mutation receipts | EXISTING |
| stale result rejection | EXISTING |
| read-only verified-export inspection | EXISTING |
| runtime ownership extraction | EXISTING-PARTIAL |

Remaining ownership work is `InteractionRuntime` behavior plus `ProjectRuntime`, `UiRuntime`, and `DiagnosticsRuntime` extraction.

## 11. Persistence/export

| Contract | Current state |
|---|---|
| project save/open | EXISTING |
| region-diff persistence | EXISTING |
| terrain/location encoders | EXISTING for representable values |
| staged verified existing-group map export | EXISTING |
| manifest-backed independent validation | EXISTING |
| extra map-group file preservation | EXISTING |
| isolated verified read-through | EXISTING, native-only |
| autosave/crash recovery | IMPLEMENTATION REQUIRED if retained in M16 scope |
| creation of new map groups | NOT IMPLEMENTED |
| hash/Whirlpool table-row validation | fail-closed, not supported |
| ordinary profile matching for edited cache | intentionally not supported by current design |

## 12. 117HD / enhanced profile

117HD is not a Reference parity target. Its proof model is data-resolution tests, shader/renderer regression tests, fixed-camera comparisons, and preservation of Reference behavior.

Implemented scope includes:

- render-profile/HDR foundation;
- 117HD data/material/override loaders;
- HD terrain/model extraction;
- ground blending/normals;
- environments/fog/directional lighting;
- directional shadows/filtering;
- water, flow/normal maps, foam, Fresnel, underwater tint/caustics;
- animated-loc HD material routing;
- material flow maps;
- parallax/displacement and displaced shadow lookup.

Remaining enhanced scope includes placed/dynamic lights, sky/day-night/star/moon/aurora systems, sky/reflection integration, remaining post/settings work, wind/character displacement, and remaining material/model visual edge cleanup.

## 13. CI certification state

Current CI has strong Tier A architecture/format/check/lint/WASM gates, Tier B full-workspace tests, and explicit Tier C semantic gates through M3-M9.

The implementation has advanced beyond the naming of Tier C. M10+ renderer/editor/export/117HD tests exist, but the release certification matrix should add explicit permanent later-stage gates instead of relying only on broad workspace coverage.

This is a **certification/tracking gap**, not evidence that M10+ production code does not exist.

## 14. Remaining Reference parity closure

Release-level Reference closure still requires:

1. exhaustive placement/side-grid shadow edge fixtures where coverage remains incomplete;
2. remaining original-plane/roof/visibility cases around the implemented min-plane path;
3. exact depth/raster and near/far authored-bias fixtures;
4. explicit alpha/depth-write/no-depth parity across relevant modes;
5. exact sampler/texture behavior verification against the selected reference path;
6. at least one independent external visual oracle;
7. measured external visual gates before a broad 1:1 claim;
8. first-class M10+ CI certification entries.

No renderer workaround may be used to hide an earlier semantic failure.
