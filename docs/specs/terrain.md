# Terrain Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `SceneTileModel.java` blob `ce6a179cfa93e02271af87164e102ee538223718`
- `FloorUnderlayDefinition.java` blob `f06136263590143afeedf5a8e448cd901f960614`
- `FloorOverlayDefinition.java` blob `f3a15cc07c53ccae74b2219db88d456b77d88d68`
- `class470.java` blob `1cd9cad5cb4be865dcae94dc633bba821644dc84`, especially `method9712(WorldView)`
- checked-in `reference-fixtures/deob_golden.txt` for executed shape/rotation outputs

Independent renderer-reference source:

- imported RuneLite `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`

Correction provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`.

## SPEC: TERRAIN-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Executable evidence:** existing shape/rotation golden fixture

### Required behavior

The semantic terrain model supports the audited shape templates `0..12`. Shape plus rotation determines exact generated vertices, interpolated heights/colors, face indices, underlay/overlay ownership, optional texture assignment, and flatness.

One tile is `128` local units. Midpoint and quarter positions and all interpolation use the reference integer arithmetic/order. GPU tessellation may repack the output but must not substitute a topologically different cut.

### Required tests

Retain exact golden output for every `13 x 4` shape/rotation combination, including vertices, faces, color-source selection, texture ids, and flatness.

---

## SPEC: TERRAIN-002

**Domain:** `OSRS_SEMANTIC` for terrain surface meaning; RuneLite upload is renderer corroboration  
**Status:** `VERIFIED`

A semantic terrain surface is either flat paint or shaped model state.

For flat paint, preserve four corner heights/colors, optional texture id, and the reference two-triangle split:

```text
NE, NW, SE
SW, SE, NW
```

`12345678` is the audited skipped/unrendered terrain-color sentinel.

For shaped terrain, consume the exact `TERRAIN-001` topology rather than reconstructing a generic quad in the renderer.

---

## SPEC: TERRAIN-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Underlay definition

For the pinned target:

- opcode `1` supplies RGB;
- post-decode converts RGB to HSL-derived fields;
- saturation/lightness clamp to `0..255`;
- `hueMultiplier` is derived from saturation/lightness and clamps to at least `1`;
- stored hue is weighted/scaled by that multiplier.

The builder consumes hue and `hueMultiplier` independently, so neither may be collapsed into a generic HSL value.

### Overlay definition

Defaults:

```text
primaryRgb   = 0
texture      = -1
hideUnderlay = true
secondaryRgb = -1
```

Relevant opcodes are `1` primary RGB, `2` texture, `5` disable `hideUnderlay`, and `7` secondary RGB. Post-decode computes secondary HSL when present and then primary HSL.

The complete builder is now source-pinned by `TERRAIN-004`; production implementation remains pending.

---

## SPEC: TERRAIN-004

**Domain:** `OSRS_SEMANTIC`  
**Source status:** `VERIFIED`  
**Production status:** `REQUIRED`

### Primary oracle

The exact public-pinned terrain builder is:

`class470.method9712(WorldView)` in blob `1cd9cad5cb4be865dcae94dc633bba821644dc84` at `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.

Earlier documentation incorrectly concluded that pinned `class470` was unrelated. Most of that file is unrelated text/layout code, but its final large static method is the complete terrain builder.

### Required builder behavior

The Rust port must preserve the reference operation order and integer behavior for at least:

1. terrain hue/lightness random-walk state;
2. per-corner slope lighting from neighboring height gradients;
3. subtraction of the weighted `Tiles_underlays2` shadow/clipping contribution;
4. separable radius-5, `11 x 11` underlay accumulation;
5. weighted hue reduction using hue multiplier, while saturation/lightness divide by participating-underlay count;
6. underlay packed HSL construction;
7. overlay shape/rotation, hide-underlay, magenta/sentinel, texture average-color, and secondary-color behavior;
8. exact corner-light application and `Scene.addTile(...)` arguments;
9. tile minimum-plane derivation described by `PLANES-004`;
10. scene finalization/normal-light call and bridge `setLinkBelow` phase.

### Shadow/clipping grid

`Tiles_underlays2` is not an "adjacent contrasting ground" color grid. The pinned object-placement path writes it from loc clipping/shadow behavior. For example, clipped boundary objects write value `50` at orientation-dependent cells, and clipped larger objects can write a height-derived shadow value over their footprint.

Terrain construction therefore depends on this object-derived grid being available before its lighting contribution is evaluated. The Rust implementation must model that dependency explicitly.

### Jitter boundary

The builder produces both unjittered and jittered underlay color forms.

- The four terrain corner color values passed through the audited `Scene.addTile` path use the unjittered packed HSL combined with corner lighting.
- Hue/lightness jitter is used for the additional derived RGB/minimap-style color field.
- Hue jitter wraps with `& 255`.
- Jittered lightness is clamped to `0..255`.

Therefore deterministic 3D terrain corner-color parity does not require randomizing the four corner HSL values. Jitter still exists as a real builder input/output concern and must not be silently deleted from the complete client-behavior model.

### Implementation gate

`TERRAIN-004` is no longer blocked by a missing source oracle. It remains unimplemented in production Rust and may not be marked `EXISTING` until a production builder and exact fixture pass.

The target profile may continue reporting the terrain builder as operationally blocked while that implementation is absent. That capability state means "implementation unavailable", not "source unknown".

### Required fixtures

Before production closure, add exact coverage for:

- slope-light grid;
- shadow-grid contribution from clipped locs;
- `11 x 11` weighted underlay blur, including borders and empty cells;
- hue-weight vs count distinction;
- overlay hidden/magenta/texture/secondary cases;
- unjittered four-corner output;
- deterministic fixed jitter and a nonzero jitter case for the derived RGB field;
- tile min-plane outputs;
- bridge/link-below sequencing.

### Related specs

`TERRAIN-003`, `PLANES-004`, `LOC-PLACEMENT-005`, `COORD-001`.
