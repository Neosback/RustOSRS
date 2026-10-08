# Semantic Parity Verification Matrix

Status: **Implementation tracking through M3**

This matrix maps every canonical semantic specification to its required verification family. Coverage status describes checked-in test/fixture implementation state, not source-evidence certainty.

## M3 P0 decode coverage

M3 closes cache transport and byte-to-canonical decoding prerequisites without claiming later scene/model/runtime semantics. The following checked-in artifacts are the concrete M3 verification anchors:

| M3 family | Exact verification artifact | Coverage |
|---|---|---|
| object definitions, including 32-bit model IDs and opcode-92 fallback/null | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, decoder unit tests | EXISTING |
| floor underlay/overlay decode and post-decode HSL/hue-multiplier state | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, `crates/osrs-core/src/floor_color.rs`, floor unit tests | EXISTING |
| varbit/varp definition inputs | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, vars unit tests | EXISTING |
| build-241 texture definition inputs | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, texture unit tests | EXISTING |
| build-241 sequence metadata | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, sequence unit tests | EXISTING |
| raw terrain tile stream | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, map unit tests | EXISTING |
| raw location smart/delta stream | `reference-fixtures/decode/m3-p0.txt`, `crates/osrs-cache/tests/m3_decode_fixtures.rs`, map unit tests | EXISTING |
| malformed/unknown decoder inputs | `crates/osrs-cache/tests/m3_fuzz_smoke.rs` plus decoder/reader unit tests | EXISTING |
| cache transport bounds, safe logical-file split, fingerprint/XTEA boundaries | `crates/osrs-cache/src/transport.rs` unit tests | EXISTING |

These rows verify the P0 acquisition/decode layer only. They do **not** promote `MORPH-001`, `TEXTURE-001`, `LOC-PLACEMENT-*`, model construction, scene placement, animation execution, or renderer behavior to `EXISTING` before their owning milestones.

## Placement

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `LOC-PLACEMENT-001` | loc types `0..22` + representative `>=12`; walls/decor x orientations | exact scene slot/type/arms/flags | PARTIAL |
| `LOC-PLACEMENT-002` | decor types `4..8` x orientation, wall present/absent, custom displacement | exact integer offsets/orientation flags | PARTIAL |
| `LOC-PLACEMENT-003` | square/non-square footprints x all orientations; sloped center samples | exact footprint, center, height inputs | REQUIRED |
| `LOC-PLACEMENT-004` | same qualifying definition through initial and pending-replacement paths | exact representation/path identity | REQUIRED |
| `LOC-PLACEMENT-005` | definition-driven collision/clipping/occlusion/wall metadata cases | exact side-effect state per promoted sub-contract | REQUIRED |
| `LOC-PLACEMENT-006` | floor decoration flat+slope cases | exact semantic Z, no implicit lift | REQUIRED |

## Model construction

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `MODEL-BUILD-001` | typed hit/miss; untyped type-10 combine; non-10 rejection; missing IDs | exact selected model IDs / `None` | REQUIRED |
| `MODEL-BUILD-002` | typed mirror truth table + untyped special case | exact vertices, indices, winding | REQUIRED |
| `MODEL-BUILD-003` | orientations, type-4 diagonal recenter, combined transform-order fixture | exact integer vertices + material substitutions | REQUIRED |
| `MODEL-BUILD-004` | `nonFlatShading` false/true path | exact semantic representation and cache state | REQUIRED |
| `MODEL-BUILD-005` | two instances share source; mutate only one | untouched source/instance exact hash | REQUIRED |

## Normals and lighting

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `NORMALS-001` | crafted smooth/flat triangles/quads + mirrored winding | exact normal components/magnitudes | PARTIAL |
| `NORMALS-002` | positive, negative, translated, hide=false, hide=true merge | exact merged normals + face render types | REQUIRED |
| `NORMALS-003` | dual-arm wall, wall/game neighbor, floor decor, plane-above | exact scene reconciliation/finalization state | REQUIRED |
| `NORMALS-004` | merge changes final lighting vs control | exact final lit values | REQUIRED |
| `LIGHTING-001` | known vector, flat/smooth, merged, ambient/contrast limits | exact integer lit/HSL output | PARTIAL |

## Morph, animation, contour

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `MORPH-001` | varbit, varp, in-range, fallback, null, footprint-changing transform | exact selected definition/absence | REQUIRED |
| `ANIMATION-001` | deterministic pose; footprint-changing morph; preserve/restart replacement | exact semantic pose/state ownership where specified | REQUIRED |
| `CONTOUR-001` | existing control + fast paths + clip thresholds + tile border + ownership | exact integer vertex Y output | PARTIAL |

## Terrain

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `TERRAIN-001` | all 13 shapes x 4 rotations | exact vertices, faces, color source, texture IDs | EXISTING |
| `TERRAIN-002` | flat diagonal, four distinct corners, sentinel, flat-vs-shaped choice | exact topology/representation | PARTIAL |
| `TERRAIN-003` | RGB/HSL boundaries, hue multiplier clamp, overlay defaults/opcodes/secondary | exact decoded integers/defaults | EXISTING |
| `TERRAIN-004` | full slope/11x11/overlay/jitter builder | exact end-to-end terrain color output | BLOCKED |

`TERRAIN-003` is linked to the M3 checked-in decode fixture family and exact `floor_color` unit vectors above. `TERRAIN-004` remains blocked until the builder source or an equivalent exact executable oracle is reproducibly pinned.

## Planes and bridges

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `PLANES-001` | API/domain guardrails + composed bridge fixture | exact distinct plane-domain values | REQUIRED |
| `PLANES-002` | encoded planes 0..3 x bridge bit on/off | exact collision plane + unchanged source plane | REQUIRED |
| `PLANES-003` | four-plane synthetic column + tagged game objects | exact tile identity/relinking/plane changes | REQUIRED |
| `PLANES-004` | renderer roof grouping toggled/replaced | semantic scene exact equality | PLANNED-GPU |

## Face/material semantics

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `FACE-001` | model retaining all optional face metadata into render extraction | exact field preservation | REQUIRED |
| `FACE-002` | priorities 0..11 around avg12/avg34/avg68 with 10/11 queues | exact reference face emission order | REQUIRED |
| `FACE-003` | alpha 0, ordinary values, sentinel, model+face transparency | exact interpreted metadata/order input | REQUIRED |
| `FACE-004` | coplanar authored-bias faces | exact bias preservation; visual stable order later | PLANNED-GPU |
| `TEXTURE-001` | explicit texture face, canonical fallback UV, projected dynamic case, animation handoff | exact/tolerance per owned stage | REQUIRED + PLANNED-GPU |

## Coordinates

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `COORD-001` | tile/local conversions, footprint centers, terrain positions | exact integer coordinates | PARTIAL |
| `COORD-002` | all loc orientations + special 256-JAU path | exact integer transformed vertices | REQUIRED |
| `COORD-003` | world/scene/local/model/render conversion and region-border fixture | exact semantic coordinates; explicit renderer conversion | REQUIRED |

## Renderer-policy verification mapping

These are not semantic specs but are merge-gating for the reference renderer milestone.

| Decision | Required verification | Coverage |
|---|---|---|
| ADR-0004 reverse-Z/culling | asymmetric landmark/front-face fixture; near/far depth; mirrored model cull | PLANNED-GPU |
| ADR-0004 authored bias | coplanar bias ordering under reference profile | PLANNED-GPU |
| ADR-0005 zone compilation | static extraction equality before/after zone rebuild; boundary dirty propagation | PLANNED-GPU |
| ADR-0005 ordered side path | exact `FACE-002` order consumed by renderer command stream | PLANNED-GPU |
| ADR-0006 Reference profile | deterministic fixed-scene image goldens | PLANNED-GPU |
| ADR-0006 Enhanced profile | lower parity unchanged; feature-specific visual checks | PLANNED-GPU |

## Editor-policy verification mapping

| Decision/area | Required verification | Coverage |
|---|---|---|
| ADR-0007 editor shell | viewport/device integration survives resize/recreate; panel layout does not affect document | PLANNED-EDITOR |
| ADR-0008 commands/history | apply/revert identity; transactions; randomized undo/redo; stale worker rejection | PLANNED-EDITOR |
| ADR-0009 persistence/export | atomic save, autosave generations, recovery, schema migration, export isolation | PLANNED-EDITOR |
| selection/picking | stable semantic identity across zone rebuild; stale pick rejection | PLANNED-EDITOR + PLANNED-GPU |
| terrain tools | same input stroke produces same semantic result independent of UI frame count | PLANNED-EDITOR |
| loc tools | preview/commit uses representable semantic tile/orientation only | PLANNED-EDITOR |

## Exit rule

A milestone cannot claim a spec as implemented merely because its row exists here. The coverage column must advance to `EXISTING`, with the actual test/fixture linked from the implementation-era version of this matrix.
