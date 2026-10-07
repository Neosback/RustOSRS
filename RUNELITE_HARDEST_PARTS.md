# 10 Hardest Parts of the Rust/wgpu Port — Ranked by Risk, With Research Tasks

Ranked by (blast radius if wrong) × (effort to verify). Each item: why it's
hard, exactly what RuneLite tells us (pins), and the research to do **now** —
before renderer code — with acceptance criteria. Items 1 and 3 contain newly
extracted math (lighting bake, camera projection) not previously documented.

Conventions: JAU = 2048-unit angle. All deob paths are melxin
`runescape-client/src/main/java/`; GPU paths are runelite-master.

---

## H1. Model lighting-bake parity (fixed-point Gouraud) — colors wrong everywhere if off

**Why hard:** every model vertex color on the GPU path is *pre-lit* by
`ModelData.toModel` in integer arithmetic. A float reimplementation will drift
by ±1 lightness step on thousands of faces — invisible per face, visibly wrong
in aggregate (washed or muddy models). There are two rigs, four face-type
branches, and two different light-application functions; missing a branch
(e.g. textured-face lightness-only) breaks whole material classes.

**What RuneLite gives us** (`ModelData.java:2095-2278`, fully readable):
- Rigs: loc path `toModel(ambient+64, contrast+768, sun=(-50,-10,-50))`;
  default path `method5285() = toModel(128, 43690, 0, -1, 0)` (sun straight
  down). Contrast is pre-scaled ×25 at decode (R20).
- Sun normalize: `len = √(x²+y²+z²)` (double!), `k = len*contrast >> 8`.
- Smooth untextured (type 0): per-vertex
  `light = (sun·n)/(k·magnitude) + ambient` → `method5263(hsl, light)`.
- Flat untextured (type 1): `light = (sun·faceN)/(k/2 + k) + ambient` — note
  the different denominator (no magnitude term).
- Type 3 → flat color 128; anything else → `faceColors3 = -2` (skip face).
- Alpha −2 forces type 3; alpha −1 forces type 2.
- Textured faces: `method5264(light)` — **bare lightness, no hue** (matches
  `frag`'s `fHsl/127` light path).
- `method5263(hsl, light)`: `lum' = clamp(hsl&127 * light >> 7, 2, 126)`,
  return `(hsl & 65408) + lum'` — same shape as terrain `method2086`.

**Research now:**
1. Transcribe the loop to Rust operating on `i32` with Java `/` truncation
   semantics (negative numerator truncation differs from Rust `/` — verify each
   division site; `sun·n` can be negative). Oracle available:
   `reference-fixtures/deob_golden.txt` (`tolit` line = loc-rig output on a
   synthetic triangle; extend `Dumper` T7 with your own fixtures as needed).
2. Acceptance: brute-force differential test over synthetic normals × sun
   vectors × ambient/contrast pairs against hand-computed Java-semantics
   values; zero mismatches on lightness bytes across ≥10⁵ samples.
3. Decide `f64` vs `i32` for the `sqrt` line (only float op in the path) and
   record the choice.

## H2. CPU priority-sort parity (zsort + 12-bucket interleave)

**Why hard:** the sort is order-dependent across three coupled structures
(depth buckets, priority buckets, distance averages); a subtle bug (e.g. wrong
injection boundary for pri-10/11) only shows on specific content (spell FX over
railings) and looks like "random flicker" — nearly undebuggable visually.

**What RuneLite gives us:** `ModelUploader.java` (full) + `priority_render.glsl`
`priority_map`/`count_prio_offset` (same algorithm, 0–17 adjusted buckets).
Static order is baked; dynamic re-sorts per frame against the camera.

**Research now:**
1. Write the algorithm as pure functions
   (`bucketize(faces) → order`, `priority_map(p, dist, ...) → adjusted`) with
   the `avg12/avg34/avg68` injection rules stated as a table, not prose.
2. Acceptance: property tests — total order preserved within equal adjusted
   priority; pri-10 faces inject exactly at the pri-0/avg12 boundary on crafted
   inputs; GLSL `priority_map` and Rust agree on ≥10⁴ random (priority,
   distance) pairs.

## H3. Camera/projection parity (the mirroring trap)

**Why hard:** the native view math is injected (not in mixins), so a port
guesses the convention — and a mirrored map still "looks right" until walls,
text, and rotations are all on wrong edges. This is the single most likely
source of a late, catastrophic rework.

**What RuneLite gives us** (`IntProjection.java:63-177`, fully readable):
- Rotation order (applied to camera-relative coords): **yaw first, then pitch**:
  `x' = yawSin·z + yawCos·x`, `z' = yawCos·z − yawSin·x`;
  `y' = pitchCos·y − z'·pitchSin`, `z'' = pitchSin·y + pitchCos·z'`.
- Sines come from `Rasterizer3D.field2798/field2791[2048]` (JAU-indexed float
  tables) — same values as `Perspective.SINEF/COSINEF`; reuse them.
- Cull: any `z'' < 50` rejects (same 50 as the GPU sorter's `p[2] < 50`).
- Screen: `sx = clipMidX + x'·zoom/z''`, `sy = clipMidY + y'·zoom/z''`.
- Software depth curve `1.0100503 + 150.75377/((z−75)·1.0100503)` (near plane
  heritage: 75/50) — informational; the GPU path uses its own projection.
- Compass: JAU 0 = south, 512 = west, 1024 = north, 1536 = east.

**Research now:**
1. Implement the exact yaw-then-pitch chain + 50-unit cull + zoom divide as
   `editor_core::camera::project_jagex`, unit-tested against hand-computed
   vectors (axis-aligned cases first: yaw 0/512/1024/1536, pitch extremes).
2. Acceptance: asymmetric-landmark render test (Lumbridge Castle) matches
   orientation bands before any further renderer work; document the locked
   handedness (which world axis maps to −z view) in the test.

## H4. HSL pipeline rounding parity (three arithmetics, one color)

**Why hard:** the same color passes through `method817` (8-bit→packed with
highlight desaturation), integer `method2086/5263` (lightness replace), and
`hslToRgb.glsl` (f32). Boundary values (sat/light near halving thresholds and
the 2/126 clamps) can disagree by a step between paths.

**What RuneLite gives us:** all three functions verbatim (deob guide §4, R12,
`hsl_to_rgb.glsl` 84 L).

**Research now:**
1. Exhaustive differential test over the full 64×8×128 packed-HSL space through
   both integer and float paths; catalog every mismatch (expect clusters at
   lightness 179/192/217/243 sat-halving and the 2/126 clamps).
2. Acceptance: written rounding policy per site (which path is truth for terrain
   vs models vs shader), encoded as unit tests, not comments.

## H5. Terrain triangulation parity (shapes × rotations)

**Why hard:** 13 shapes × 4 rotations with three different index-remap rules and
two color sets; one wrong remap cracks every shoreline in one orientation only.

**What RuneLite gives us:** tables verbatim + constructor pseudocode + the
minimap oracle pattern (`tileShape2D[shape][rotationTable[i]] != 0 → overlay`),
plus the new precisions (set1 = underlay, set2 = overlay; `addTile` receives
shape+1 with 0/1 as flat cases).

**Research now:**
1. Emit the 13×4 cell oracle as a Rust test fixture (from the minimap
   selector); port the constructor; assert cell-equality for all 52 combos
   plus per-vertex midpoint averages. Executed oracle available:
   `reference-fixtures/deob_golden.txt` (`tri …` lines cover all 52 combos
   with positions, heights, colors, texture ids).
2. Acceptance: 52/52 oracle match; a rendered 3×3 shape sampler matches the
   reference screenshot cell-for-cell.

## H6. Underlay blur + slope shading parity

**Why hard:** separable 11×11 window with mixed weighting (hue by
hueMultiplier, sat/light by count), jitter, per-corner brightness, and a
separate `Tiles_underlays2` darkening term — five interacting pieces, each
visually subtle alone.

**What RuneLite gives us:** exact kernel (R11–R12), now including the
`underlays2` correction and the jitter random-walk spec (pin jitter to 0 for
determinism).

**Research now:**
1. Implement as pure functions on an extended grid; hand-verify on a 3×3
   high-contrast fixture (desert/grass border) computed independently.
2. Acceptance: fixture match + full-region golden test vs a live-client
   screenshot pair (Lumbridge grass, desert edge).

## H7. `computeFaceUvs` parity (tangent-space + camera projection)

**Why hard:** tangent/bitangent/normal frame + per-vertex camera-ray
intersection in float; degenerate (thin) triangles blow up the denominators;
the 0/1 fast path must trigger on exactly the same faces.

**What RuneLite gives us:** full function in `ModelUploader` (lines 669+),
both branches verified.

**Research now:**
1. Port with explicit degenerate guards; property-test UV continuity across
   shared edges on real model data.
2. Acceptance: textured faces match reference renders (water/lava direction +
   speed are the visible signal); fast-path trigger set identical on a corpus
   of untextured models.

## H8. `contourGround` + normals parity

**Why hard:** two warp modes (full vs height-ratio partial), `>>7` fixed-point
bilinear with Java truncation, bounds-cylinder gating, plus `calculateVertexNormals`
accumulation with its `>>1` rescale loop and 256-normalization.

**What RuneLite gives us:** 1:1 pseudocode (deob guide §3) + normals loop head
(verified per-model scope — no welding, R17).

**Research now:**
1. Transcribe both warp modes + the normals loop end (magnitude divide,
   zero-guard) with truncation-semantics tests on negative inputs. Oracle:
   `reference-fixtures/deob_golden.txt` (`contour flat/slope` lines;
   hand-verified slope value −28).
2. Acceptance: cliff/wall bases sit exactly on slopes in the sampler scene; no
   lighting creases *within* single models (creases *between* models are
   correct per R17).

## H9. Alpha two-pass + zone ranges + thread ordering

**Why hard:** three passes (`PASS_OPAQUE/PRE_PASS_ALPHA/PASS_ALPHA`), per-zone
`removeTemp`, `useStaticUnsorted` fast path, per-thread VAO lists, and
`renderAlpha(mu, …)` threading the uploader through — ordering bugs show as
translucency that depends on camera angle.

**What RuneLite gives us:** `drawPass` fully read (GpuPlugin 1166–1212),
`Zone.renderAlpha` signature, synchronized alpha cache.

**Research now:**
1. Draw the pass/state machine as a diagram first (which thread owns what,
   where temp models live/die, when `vaoA.unmap` runs), then implement.
2. Acceptance: scripted camera orbit over glass/water/ice content with zero
   popping frames vs a fixed-seed reference run.

## H10. egui/wgpu interop + reverse-Z plumbing

**Why hard:** shared device/queue with eframe, sRGB swapchain vs linear
pipeline, MSAA resolve inside the egui callback encoder, and reverse-Z
(`Greater`/clear-0/`Depth32Float`) inside someone else's render pass — one
wrong default (e.g. eframe's depth state) silently un-does R21.

**What RuneLite gives us:** the complete GL state inventory (blend equations,
cull enable window, MSAA renderbuffers, fog/brightness/tick producers) in
A5/A9 + port notes §3/§6.

**Research now:**
1. Spike first: minimal eframe app with one `CallbackTrait` triangle drawn
   with `Greater`/clear-0/depth32float + MSAA resolve, proving the state
   survives egui compositing.
2. Acceptance: the spike renders the reverse-Z + bias micro-offset correctly
   (two coplanar quads with different bias bytes resolve deterministically)
   before any scene code lands.

---

## Work order (dependency-aware)

H10 spike → H4 rounding policy → H1 lighting → H5 terrain → H6 blur/shade →
H8 contour/normals → H7 UVs → H2 sort → H3 camera lock (needs H5 scene to
judge) → H9 passes. H3's landmark test needs a renderable scene, so camera
math is researched now (formulas above) but *locked* after H5.
