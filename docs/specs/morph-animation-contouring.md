# Morph, Animation, and Contouring Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `DynamicObject.java` blob `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d`
- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

## SPEC: MORPH-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required transform selection

For an object definition with transforms:

1. if `transformVarbit != -1`, obtain the selector from that varbit;
2. otherwise if `transformVarp != -1`, obtain the selector from that varp;
3. when the selector addresses one of the non-fallback transform entries, select that id;
4. otherwise select the final transform entry as fallback;
5. selected id `-1` means there is no active transformed definition/model.

The decode contract must preserve opcode 77/92 semantics including the opcode-92 fallback id.

### Invariants

- morph state is runtime/preview state, not permanently baked into imported map data;
- varbit wins over varp when both fields are present in the audited definition representation;
- null transformation is a valid semantic result;
- transformed definition footprint/model/animation fields replace the original values where the consuming path resolves the transform.

### Failure signature

Doors, crops, quest scenery, and other stateful locs display the wrong variant, never disappear, or retain the wrong footprint after state changes.

### Required tests

Varbit selection, varp selection, in-range transform, fallback transform, null transform, and a transform that changes `sizeX/sizeY`.

### Related specs

`ANIMATION-001`, `LOC-PLACEMENT-003`.

---

## SPEC: ANIMATION-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` at the dynamic-object ownership/model-resolution level; complete sequence-frame math requires its own later decoder/animation sub-specs

### Applies to

Dynamic loc model resolution.

### Required behavior

A dynamic object resolves its active object definition at model time, not only at map-load time.

The audited path:

- loads the original definition;
- resolves its transform chain against current varbit/varp state;
- returns no model when the active transform is null;
- derives footprint from the active transformed definition;
- transposes that footprint for orientations `1` and `3`;
- samples the applicable height/center for the active footprint;
- advances sequence state according to client-cycle/frame rules;
- requests the dynamic-model path using loc type, orientation, height field, position, sequence, and current frame.

When replacement/restart rules permit, the dynamic-object constructor may carry animation state forward from the object it replaces.

The dynamic model path begins from the cached lit base model, creates/uses a safe working model for sequence transformation, and contours the resulting instance when required.

### Invariants

- dynamic animation does not mutate the shared cached base model;
- morph resolution occurs before footprint-dependent height/center calculation;
- static scene normal-reconciliation state is not reused as a mutable animation instance;
- null active morph means null dynamic model.

### Failure signature

Animated doors jumping position, morphing locs retaining stale footprint, one animated object corrupting all instances sharing its base model, animation resetting unnecessarily after valid replacement.

### Required tests

- active transform changes footprint during animation;
- null morph;
- one frame pose with source-cache immutability;
- replacement preserving sequence state when permitted;
- replacement restarting when reference restart rules require it.

### Related specs

`MORPH-001`, `MODEL-BUILD-005`, `CONTOUR-001`.

---

## SPEC: CONTOUR-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `Model.contourGround(...)`  
**Executable evidence:** existing deob contour fixture

### Required terrain-contour behavior

Model contouring samples terrain in 128-unit tile space with the audited integer bilinear interpolation.

For contour clip `0`:

```text
vertexY' = sampledGround + originalVertexY - baseHeight
```

for every applicable vertex.

For nonzero clip, the audited path computes a vertical ratio using integer fixed-point arithmetic based on model height and vertex Y, and only blends vertices satisfying the clip threshold.

The operation respects its copy/mutate contract and resets model bounds after geometry changes.

Reference fast paths may return the original model when contouring cannot change output, including the audited out-of-range/equal-corner-height conditions.

### Integer semantics

- tile size is exactly `128` local units;
- interpolation uses reference integer operation order;
- fixed-point shift/division order is normative;
- do not replace semantic contouring with GPU displacement in parity mode.

### Ownership

Contouring acts on the correct instance/working model. It must never deform a shared canonical base model used by unrelated instances.

### Failure signature

Objects floating above or slicing through sloped terrain, different output at tile boundaries, later instances inheriting another object's ground deformation.

### Required tests

Retain existing golden contour case and add:

- flat fast path;
- all-equal sampled corners;
- nonzero clip threshold cases above/below boundary;
- negative/positive vertex Y;
- adjacent tile interpolation boundary;
- shared-source ownership test.

### Related specs

`COORD-001`, `MODEL-BUILD-005`, `ANIMATION-001`.