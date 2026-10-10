# ADR-0004: Reverse-Z and Canonical Raster Conventions

Status: **Accepted, corrected by M10 foundation audit**  
Date: 2026-10-07  
Correction date: 2026-10-10

## Context

RustOSRS needs one stable raster convention for depth precision, authored face bias, culling, picking, diagnostics, and renderer verification.

The imported RuneLite GPU path uses reverse-Z, strict `GL_GREATER`, clear depth `0`, and an explicit authored face-bias term. The earlier version of this ADR selected `GreaterEqual`; that was a project convenience that was incorrectly described too close to reference behavior. The Reference profile now follows the stricter imported rule.

Raster policy remains renderer-owned. Semantic geometry, winding, face bias, priority, and suppression state remain observable inputs rather than being repaired by the GPU.

## Decision

RustOSRS uses reverse-Z as the baseline renderer convention.

### Reference profile baseline

```text
depth format  = Depth32Float when supported
depth clear   = 0.0
near depth    = larger value
far depth     = smaller value
depth compare = Greater
front face    = CCW
cull mode     = Back
```

`Greater` is intentional. Equal-depth fragments do not pass merely to make multi-pass replay convenient. Any pass requiring equal-depth replay must prove and document its own state rather than weakening the baseline Reference comparison.

### Projection

The imported RuneLite reference matrix is reverse-Z with no finite far-plane term. In its audited form, depth behaves proportionally to `2 * near / z` after perspective division.

RustOSRS wgpu projection must map the equivalent Reference contract into wgpu's `0..1` NDC range and retain the no-far-plane behavior unless a separately versioned renderer profile intentionally differs.

Projection helpers must therefore be tested from the matrix contract, not selected only because a camera library labels a helper "reverse Z."

### Winding

Render extraction normalizes GPU-facing winding to CCW exactly once. Semantic mirroring and transform behavior remain governed by `MODEL-BUILD-*`; negative-scale GPU transforms are not a substitute.

### Authored face bias

Reference profile uses the audited RuneLite-style pre-divide depth adjustment:

```text
position.z += face_bias / 128.0
```

at the same logical stage as the pinned renderer path.

Because the reference projection is perspective/no-far, the resulting normalized-depth separation is distance dependent. The same authored bias does **not** produce a constant screen-depth offset as distance increases.

The semantic `faceBias` remains separately retained. This formula may be superseded only by another ADR with differential evidence.

## Transparency/depth-write boundary

The imported RuneLite GPU snapshot does not globally disable depth writes for blended alpha geometry. Normal depth-tested ranges retain ordinary depth writes; explicit `*_NO_DEPTH` render modes are a separate path.

Therefore the RustOSRS Reference profile must not assume the conventional modern rule "transparent means depth writes off" without a fixture proving that is the intended target behavior.

An Enhanced profile may choose conventional blended/depth-read-only composition, but that choice must be explicitly separate from Reference parity.

## Consequences

Positive:

- Reference depth comparison matches the strongest imported renderer evidence;
- equal-depth behavior becomes testable rather than silently tolerated;
- authored bias retains the correct distance-dependent projection behavior;
- winding/culling defects remain visible;
- picking can share one explicit depth convention.

Costs:

- passes that relied on `GreaterEqual` must be redesigned explicitly;
- alpha composition cannot assume standard depth-write-off behavior;
- reference projection and bias require exact near/far-distance fixtures;
- third-party projection helpers require contract verification.

## Alternatives considered

### `GreaterEqual`

Rejected for Reference mode after the foundation audit because imported RuneLite uses strict `GL_GREATER`. `GreaterEqual` remains available only to a separately documented non-reference path if needed.

### Conventional less-than depth

Rejected as the baseline because it diverges from the strongest renderer evidence and changes precision behavior.

### Disable culling

Rejected for production Reference rendering because it hides semantic winding defects.

### Hardware polygon offset for face bias

Rejected as the canonical parity mechanism because the target input is an authored byte and the imported renderer uses an explicit depth adjustment.

## Required verification

Before M11/M12 may claim Reference raster parity:

- exact matrix tests for near, intermediate, and very distant camera-space depths;
- strict equal-depth test proving the second equal fragment fails under `Greater`;
- ordinary and mirrored winding fixtures;
- terrain rotation culling fixtures;
- authored-bias fixtures at at least two substantially different distances;
- alpha geometry fixture proving Reference depth-write behavior;
- `*_NO_DEPTH` mode/policy fixture if those modes are represented;
- picking-depth equivalence test.

## Supersedes

This correction supersedes the original `GreaterEqual` statement in this ADR and any blueprint text derived from it.
