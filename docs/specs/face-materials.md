# Face Metadata and Texture Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

Independent October RuneLite reference pins:

- `ModelUploader.java` blob `35347963838d1be74deddd59027a73623d7872f3`
- `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`
- imported `Texture.java` blob `80a4d1a45a51b28c3d5da9f0ad47a03ccf0a482f`
- staged live `vert.glsl` blob `d899cf3180295bd18d30adf901fd7a460e560318`

## SPEC: FACE-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required semantic face metadata

The canonical model representation must preserve, when present:

- triangle vertex indices;
- per-face color/light result inputs or baked reference colors as owned by the semantic stage;
- face render type;
- per-face priority or model-level default priority;
- per-face alpha/transparency;
- face texture id;
- texture-coordinate/texture-face selector;
- texture-triangle indices/data needed to reproduce mapped texture coordinates;
- authored face-bias metadata when supplied by the target model data path.

Absence of an optional array is not equivalent to inventing arbitrary zero-filled semantics unless the reference default is explicitly defined.

### Invariants

Renderer packing may change, but semantic fields cannot be discarded merely because one initial wgpu path does not use them yet.

### Failure signature

Models render correctly only for simple opaque/untextured assets; later transparent, prioritized, textured, or bias-authored models cannot be reproduced without re-decoding/rearchitecting.

### Required tests

Decode/model-construction fixture retaining every optional face metadata class through semantic-to-render handoff.

### Related specs

`FACE-002`, `FACE-003`, `TEXTURE-001`, `LIGHTING-001`.

---

## SPEC: FACE-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for reference ordering semantics; exact Rust execution strategy is deferred to `RENDERER_POLICY`

### Required priority meaning

Face priority values `0..11` are not a generic ascending/descending sort key.

The audited software/reference algorithm:

1. obtains visible faces in depth buckets;
2. maintains distinct queues for priorities `0..11`;
3. computes average depth thresholds for groups `(1,2)`, `(3,4)`, and `(6,8)`;
4. interleaves priority `10/11` faces against ordinary priority processing at priority boundaries `0`, `3`, and `5` according to those thresholds;
5. emits priorities `0..9` in their reference pass ordering, preserving the depth ordering established within each queue.

The imported October RuneLite `ModelUploader` independently reproduces this structure.

### Semantic requirement

The semantic model must preserve priority exactly. Any renderer claiming reference parity must prove its chosen algorithm matches crafted ordering fixtures. A simple tuple sort such as `(priority, depth)` is not accepted as equivalent without proof.

### Failure signature

Cape/foliage/window/intersecting-surface faces pop through one another at specific camera angles despite correct geometry and depth buffer configuration.

### Required tests

Crafted face set containing all priorities `0..11`, depths straddling `avg12`, `avg34`, `avg68`, and both priority-10/11 queues, with exact reference emission order.

### Related specs

`FACE-003`, future renderer priority ADR.

---

## SPEC: FACE-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for alpha metadata and reference interpretation boundaries

### Required behavior

Per-face alpha/transparency is semantic metadata and must survive decode/model construction.

In the audited software path:

- absent face-alpha data behaves as the reference default;
- ordinary stored alpha bytes are interpreted using reference unsigned/sentinel rules at draw time;
- signed byte `-1` receives special treatment in the normal software face path rather than being naively interpreted as a generic alpha value.

The imported RuneLite renderer also distinguishes alpha faces in upload/sort paths and may combine model-level transparency with face-level transparency for dynamic/sorted rendering.

### Scope boundary

Exact blend-state choice, pass count, order-independent techniques, and GPU buffer partitioning belong to Checkpoint 5. They may differ only if reference appearance and ordering semantics are proven equivalent for the target profile.

### Failure signature

Transparent faces become opaque, fully hidden, or blend in the wrong order; models with model-level transparency differ between static/dynamic paths.

### Required tests

Face-alpha values `0`, ordinary nonzero values, the reference `-1/255` sentinel case, plus model-level transparency interaction in the renderer reference suite.

### Related specs

`FACE-002`, future renderer transparency ADR.

---

## SPEC: FACE-004

**Domain:** `OSRS_SEMANTIC` for authored metadata; exact depth application is `RENDERER_POLICY`  
**Status:** `VERIFIED` separation

### Required behavior

Authored face-bias data, when present in the model representation, must be preserved through the semantic-to-render boundary.

The imported RuneLite renderer packs this bias per face and its live vertex shader applies it as a clip-space depth offset. That exact formula is not promoted here as an OSRS semantic requirement.

### Invariants

A renderer may choose another mathematically appropriate implementation, but it may not discard bias metadata and rely only on polygon depth coincidence.

### Failure signature

Coplanar or intentionally layered faces Z-fight, decals flicker, or surface ordering differs on models that authored explicit bias.

### Required tests

At least two coplanar faces with distinct authored bias proving stable reference ordering under the selected renderer policy.

### Related specs

Future renderer depth/bias ADR.

---

## SPEC: TEXTURE-001

**Domain:** `OSRS_SEMANTIC` for texture/material inputs; UV generation/upload/animation application may include `RENDERER_POLICY`  
**Status:** `VERIFIED` boundary and reference oracle

### Required semantic inputs

The reusable semantic model must retain enough information to reproduce reference model texturing:

- face texture id;
- optional texture-face selector;
- texture triangle indices/vertices;
- decoded texture animation direction and speed;
- any target-revision texture/material definition fields required by the selected profile.

Do not encode RuneLite's fixed texture-array capacity as a cache semantic invariant.

### Reference UV oracle

The imported `ModelUploader.computeFaceUvs` provides a differential reference:

- with an explicit texture face, reconstruct coordinates from the texture triangle tangent/bitangent basis;
- in the dynamic/sorted projected case, reference behavior may project face vertices from the camera ray onto the texture plane before basis evaluation;
- without an explicit texture face, the reduced mapping is:

```text
A = (0,0)
B = (1,0)
C = (0,1)
```

Terrain UVs are handled by terrain/render contracts and must not be conflated with model texture-triangle mapping.

### Texture animation boundary

Animation direction/speed are semantic decoded inputs. Converting them to a 2D vector and applying `tick`-scaled UV motion is renderer/material preparation. The staged RuneLite shader's `1/128` animation unit is reference evidence for Checkpoint 5, not a universal cache limit.

### HSL override reference

The imported RuneLite model uploader does not apply its model HSL override to textured faces. A RuneLite-parity rendering profile must test that behavior, but the override mechanism remains a renderer/reference concern unless separately proven as client semantic state.

### Failure signature

Textures shear, rotate, or swim on models; animated water/lava moves on the wrong axis/speed; texture ids truncate due fixed-capacity assumptions.

### Required tests

Explicit texture-face UV fixture, no-texture-face canonical mapping, projected/dynamic reference case, and animation direction/speed material handoff.

### Related specs

`FACE-001`, `TERRAIN-001`, future renderer material ADR.