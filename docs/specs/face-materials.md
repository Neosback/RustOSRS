# Face Metadata and Texture Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

Independent imported RuneLite renderer pins:

- `ModelUploader.java` blob `35347963838d1be74deddd59027a73623d7872f3`
- `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`
- `TextureManager.java` blob `e830a518dc13c173f22f006fe52cb25e3c0fc8b3`
- imported `Texture.java` blob `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f`
- imported `frag.glsl` blob `0ca7180d50ef90e5083c85f7182e60521ef76baa`
- staged/imported vertex shader reference for texture animation and face bias

Correction provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`.

## SPEC: FACE-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

The canonical model/render handoff must preserve, when present:

- triangle vertex indices;
- baked/reference face colors or their owning semantic inputs;
- face render type;
- per-face priority or model default priority;
- raw per-face alpha;
- face texture id;
- texture-face selector;
- texture-triangle data required for mapped UVs;
- authored face bias;
- absence/presence of optional parallel arrays.

Renderer packing may change but cannot erase metadata needed by another reference path.

---

## SPEC: FACE-002

**Domain:** `OSRS_SEMANTIC` for authored priority and reference software ordering; dispatch is `RENDERER_POLICY`  
**Status:** `VERIFIED` oracle

### Software-client priority oracle

Face priorities `0..11` are not a generic sort key. The pinned software/reference algorithm:

1. admits visible faces into depth buckets;
2. maintains queues for priorities `0..11`;
3. computes average-depth thresholds for `(1,2)`, `(3,4)`, and `(6,8)`;
4. interleaves the priority `10/11` queues at ordinary-priority boundaries `0`, `3`, and `5`;
5. preserves the required depth order inside those queues.

A tuple sort such as `(priority, depth)` is not equivalent.

### RuneLite GPU qualification

The imported RuneLite GPU code contains the same priority sorting machinery, but does **not** dispatch every renderable through it.

In the captured `GpuPlugin`, `prioritySort` is enabled specifically for `RENDERMODE_SORTED_NO_DEPTH`. Static zone upload and ordinary depth-tested dynamic rendering use different operational paths.

Therefore these are distinct parity targets:

- software-client reference ordering semantics;
- captured RuneLite GPU render-mode dispatch.

RustOSRS Reference rendering chooses the software-client algorithm for renderables classified as requiring reference ordered behavior. This is a deliberate renderer policy and must not be described as an exact clone of RuneLite's dispatch rules.

### Required tests

Crafted faces spanning all priorities, threshold crossings, both 10/11 queues, and exact expected emission order. Renderer tests should additionally cover render-mode/classification decisions separately from the priority oracle itself.

---

## SPEC: FACE-003

**Domain:** `OSRS_SEMANTIC` for raw metadata and pre-lighting sentinel interpretation; blend/pass strategy is `RENDERER_POLICY`  
**Status:** `VERIFIED`

### Raw alpha metadata

Raw signed per-face alpha must survive decode/model construction until the operation that owns its interpretation.

### ModelData pre-lighting sentinels

Before final model lighting, the pinned `ModelData` path applies special alpha-driven render-type behavior:

- raw alpha `-2` selects render type `3`; for ordinary untextured final color this produces gray/lightness `128` behavior;
- raw alpha `-1` selects render type `2`; this is a hidden/suppressed face form in the final untextured model path.

This preprocessing is distinct from later draw-time alpha interpretation.

### Software draw-time interpretation

Where a raw face alpha reaches the normal software `Model.drawFace` path, signed byte `-1` is interpreted as unsigned `253`, not generic `255` opacity/transparency semantics.

Absent alpha follows the reference default and ordinary stored bytes use the reference unsigned conversion.

### RuneLite renderer boundary

RuneLite uses path-specific alpha handling and may combine model-level and per-face transparency for dynamic/sorted models. Static zone alpha splitting and no-depth render modes are renderer behavior, not replacements for the semantic alpha/sentinel rules above.

### Required tests

Cover:

- absent alpha;
- ordinary signed/unsigned values;
- raw `-1` draw-time `253` case;
- ModelData `-1` -> render type `2` preprocessing;
- ModelData `-2` -> render type `3`/gray `128` case;
- model-level plus face-level transparency in renderer-reference tests.

---

## SPEC: FACE-004

**Domain:** `OSRS_SEMANTIC` for authored metadata; application is `RENDERER_POLICY`  
**Status:** `VERIFIED` separation

Authored face-bias metadata must survive extraction. The imported RuneLite reference applies that bias as a clip-depth adjustment, but the semantic contract is the authored value itself.

The Reference renderer requires an explicit equal-depth fixture and a distance-sensitive projection fixture before claiming parity. A generic polygon offset is not an assumed substitute.

---

## SPEC: TEXTURE-001

**Domain:** `OSRS_SEMANTIC` for texture/material inputs; upload, sampling, UV evaluation, and animation application are `RENDERER_POLICY`  
**Status:** `VERIFIED` boundary and reference oracle

### Required semantic inputs

Preserve:

- full-width face texture id;
- optional texture-face selector;
- texture-triangle indices/data;
- decoded texture animation direction and speed;
- target-revision texture/material definition fields needed by the selected profile.

Do not encode RuneLite's historical `TEXTURE_COUNT = 256` as semantic truth.

### Model UV oracle

Imported `ModelUploader.computeFaceUvs` provides the renderer differential oracle.

With an explicit texture face, coordinates are reconstructed from the texture triangle basis. The dynamic/sorted projected path may first project face vertices from the camera ray onto that plane.

Without an explicit texture face:

```text
A = (0,0)
B = (1,0)
C = (0,1)
```

Terrain UVs remain a separate terrain/render contract.

### Texture animation

Animation direction/speed are semantic decoded inputs. Renderer preparation converts them to a UV vector, and the captured RuneLite shader uses a `1/128` tick unit. Deterministic render tick is required for reproducible output.

### Captured RuneLite texture upload/sampling reference

For the imported renderer snapshot:

- texture-array layers are `128 x 128` and stored as `GL_RGBA8`;
- source pixel value `0` produces zero RGBA, while nonzero RGB receives alpha `255`;
- the fragment shader samples level 0 separately and discards when level-0 alpha is below `1.0`;
- upload temporarily sets texture-provider brightness to `1.0`, while final brightness is applied in the shader with `pow`;
- textured-face light uses `fHsl / 127` unless texture-light mode selects RGB tinting;
- magnification uses nearest filtering;
- minification starts as nearest but can become `GL_NEAREST_MIPMAP_LINEAR` when mipmaps/anisotropy are enabled;
- S wrap is explicitly clamp-to-edge;
- T wrap is not overridden in the captured setup and therefore follows the GL default repeat behavior.

These are RuneLite Reference-profile facts, not cache invariants. Enhanced rendering may deliberately choose different filtering, but must not call that choice RuneLite-reference-equivalent.

### HSL override reference

The captured model uploader does not apply its model HSL override to textured faces. Preserve this as a RuneLite-profile renderer fixture, not as generic cache semantics.

### Required tests

- explicit texture-triangle mapping;
- canonical no-selector mapping;
- projected dynamic mapping;
- full-width texture id mapping;
- animation direction/speed/tick;
- transparent-zero texture pixel discard;
- reference sampler/wrap state;
- shader brightness/lightness behavior.

### Related specs

`FACE-001`, `FACE-003`, `TERRAIN-001`.
