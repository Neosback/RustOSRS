# Face Metadata and Texture Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

Independent imported RuneLite renderer pins:

- `ModelUploader.java` blob `35347963838d1be74deddd59027a73623d7872f3`
- `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`
- `GpuPlugin.java` blob `8c233cb381e288146af47ac375fa3dfc41a53d76`
- `Zone.java` blob `80594f4ca2759a96c9a3a9800008790b91c5d692`
- `TextureManager.java` blob `e830a518dc13c173f22f006fe52cb25e3c0fc8b3`
- `Renderable.java` blob `bce439b4dd2000a98d83191f3d0348b64ff83b3c`
- imported `Texture.java` blob `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f`
- staged `vert.glsl` blob `d899cf3180295bd18d30adf901fd7a460e560318`
- staged `frag.glsl` blob `0ca7180d50ef90e5083c85f7182e60521ef76baa`

## SPEC: FACE-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

The canonical model/render handoff preserves, when present:

- triangle indices;
- baked/reference face colors;
- original face render type;
- per-face priority or model default priority;
- raw signed face alpha;
- face texture ID;
- texture-face selector and texture triangles;
- authored face bias;
- semantic model identity/provenance.

Optional arrays remain optional. Absence is not replaced by invented authored zeros unless a later policy explicitly applies the reference default.

### Suppressed faces

`ModelData -> Model` conversion may retain topology while marking a face suppressed with baked color slot `c == -2`. Renderer extraction must preserve that state, and a Reference draw path must not re-admit that face merely because its triangle indices are still present.

---

## SPEC: FACE-002

**Domain:** `OSRS_SEMANTIC` for priority metadata and software-reference ordering; realization is `RENDERER_POLICY`  
**Status:** `VERIFIED` for the pinned software/client algorithm; M10 CPU implementation exists

### Software/client reference ordering

Face priority `0..11` is not a generic sort key.

Pinned `Model.method5946` behavior:

1. establish depth-bucket ordering;
2. distribute into priority queues `0..11`;
3. compute average thresholds for `(1,2)`, `(3,4)`, `(6,8)`;
4. process the priority-10 queue followed by priority-11 as the special stream;
5. interleave special faces before ordinary priority boundaries `0`, `3`, and `5` according to those thresholds;
6. retain queue/depth ordering required by the reference routine.

The M10 CPU preparation path intentionally targets this software/client oracle.

### Important RuneLite GPU distinction

The imported October RuneLite GPU renderer does **not** apply that priority algorithm to every renderable.

Its `Renderable` API distinguishes:

```text
RENDERMODE_DEFAULT           = 0
RENDERMODE_SORTED            = 1
RENDERMODE_SORTED_NO_DEPTH   = 2
RENDERMODE_UNSORTED          = 3
RENDERMODE_UNSORTED_NO_DEPTH = 4
```

In the imported GPU path, `ModelUploader.uploadSortedModel(..., prioritySort)` runs the priority queues only when `prioritySort` is true. `GpuPlugin` enables that for `RENDERMODE_SORTED_NO_DEPTH`; ordinary dynamic upload passes `false`, and static opaque upload does not globally reproduce the software priority queue.

Imported RuneLite alpha/static ordering also uses model/face distance/depth ordering in `Zone`, not universal software-priority ordering.

Therefore two reference targets must never be conflated:

- **OSRS software/client parity:** exact `Model.method5946` priority behavior where the software path owns ordering;
- **imported RuneLite GPU parity:** render-mode/path-specific sorting and depth behavior.

RustOSRS Reference mode currently chooses the software/client priority algorithm as the stronger ordering contract for priority-sensitive renderables. Documentation must describe that as a project/reference decision, not as "what RuneLite GPU always does."

---

## SPEC: FACE-003

**Domain:** `OSRS_SEMANTIC` for alpha metadata and pre-lighting sentinel meaning; draw/blend realization is `RENDERER_POLICY`  
**Status:** `VERIFIED`

### Stage 1: ModelData lighting sentinels

Before final lighting:

```text
raw alpha -2 -> effective render type 3
raw alpha -1 -> effective render type 2
```

For untextured type `3`, baked output is gray `128` with flat marker `c == -1`.

Type `2` becomes suppressed baked output `c == -2`. Textured unsupported types are likewise suppressed.

### Stage 2: software draw alpha

For a face that reaches the normal software draw path:

- no face-alpha array -> rasterizer alpha `0`;
- ordinary signed stored byte -> unsigned-byte interpretation;
- raw `-1` receives the pinned special normal-path value `253` rather than generic `255` handling.

These two stages must not be collapsed. The semantic lighting sentinel can suppress a face before later raster alpha would matter.

### RuneLite GPU distinction

The imported GPU renderer has its own static/dynamic alpha sorting and model-level transparency composition paths. Those are corroborating renderer evidence, not proof that the software/client draw path and RuneLite GPU are identical.

---

## SPEC: FACE-004

**Domain:** authored metadata is `OSRS_SEMANTIC`; exact depth application is `RENDERER_POLICY`  
**Status:** `VERIFIED` boundary

Authored face bias is preserved through semantic and extraction layers.

The imported RuneLite vertex shader applies:

```text
clip_or_view_depth_term += faceBias / 128.0
```

before the final perspective divide/projection result is consumed. Under the imported no-far reverse-Z projection, the resulting normalized depth separation decreases with distance. RustOSRS requires a near/far bias fixture rather than assuming the offset is screen-depth constant.

---

## SPEC: TEXTURE-001

**Domain:** texture/material inputs are `OSRS_SEMANTIC`; image construction, UV realization, sampling, paging, and animation are `RENDERER_POLICY`  
**Status:** `VERIFIED` boundary; M10 CPU material/UV handoff exists; GPU realization remains later

### Semantic inputs

Preserve:

- full-width semantic texture ID;
- optional texture-face selector;
- texture triangle indices;
- decoded texture source/composition inputs;
- decoded animation direction and speed;
- target-revision material fields required by the selected profile.

RuneLite's imported fixed `TEXTURE_COUNT = 256` is an implementation capacity for that snapshot, **not semantic truth**.

### Model UV oracle

Imported `ModelUploader.computeFaceUvs` is the differential renderer oracle:

- no explicit texture face -> canonical A `(0,0)`, B `(1,0)`, C `(0,1)`;
- explicit texture face -> reconstruct from the texture-triangle tangent/bitangent basis;
- projected/dynamic path may first project model vertices onto the texture plane before basis evaluation.

Terrain UVs have a separate terrain contract.

### Imported RuneLite texture-image construction

The audited `TextureManager` path uses 128x128 RGBA source images for this renderer snapshot. Pixel conversion treats source RGB value `0` as transparent and nonzero RGB as alpha `255`.

The staged fragment shader samples level 0 for alpha-cutout testing and discards when the level-0 alpha is below `1`. Brightness adjustment is applied in shader-side color processing; textured model lighting uses the face lightness payload (`fHsl / 127` in the staged shader path) rather than re-running semantic normal lighting.

These are renderer-profile requirements, not cache-definition invariants.

### Imported RuneLite sampler behavior

Do not summarize the imported path as simply "nearest filtering."

Verified state:

- magnification filter: `NEAREST`;
- minification with anisotropic/filter level `0`: `NEAREST`;
- minification with level `>= 1`: `NEAREST_MIPMAP_LINEAR`;
- imported config default filtering level is `1`;
- S wrap is explicitly `CLAMP_TO_EDGE`;
- T wrap is not overridden in the audited texture setup and therefore follows the OpenGL default repeat behavior unless another owning setup proves otherwise.

RustOSRS Reference sampling must encode the exact selected reference profile explicitly. Enhanced anisotropic/filtering choices remain separate.

### Texture animation

Animation direction/speed remain decoded semantic inputs. Renderer material preparation converts them to the pinned UV velocity convention and deterministic render tick. The staged shader's `1/128` animation unit is renderer reference evidence, not a semantic texture-size assumption.

### Required tests before GPU parity claim

- explicit and canonical model UV cases;
- projected texture-plane case;
- pixel-zero transparency conversion;
- alpha-cutout threshold;
- textured lightness behavior;
- brightness path;
- S clamp / T repeat fixture;
- min/mag filter profile fixture;
- deterministic animation direction/speed/tick;
- full-width texture ID mapping independent of fixed array capacity.

### Related specs

`FACE-001`, `FACE-003`, `TERRAIN-001`, renderer depth/material ADRs.
