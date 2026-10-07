# RuneLite Deob Reading Guide — Is It Readable? Does It Need Cleanup?

Question answered here: for each Group E pin in `RUNELITE_RENDER_SOURCES.md`, is
the melxin `runescape-client` source readable as-is, or does it need cleanup /
recovery work? All verdicts below come from **first-hand reads** of the files
(line numbers cited). Short version: **no re-deobfuscation needed, no cleanup
pass needed — except one item** (the underlay blur kernel, §8).

Conventions: deob code names locals `varN`, methods `methodN`, fields `fieldN`,
but `@Export`/`@ObfuscatedName` annotations give stable anchors. "Readable"
below means the algorithm is recoverable as written.

**Machine backing:** every pin below has been *executed*, not just read —
`tools/deob-harness/` compiles the deob tree headless and `Dumper` dumps
`reference-fixtures/deob_golden.txt` (tables, all 52 shape×rotation
triangulations, color-function sweeps, contour + lighting fixtures). The
pseudocode in §§2–5 was verified against actual outputs (e.g. the contour
slope case yields exactly the hand-computed bilinear value).

---

## 1. Verdict table

| Pin | File | Verdict |
|---|---|---|
| `tileShape2D` / `tileRotation2D` | `Scene.java:29-33, 285-286` | **Readable — copy verbatim.** Static int matrices, zero logic. No cleanup. |
| `triangleTextureIndices` / `faceIndices` | `SceneTileModel.java:10-13, ~79` | **Readable — copy verbatim.** Same as above. |
| `SceneTileModel` constructor | `SceneTileModel.java:89-318` | **Readable with the variable map in §2.** Arithmetic is plain; only names are obfuscated. No tooling needed. |
| `addTile` branches | `Scene.java:480-518` | **Readable.** Three branches + sentinel logic are obvious. Record arg order, move on. |
| `newBoundaryObject` / `newFloorDecoration` / game-object add | `Scene.java:583+` | **Readable.** Slot assignment + offset formulas visible at the draw sites (lines ~1918–2301 use the same `x*4096+xOffset` form the GPU path reads). |
| `Model.contourGround` | `Model.java:473-545` | **Fully readable math.** Cleaned pseudocode in §3 is 1:1. |
| `ObjectComposition.getModel*` + light constants | `ObjectComposition.java:662-768` | **Fully readable.** Cache keys, `ambient+64`/`contrast+768`, sun vector, branch order all explicit. |
| `FloorUnderlayDefinition.setHsl` | `FloorUnderlayDefinition.java:111-181` | **Fully readable.** Textbook RGB→HSL. Pseudocode in §4. |
| Shape-table *use* (which sub-tile is overlay?) | `Scene.drawTileMinimap:1162-1202` | **Readable reference implementation** (`tileShape2D[shape][rotationTable[i]] != 0 → overlay`). Reuse for tests. |
| **Underlay spatial blur kernel** | `class470.java:880-1049` (terrain build) | **Solved — see §8.** Separable 11×11 blur, hue weighted by hueMultiplier, plus slope brightness. No recovery work remains. |

**Bottom line:** do not run a cleanup/deob pass. The tables copy out, the
algorithms below port from the pseudocode, and the blur kernel — the one item
flagged as open — has since been recovered from `class470` (see §8 and
`RUNELITE_RUNTIME_RULES.md` R11–R12). Nothing is outstanding.

---

## 2. `SceneTileModel` constructor — variable map + scheme

Signature (19 params):
`ctor(shape, rotation, texId, tileX, tileY, hSW,hSE,hNE,hNW, uSW,uSE,uNE,uNW, oSW,oSE,oNE,oNW, underlayRgb, overlayRgb)` —
pins resolved against the `addTile` call sites: value set 1 (`u*`) is the
**underlay** corner colors, set 2 (`o*`) the **overlay** corner colors
(`Scene.addTile` receives already-split underlay/overlay color quads plus
`shape+1`, where 0 and 1 are the two flat-paint cases and 2–12 index the shape
tables — see `class470.java:1075`).

- Grid unit is 128 (`var20`); halves `64`, quarters `32`, three-quarters `96`.
  Base origin `(tileX*128, tileY*128)`.
- **Vertex index scheme** (index → position / height source / value sources):
  `1`=SW corner, `2`=S edge midpoint (averaged heights/colors),
  `3`=SE corner, `4`=E midpoint, `5`=NE corner, `6`=N midpoint, `7`=NW corner,
  `8`=W midpoint, `9–12`=edge quarter-points, `13–16`=inner quarter-points.
  Midpoints average their two neighbors (`(a+b)>>1`) in height *and* both value
  sets — that averaging is what keeps lighting continuous across cuts.
- **Rotation remap** (applied to the per-shape vertex list before placement):
  indices 1–8 rotate as `(v − 2·rot − 1 & 7) + 1`; 9–12 as `(v − 9 − rot & 3) + 9`;
  13–16 as `(v − 13 − rot & 3) + 13`. Face-vertex refs `< 4` rotate as
  `(v − rot & 3)`.
- **Face emission:** `faceIndices[shape]` is walked 4 ints at a time
  `(flag, a, b, c)`. `flag == 0` → underlay colors, `triangleTextureId = -1`;
  else overlay colors with `triangleTextureId = texId` (the overlay texture, or
  -1 for flat HSL overlays).
- `isFlat` = all four corner heights equal (checked up front, line 91).
- Trailing min/max height block (lines 290–317, `/14`) computes bounds then
  discards them in this snapshot — dead code after deobfuscation; ignore it
  (or keep as a culling hint if your editor wants one).

---

## 3. `contourGround` — cleaned pseudocode (1:1 with `Model.java:473-545`)

```
contourGround(tileHeights[][], x, baseH, z, copy, clip /* = clipType*65536 */):
  computeBounds() → xzRadius
  tileRange = [x−r, x+r] × [z−r, z+r]  (in world units)
  if range outside heightmap: return this
  snap range to tile indices (>>7 floors, +127>>7 ceils)
  if all four range corners == baseH: return this        # flat fast path
  out = copy ? deepCopy(yOnly) : this
  if clip == 0:                                          # full warp
    for each vertex v:
      wx = x + vx[v];  wz = z + vz[v]
      subX = wx & 127;  subZ = wz & 127
      tx = wx >> 7;     tz = wz >> 7
      row0 = h[tx][tz]*(128−subX)   + h[tx+1][tz]*subX   >> 7
      row1 = h[tx][tz+1]*(128−subX) + h[tx+1][tz+1]*subX >> 7
      ground = row0*(128−subZ) + row1*subZ >> 7
      vy[v] = ground + vy[v] − baseH
  else:                                                 # partial warp (tall objects)
    for each vertex v:
      depthRatio = (−vy[v] << 16) / modelHeight
      if depthRatio < clip:                             # only low verts warp
        ground = bilinear(tileHeights, x+vx[v], z+vz[v])  # as above
        vy[v] = (clip − depthRatio)*(ground − baseH)/clip + vy[v]
  resetBounds(); return out
```

Notes: heights are ints in world units as stored in `tileHeights` — the GPU
upload path applies no scale or negation (verified; see R22), so use them raw. `clipType` arrives as `clipType*65536`, so
`clip == 0` means "object opcode 21" and negative means "no contour" (guard is
`clipType*65536 >= 0` at all three call sites in `ObjectComposition`).

---

## 4. `setHsl` — cleaned pseudocode (1:1 with `FloorUnderlayDefinition`)

```
setHsl(rgb24):
  r,g,b = bytes(rgb)/256.0
  lo = min(r,g,b);  hi = max(r,g,b)
  lum = (lo+hi)/2;  sat = 0;  hue = 0
  if lo != hi:
    sat = (hi−lo) / (lum<0.5 ? (hi+lo) : (2−hi−lo))
    hue = (hi==r) ? (g−b)/(hi−lo)
        : (hi==g) ? (b−r)/(hi−lo) + 2
        :           (r−g)/(hi−lo) + 4
    hue /= 6
  saturation = clamp(256·sat);  lightness = clamp(256·lum)
  hueMultiplier = max(1, (lum>0.5 ? (1−lum)·sat : lum·sat)·512)
  hue = hueMultiplier · hueAngle
```

`hueMultiplier` doubles as the **blend weight** for the spatial kernel (§8):
saturated tiles influence neighbors more. Any reimplementation must reuse these
exact fields, not plain RGB averages.

---

## 5. `ObjectComposition` model pipeline (what the calls mean)

- Cache keys: entity path `id`-based (`var2 + (id<<10)`, plus `var1<<3` when
  multi-model); model path same scheme. Two separate caches
  (`…_cachedEntities`, `…_cachedModels`) — an editor-side model cache should keep
  the same split (lit vs unlit).
- Light call: `toModel(ambient+64, contrast+768, sun=(-50,-10,-50))`. The `+64 /
  +768` offsets are engine constants — bake them in, don't "fix" them. Decode
  scales to match: ambient is raw (`readByte`, opcode 29) but contrast arrives
  multiplied — `contrast = readByte()*25` (opcode 39). FileStore keeps the raw
  byte, so apply ×25 at light-computation time.
- Branch order per request: fetch-or-build ModelData → if `!nonFlatShading`
  light immediately, else set `ambient/contrast` fields + `calculateVertexNormals`
  (smooth path) → `copyModelData` when smooth (callers may mutate) → contour if
  `clipType*65536 >= 0`.
- `getModelDynamic(..., SequenceDefinition anim, frame)`: `anim == null &&
  clipType == -1` → cached model as-is; else `transformObjectModel(model, frame)`
  (skeletal pose) or `toSharedSequenceModel(true)`, then contour with
  `copy=false` (pose buffers are transient — never contour the cached original).

---

## 6. Shape-table use pattern (from `drawTileMinimap`, reusable as a test)

```
sub = tileShape2D[shape]          # 16 entries, 0 = underlay cell
order = tileRotation2D[rotation]  # 16-entry permutation
for each of 16 sub-cells i:
  cell = sub[order[i]]
  color = (cell == 0) ? underlayRgb : overlayRgb
```

Any tessellator port must reproduce this selector exactly; drive it with the
same two tables and compare cell-by-cell on shapes 0–12 × rotations 0–3.

---

## 7. What "cleanup" would even mean — and why you skip it

A cleanup pass would rename `varN`/`methodN`/`fieldN` across ~2,800 deob files.
Cost: days, plus re-pinning risk every rev. Benefit for this project: ~zero —
the render-relevant surface is 7 pins, all readable above, and the live
rendering code (Groups A–D) is clean RuneLite API/plugin source, not deob.
The correct investment is the pseudocode in §§2–6 (done) plus §8.

---

## 8. The blur kernel: SOLVED (was "needs recovery" — recovered 2026-10-07)

The kernel was found in `class470.java:880-1049` (terrain build). Exact rules:

- **Slope brightness first** (893–904): per-tile light from the height gradient
  with sun `(−50,−50,−10)`, minus an overlay-shape darkening kernel
  (`>>3,>>2,>>2,>>3,>>1` neighbor weights). Stored per corner.
- **11×11 separable box blur**: column sums over `x±5`, then row sums over
  `z±5`, of `(hue, saturation, lightness, hueMultiplier, count)` — non-underlay
  tiles excluded via the count channel.
- **Reduction**: `hue = Σhue·256/ΣhueMultiplier` (weighted),
  `sat = Σsat/count`, `light = Σlight/count`; then `hue+rndHue` /
  `light+rndLightness` jitter (clamped) and per-corner palette lookup with the
  slope brightness. Full port-ready spec: `RUNELITE_RUNTIME_RULES.md` R11–R12.

No `MappingDumper` work needed. This closes the last open recovery item.
