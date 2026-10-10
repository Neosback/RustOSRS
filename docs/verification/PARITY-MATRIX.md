# Semantic Parity Verification Matrix

Status: **Implementation tracking through M7 milestone exit audit**

This matrix maps every canonical semantic specification to its required verification family. Coverage status describes checked-in production test/fixture implementation state, not source-evidence certainty.

## M3 P0 decode coverage

M3 closes cache transport and byte-to-canonical decoding prerequisites without claiming later scene/model/runtime semantics.

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

These rows verify the P0 acquisition/decode layer only. They do **not** promote `MORPH-001`, `TEXTURE-001`, `LOC-PLACEMENT-*`, scene placement, animation execution, or renderer behavior to `EXISTING` before their owning milestones.

## M4 P0/P1 model coverage

M4 closes target-era ModelData decode plus exact pre-GPU object model selection, raw mirroring, multi-model combination, and instance transforms.

| M4 family | Exact verification artifact | Coverage |
|---|---|---|
| build-241 `FF FD` / `FF FE` ModelData decode, topology, face/material metadata, skins/skeletal inputs, format identity, provenance | `crates/osrs-cache/src/decode/model.rs`, decoder unit tests, ignored exhaustive `crates/osrs-cache/tests/m4_target_probe.rs` | EXISTING |
| exact model selection and multi-model combine | `reference-fixtures/model/m4-p0-p1.txt`, `crates/osrs-core/src/model_construction.rs` unit tests | EXISTING |
| raw mirror selection, geometry, winding, and distinct mirrored cache variant | `reference-fixtures/model/m4-p0-p1.txt`, model-construction tests, `crates/osrs-cache/src/object_model.rs` | EXISTING |
| exact instance transform order and integer coordinate behavior | `reference-fixtures/model/m4-p0-p1.txt`, `crates/osrs-core/tests/m4_instance_transform.rs` | EXISTING |
| malformed/truncated/random target-model bytes | `crates/osrs-cache/tests/m4_model_fuzz_smoke.rs` | EXISTING |
| M4 ownership subset: immutable raw/cache sources across mirror, combine, recolor/retexture, orientation, resize, translation | repository/model-construction tests and `m4_instance_transform.rs` | PARTIAL |

The pinned OpenRS2 2727 target sweep decoded all `62,043` build-241 model groups through the production repository path: `35,103` `FF FD`, `26,940` `FF FE`, zero `FF FF`, zero legacy, zero empty groups, and zero multi-file model groups.

M7 closes `MODEL-BUILD-004` with the exact `nonFlatShading` representation/cache split and scene-local finalization path. `MODEL-BUILD-005` remains `PARTIAL` overall because contouring and animation pose are later-owned, while the M7 scene-normal ownership and source-immutability portion is now exercised. `FACE-001` remains `REQUIRED` until render extraction proves end-to-end optional face metadata preservation.

## M5 reference-fixture infrastructure coverage

M5 adds source-pinned normalized fixtures, an exact offline runner, candidate-only regeneration, and an indexed historical evidence catalog. Evidence for later-owned semantics does not promote those production semantics to `EXISTING` by itself.

| M5 fixture/evidence family | Exact verification artifact | Execution / coverage |
|---|---|---|
| typed model selection | `model.selection.typed_exact.orientation_4`; `reference-fixtures/manifest/model-selection-typed-orientation-4.yaml` | semantic / EXISTING |
| mirror geometry and winding | `model.mirror.geometry_winding`; `reference-fixtures/manifest/model-mirror-geometry-winding.yaml` | semantic / EXISTING |
| type-4 transform order | `model.transform.type4_order`; `reference-fixtures/manifest/model-transform-type4-order.yaml` | semantic / EXISTING |
| base normals smooth/flat | `normals.base.smooth_triangle`, `normals.base.flat_triangle`; `reference-fixtures/manifest/normals-base-*.yaml` | evidence-only; production promotion occurs in the M7 suite |
| cross-model normal merge controls | `normals.merge.coincident_triangle.hide_false`, `normals.merge.coincident_triangle.hide_true`, `normals.merge.translated_negative`; `reference-fixtures/manifest/normals-merge-*.yaml` | evidence-only; production promotion occurs in the M7 suite |
| four-plane link-below relinking | `planes.link_below.four_plane_column`; `reference-fixtures/manifest/planes-link-below-four-plane-column.yaml` | semantic / EXISTING |
| priority thresholds and priority-10/11 queues | `priority.all_0_11.threshold_crossing`; `reference-fixtures/manifest/priority-all-0-11-threshold-crossing.yaml` | evidence-only; `FACE-002` remains REQUIRED |
| historical deob evidence index | `reference-fixtures/historical/deob_golden.index.json`; `scripts/test_deob_golden_index.py` | indexed historical evidence; later production rows retain their existing status |

At M5 exit, the repository-wide normalized runner validated all ten canonical YAML fixtures with three production semantic executors and seven evidence-only fixtures. M6 Checkpoint 3 promotes `planes.link_below.four_plane_column` to production semantic execution, so the current runner exercises four semantic fixtures while six remain evidence-only. M7 keeps the normal fixtures source-pinned and additionally exercises the production normal algorithms through dedicated exact Rust tests.

The historical index exposes stable fixture IDs for `terrain.shape_gallery.all_13x4`, `contour.synthetic.flat_slope`, `lighting.synthetic_triangle.loc_rig`, `placement.wall_types.orientation_matrix`, `placement.decor_types.orientation_matrix`, `placement.floor_type22.storage`, and `placement.game_object.footprint_and_capacity`.

## M6 semantic scene coverage

M6 Checkpoints 1 through 5 establish exact terrain topology, typed plane semantics, structural link-below processing, loc placement/storage, and definition-driven side-effect planning. Checkpoint 6 closes the remaining fully owned semantic gaps with source-pinned placement-height sampling, explicit terrain sentinel interpretation, composed golden semantic scenes, and a permanent Tier C `osrs-scene` gate.

| M6 semantic family | Exact verification artifact | Coverage |
|---|---|---|
| fixed scene-layer insertion and query for loc types `0..22` plus representative `>=12` | `crates/osrs-scene/tests/m6_scene_insertion.rs`, `crates/osrs-scene/src/placement.rs` unit tests | EXISTING |
| game-object full-footprint insertion, shared identity, edge masks, five-object capacity, atomic rejection, exact sloped-height sampling | `crates/osrs-scene/src/scene.rs`, `crates/osrs-scene/src/placement_height.rs`, `crates/osrs-scene/tests/m6_scene_insertion.rs`, `crates/osrs-scene/tests/m6_exit_golden_scenes.rs` | EXISTING |
| flat/shaped terrain ownership, exact flat diagonal, NE sentinel skip contract, shaped first-face sentinel contract | `crates/osrs-scene/src/terrain.rs`, `crates/osrs-scene/src/terrain_contract.rs`, `crates/osrs-scene/tests/m6_exit_golden_scenes.rs` | EXISTING |
| floor-decoration storage at supplied flat and synthetic slope heights with no implicit lift | `crates/osrs-scene/tests/m6_scene_insertion.rs` | EXISTING |
| definition-driven collision operation selection, including floor-decoration special case and definition-footprint collision for visually 1x1 type `9` | `crates/osrs-scene/src/side_effects.rs` unit tests, pinned `FriendSystem.addObjects` | EXISTING at operation-plan level |
| definition clipping/model-clipping/ground-obstruction inputs and audited wall-displacement metadata retained for deterministic later side-grid mutation | `DefinitionSideEffectInputs`, `SceneSideEffectPlan`, side-effect planner unit tests | PARTIAL; concrete shadow/occlusion/collision bit-grid mutations remain intentionally unimplemented |
| multi-tile game-object plane identity under link-below mutation | `crates/osrs-scene/src/scene.rs` unit tests | EXISTING |
| world/scene/region/local coordinate composition across a 64-tile region border | `crates/osrs-core/src/coords.rs`, `crates/osrs-scene/tests/m6_exit_golden_scenes.rs` | EXISTING semantic side |

### M6 required golden semantic scenes

| Required scene | Exact artifact | Status |
|---|---|---|
| terrain shape gallery | `crates/osrs-scene/tests/m6_terrain_shape_gallery.rs`; historical ID `terrain.shape_gallery.all_13x4` | EXISTING |
| wall/decor orientation scene | `m6_exit_golden_scenes::golden_wall_and_decor_orientation_scene_survives_semantic_storage`; historical placement matrix IDs | EXISTING |
| four-plane bridge column | `m6_exit_golden_scenes::golden_four_plane_bridge_column_relinks_structurally`; normalized fixture `planes.link_below.four_plane_column` | EXISTING |
| region-border coordinate fixture | `m6_exit_golden_scenes::golden_region_border_fixture_preserves_world_scene_and_local_identity` | EXISTING |

## M7 normals, lighting, and scene finalization coverage

M7 closes production base normals, cross-model normal reconciliation, the scene neighbor traversal/finalization lifecycle, exact final object lighting, initial `nonFlatShading` ownership, and constructed-model identity threading before renderer work begins.

| M7 semantic family | Exact verification artifact | Coverage |
|---|---|---|
| exact smooth/flat base normals, quad accumulation, winding reversal, and source immutability | `crates/osrs-core/src/normals.rs` unit tests; `crates/osrs-reference/tests/m7_base_normals.rs` | EXISTING |
| translated cross-model merge, no-match control, hide=false/true, matched-face type `2`, repeated accumulation, and source immutability | `crates/osrs-core/src/normals.rs` unit tests; `crates/osrs-reference/tests/m7_normal_merge.rs` | EXISTING |
| dual-arm boundary, boundary/game neighbor, floor-decoration hiding, plane-above height delta, and ModelData close state | `crates/osrs-scene/tests/m7_scene_normal_reconciliation.rs` | EXISTING |
| initial flat/non-flat static entity split, cached ModelData ownership, scene-local copies, reconciliation-before-lighting, and retained loc lighting parameters | `crates/osrs-reference/tests/m7_static_entity_lifecycle.rs`; `crates/osrs-reference/tests/m7_initial_scene_finalization.rs` | EXISTING |
| exact known-vector lighting, flat/smooth, textured/untextured, merged-normal precedence, alpha sentinels, HSL/lightness clamps, ambient/contrast extremes | `crates/osrs-reference/tests/m7_lighting.rs`; `crates/osrs-core/src/lighting.rs` unit tests | EXISTING |
| constructed-model identity retained through working/lit/cache lifecycle and genuine two-source M4 assembly admission | `crates/osrs-reference/tests/m7_identity_threading.rs`; `crates/osrs-reference/tests/m7_composite_admission.rs` | EXISTING |

The permanent Tier C gate in `.github/workflows/ci.yml` explicitly runs the M7 core normal/lighting tests, M7 reference semantic tests, and the M7 scene reconciliation test. This prevents future path-based or test-selection changes from silently dropping the milestone's owning semantic suite.

## Placement

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `LOC-PLACEMENT-001` | loc types `0..22` + representative `>=12`; walls/decor x orientations; indexed evidence IDs `placement.wall_types.orientation_matrix`, `placement.decor_types.orientation_matrix`, `placement.floor_type22.storage` | exact scene slot/type/arms/flags | EXISTING |
| `LOC-PLACEMENT-002` | decor types `4..8` x orientation, wall present/absent, custom displacement; indexed evidence ID `placement.decor_types.orientation_matrix` | exact integer offsets/orientation flags | EXISTING |
| `LOC-PLACEMENT-003` | square/non-square footprints x all orientations; sloped center samples; indexed evidence ID `placement.game_object.footprint_and_capacity` | exact footprint, center, height inputs | EXISTING |
| `LOC-PLACEMENT-004` | same qualifying definition through initial and pending-replacement paths | exact representation/path identity | PARTIAL; M7 initial path EXISTING, pending/live replacement still REQUIRED |
| `LOC-PLACEMENT-005` | definition-driven collision/clipping/occlusion/wall metadata cases | exact side-effect state per promoted sub-contract | PARTIAL |
| `LOC-PLACEMENT-006` | floor decoration flat+slope cases; indexed evidence ID `placement.floor_type22.storage` | exact semantic Z, no implicit lift | EXISTING |

`LOC-PLACEMENT-003` is closed by the source-pinned rotated-footprint midpoint sampler, including the exact scene-edge anchor/anchor+1 fallback and Java-style four-sample average. `LOC-PLACEMENT-005` remains `PARTIAL` by the explicit M6 roadmap exception: collision-operation selection and wall-displacement ownership are implemented, while narrower private collision/shadow/occlusion bit-grid formulas remain unpromoted. M7 closes the initial `LOC-PLACEMENT-004` side by proving qualifying `nonFlatShading` ModelData survives into normal reconciliation and final lighting. The pending/live replacement half remains later-owned and keeps the full row `PARTIAL`.

## Model construction

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `MODEL-BUILD-001` | typed hit/miss; untyped type-10 combine; non-10 rejection; missing IDs; fixture `model.selection.typed_exact.orientation_4` | exact selected model IDs / `None` | EXISTING |
| `MODEL-BUILD-002` | typed mirror truth table + untyped special case; fixtures `model.selection.typed_exact.orientation_4`, `model.mirror.geometry_winding` | exact vertices, indices, winding | EXISTING |
| `MODEL-BUILD-003` | orientations, type-4 diagonal recenter, fixture `model.transform.type4_order` | exact integer vertices + material substitutions | EXISTING |
| `MODEL-BUILD-004` | `nonFlatShading` false/true path | exact semantic representation and cache state | EXISTING |
| `MODEL-BUILD-005` | two instances share source; mutate only one | untouched source/instance exact hash | PARTIAL; M7 scene-normal ownership EXISTING, contour/animation portions later-owned |

## Normals and lighting

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `NORMALS-001` | crafted smooth/flat triangles/quads + mirrored winding; M5 evidence `normals.base.smooth_triangle`, `normals.base.flat_triangle` | exact normal components/magnitudes | EXISTING |
| `NORMALS-002` | positive, negative, translated, hide=false, hide=true merge; M5 evidence `normals.merge.coincident_triangle.hide_false`, `normals.merge.coincident_triangle.hide_true`, `normals.merge.translated_negative` | exact merged normals + face render types | EXISTING |
| `NORMALS-003` | dual-arm wall, wall/game neighbor, floor decor, plane-above | exact scene reconciliation/finalization state | EXISTING |
| `NORMALS-004` | merge changes final lighting vs control | exact final lit values | EXISTING |
| `LIGHTING-001` | known vector, flat/smooth, merged, ambient/contrast limits; indexed historical evidence `lighting.synthetic_triangle.loc_rig` | exact integer lit/HSL output | EXISTING |

M7 satisfies the roadmap hard exit gate: every normal/lighting matrix row is now `EXISTING` through production exact tests. The M5 normalized normal fixtures remain evidence-only as fixture-runner artifacts, but their semantics are independently exercised against production code by the dedicated M7 tests.

## Morph, animation, contour

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `MORPH-001` | varbit, varp, in-range, fallback, null, footprint-changing transform | exact selected definition/absence | REQUIRED |
| `ANIMATION-001` | deterministic pose; footprint-changing morph; preserve/restart replacement | exact semantic pose/state ownership where specified | REQUIRED |
| `CONTOUR-001` | existing control + fast paths + clip thresholds + tile border + ownership; indexed historical evidence `contour.synthetic.flat_slope` | exact integer vertex Y output | PARTIAL |

## Terrain

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `TERRAIN-001` | all 13 shapes x 4 rotations; indexed historical evidence `terrain.shape_gallery.all_13x4` | exact vertices, faces, color source, texture IDs | EXISTING |
| `TERRAIN-002` | flat diagonal, four distinct corners, sentinel, flat-vs-shaped choice | exact topology/representation | EXISTING |
| `TERRAIN-003` | RGB/HSL boundaries, hue multiplier clamp, overlay defaults/opcodes/secondary | exact decoded integers/defaults | EXISTING |
| `TERRAIN-004` | full slope/11x11/overlay/jitter builder | exact end-to-end terrain color output | BLOCKED |

`TERRAIN-002` is closed by exact flat-paint diagonal ownership, explicit flat-vs-shaped semantic representation, and source-pinned sentinel tests. `TERRAIN-004` remains blocked until the builder source or an equivalent exact executable oracle is reproducibly pinned; M6 does not infer that algorithm from plausible historical prose.

## Planes and bridges

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `PLANES-001` | API/domain guardrails + composed bridge fixture | exact distinct plane-domain values | EXISTING |
| `PLANES-002` | encoded planes 0..3 x bridge bit on/off | exact collision plane + unchanged source plane | EXISTING |
| `PLANES-003` | four-plane synthetic column + tagged game objects; fixture `planes.link_below.four_plane_column` | exact tile identity/relinking/plane changes | EXISTING |
| `PLANES-004` | renderer roof grouping toggled/replaced | semantic scene exact equality | PLANNED-GPU |

`planes.link_below.four_plane_column` is source-pinned to `Scene.setLinkBelow` and executes against the M6 production `osrs-scene` relinking path. Renderer roof grouping remains separate policy under `PLANES-004`.

## Face/material semantics

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `FACE-001` | model retaining all optional face metadata into render extraction | exact field preservation | REQUIRED |
| `FACE-002` | priorities 0..11 around avg12/avg34/avg68 with 10/11 queues; M5 evidence `priority.all_0_11.threshold_crossing` | exact reference face emission order | REQUIRED |
| `FACE-003` | alpha 0, ordinary values, sentinel, model+face transparency | exact interpreted metadata/order input | REQUIRED |
| `FACE-004` | coplanar authored-bias faces | exact bias preservation; visual stable order later | PLANNED-GPU |
| `TEXTURE-001` | explicit texture face, canonical fallback UV, projected dynamic case, animation handoff | exact/tolerance per owned stage | REQUIRED + PLANNED-GPU |

`priority.all_0_11.threshold_crossing` is intentionally `evidence_only`. The renderer-owned production priority executor does not exist yet, so `FACE-002` remains `REQUIRED`.

## Coordinates

| Spec | Required verification | Comparison | Coverage |
|---|---|---|---|
| `COORD-001` | tile/local conversions, footprint centers, terrain positions | exact integer coordinates | PARTIAL |
| `COORD-002` | all loc orientations + special 256-JAU path; fixture `model.transform.type4_order` | exact integer transformed vertices | EXISTING |
| `COORD-003` | world/scene/local/model/render conversion and region-border fixture | exact semantic coordinates; explicit renderer conversion | EXISTING semantic side; renderer conversion PLANNED-GPU |

M6 closes the semantic side of `COORD-003` with a composed region-border fixture that crosses the 63/0 region boundary, round-trips world and scene coordinates, preserves exact 128-unit local centers, and stores both objects without changing world identity. Camera/view/clip conversion remains renderer-owned.

## Renderer-policy verification mapping

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

A milestone cannot claim a spec as implemented merely because source evidence or a row exists here. Coverage advances to `EXISTING` only when the owning production implementation is exercised by the required exact test/fixture.
