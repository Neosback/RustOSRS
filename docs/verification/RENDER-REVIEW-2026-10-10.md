# Render review — tiles, walls, decorations, GPU state (2026-10-10)

Scope: the user asked for a source-level review of tile rendering (all shapes), wall merging and
normals, wall decorations (offsets, mirrors), object placement/scale/multi-tile shading, and the
GPU state/shader configuration, against the pinned client (`melxin/runelite@1ad572d7`, used for
placement, lighting and terrain) and `runelite-master` (build-241-accurate GPU plugin).

## Method

* Terrain: pinned-client terrain oracle (`tools/deob-harness/run-terrain-oracle.sh`) run on extra
  windows and diffed against `osrs_world::oracle_dump` through
  `crates/osrs-reference/tests/terrain_oracle_diff.rs`. The test now accepts
  `RUSTOSRS_ORACLE_IN` / `RUSTOSRS_ORACLE_OUT` to diff any window.
  The oracle JVM sometimes does not exit after writing its (complete) output; kill it.
* Placement / normal merge / lighting: line-by-line reading of `FriendSystem.addObjects`,
  `Scene.newWallDecoration`, `WallDecoration.method6262`, `Scene.method5576/5585/5586/5587`,
  `ModelData.method5262`, `ObjectComposition.getEntity/getModelData`, `DynamicObject` against
  `osrs-world::loc_stage`, `osrs-scene::normal_finalization`, `osrs-core::normals` and
  `osrs-core::model_construction`.
* GPU: `SceneUploader`, `ModelUploader.computeFaceUvs`, `Zone.renderOpaque/renderAlpha/flush`,
  `GpuPlugin` state setup, `vert.glsl`/`frag.glsl`, `GpuPluginConfig` against `osrs-render`.

## Verified equal to the reference

| Area | Evidence |
| --- | --- |
| Terrain loader + builder, every shape/rotation, skipped faces, bridges, min planes | Oracle diff passes byte-for-byte on Lumbridge (committed fixture), Yanille, Varrock, Falador (see the window table below). |
| Normal merge traversal (`method5585/5586/5587`) | Plane range `p..p+1`, x-start decrement, corner exclusion, translation formulas, boundary primary/secondary order, floor-decoration neighbourhood: identical. |
| `ModelData.method5262` | Bounds constants, base-normal contributions, match test, `>= 3` matched pairs hide rule: identical. |
| Instance transforms (mirror, type-4 `256` + `(45,0,-45)`, rotate, recolor, retexture, resize, offset) | Identical order to `getModelData`. |
| Placement constants (`Tiles.field800..805`), wall-decoration nudge in `method6262` | Identical. |
| Loc plane vs collision plane for bridges | Only the collision map plane is lowered; scene plane unchanged. Matches `loc_stage`. |
| Terrain/model upload: vertex order, UVs, `-1/-2` face colours, bias, alpha split | Identical to `SceneUploader`/`ModelUploader`. |
| Pipeline state: reverse-Z, `GREATER`, back-face cull CCW, blend `SRC_ALPHA,1-SRC_ALPHA / ONE,ONE` | Identical to `GpuPlugin`. |
| Vertex/fragment shader maths (hsl→rgb, bias, texture anim, cutout, texture lighting) | Identical to `vert.glsl`/`frag.glsl`. |

## Differences found and fixed

1. **Missing `Scene.method5576`** (retroactive wall-decoration rescale). When a type-0 / type-2
   boundary, a type-9 diagonal wall or any type >= 12 object with `decoration_displacement != 16`
   is placed on a tile that already holds a wall decoration, the client re-sets the decoration's
   offsets to `displacement * base / 16`. Not ported before; 4-118 decorations per window were
   mis-offset. Fixed in `loc_stage.rs` (`LocStageStats::decorations_rescaled`).
2. **Wall-decoration slot overwrite.** `newWallDecoration` replaces the tile's previous wall
   decoration; only boundary and floor-decoration replacement was modelled. Fixed in `finish`.
3. **Alpha pass depth state.** `Zone.flush` draws alpha with `glDepthMask(false)`; ours wrote
   depth. Alpha is now drawn without depth writes, after all opaque geometry, level by level,
   farthest zone first.
4. **Shading mode.** RuneLite's `smoothBanding` uniform is inverted (`config ? 0 : 1`); the
   default config ("Remove color banding" on) shades with vertex RGB, while ours interpolated HSL
   and converted per pixel (the banded CPU look). Default now matches RuneLite; the banded mode is
   available (`FrameParams::remove_color_banding`, "smooth shading" checkbox).
5. **MSAA.** RuneLite defaults to 2x MSAA; the scene pass is now 4x MSAA resolved into the
   viewport target.
6. **Whole-region build failure.** A non-flat wall decoration (`ModelData` the client never
   reconciles, so it draws nothing) left the finalizer "pending" and aborted the whole region.
   Now omitted exactly as the client does. All 2,934 existing map squares build.

## The coastline gaps (user screenshot, Yanille east coast)

Not a renderer bug. The pinned client itself produces skipped faces there: shaped water-overlay
tiles (overlay 442, shapes 1-5) whose underlay id is 0, so the underlay half of the tile has no
colour (`ca=12345678` in the oracle output for tile `0 41 45` of the Yanille window). Our output
matches the oracle line for line. In RuneLite those faces are skipped too and the clear colour
shows through; vanilla/RuneLite default the sky colour to black, and the optional RuneLite
"Skybox" plugin supplies an old-school per-region sky colour. The editor now clears to black by default with a colour picker; porting `skybox.txt` region
colours is not done.

## Wall-merge seams (objects 1904 / 1907, Yanille garden wall)

Third-party write-up claim: the client's normal merge (`ModelData.method5262`) requires
strict `x`, `y` and `z` equality, and some adjacent wall pieces differ by 1 unit in `y`, so they
never merge (visible lighting seam, coincident end caps left visible).

Verified on the real cache (`examples/merge_probe.rs`, `examples/loc_finder.rs`): object 1904
has `translation.y = 1`, 1907 has `0`; the pair at (2585,3085) type 9 / (2584,3085) type 1 has 18
x/z-coincident vertices, **0** strict-equal, 9 within 2 units. The pinned client's condition is
`var11 == var1.verticesY[var14]` (strict), so the reference also fails to merge these pieces and
shows the same seam; our port is identical. The write-up's 2-unit tolerance is a deliberate
deviation from the client, not an exactness fix. It is not applied. If a "visual tolerance"
mode is wanted it should be an opt-in editor setting (parameter on
`merge_model_normals`), off by default.

Same write-up, diagonal decorations (shape 8 on diagonal wall 1902): type-9 diagonal walls are
*game objects*, so `getBoundaryObjectTag` is 0 and the decoration displacement is the default
`8`; offsets `8 * (+-1, +-1)` and the second entity at orientation `+2` match our plan. The
"plane bias / roof pull" section is not in RuneLite (face bias only, levels share one depth
buffer), so it is intentionally not applied.

## Update: seam tolerance option and plane visibility

* The strict `y` match is the pinned client's code, but a project the user validated against the
  game shows these seams are not visible in OSRS, so the January deob is probably behind build 241
  here (or the offsets are applied elsewhere). `TerrainPresentation::wall_merge_tolerance`
  (default 0 in the library) adds an opt-in vertical tolerance; the editor enables 2 by default
  ("merge wall seams" checkbox, rebuilds loaded regions). This is a **non-reference** setting.
* Plane visibility was wrong: `Scene.draw` iterates every plane from `minPlane` and draws a tile
  when `tile.minPlane <= Scene_plane` (and RuneLite uploads `TILE_FLAG_VIS_BELOW` tiles of upper
  levels into the level-0 draw). We additionally required `level <= plane`, hiding e.g. the
  Lumbridge battlements. Now only `min_plane <= plane` is tested.
* Draw order: opaque order only matters for exact depth ties (first drawn wins under `GREATER`)
  and for alpha. RuneLite emits per tile (paint, model, wall, decorative, ground, game objects);
  we emit all terrain of a zone then its locs. Not changed.

## Open items (not yet matching the reference)

* **Alpha ordering.** RuneLite additionally sorts alpha models by distance and counting-sorts
  each close model's faces back to front (`Zone.renderAlpha`). We only order zones.
* **Face-priority ordered path** for animated/dynamic models (`ModelUploader.uploadSortedModel`)
  is not ported; dynamic objects use the static face order.
* **Roof/plane visibility.** RuneLite draws levels `[minLevel, maxLevel]` with a hidden-roof
  set; we draw `level <= plane && min_plane <= plane`.
* **Animation:** `randomizeAnimStart` / `deferAnimChange` object flags are not applied; sequences
  without a loop-back step restart at frame 0 (client resets them); skeletal sequences are static.
* **`ground_raise` (opcode 96) and `full_recolor` (opcode 42)** semantics are still unverified:
  `runelite-master` only stores them; no client in the repo uses them.
* **Sky colour / skybox plugin** (above); fog is not implemented (RuneLite default is off).
* No whole-scene oracle for objects: the January deob cannot decode build-241 object
  definitions, so placement/normal/lighting parity rests on the source reading above.

## Work log: extra terrain oracle windows

Windows are regenerated on demand with `export_terrain_oracle_input` + `run-terrain-oracle.sh`;
the committed fixture is unchanged.

| Window (scene base) | Result |
| --- | --- |
| Yanille east coast (2592,3048) | pass |
| Varrock (3112,3368) | pass |
| Falador (2920,3304) | pass |
| Karamja (2792,3112) | pass |
