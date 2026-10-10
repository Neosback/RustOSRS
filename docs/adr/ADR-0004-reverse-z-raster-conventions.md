# ADR-0004: Reverse-Z and Canonical Raster Conventions

Status: **Accepted, amended 2026-10-10**  
Date: 2026-10-07  
Amendment provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`

## Context

RustOSRS needs one stable raster convention for depth precision, authored face bias, culling, picking, diagnostics, and reference tests.

The imported RuneLite GPU path uses reverse-Z and explicitly preserves authored per-face depth bias. A post-M9 source audit corrected one earlier mismatch: the captured RuneLite baseline uses strict `GL_GREATER`, not a greater-or-equal comparison.

These are renderer decisions. Semantic mirroring, winding, face bias, priority, and alpha remain independently preserved before rasterization.

## Decision

RustOSRS uses reverse-Z as the baseline renderer convention.

Reference-profile baseline state:

```text
depth format  = Depth32Float (when supported by the selected surface path)
depth clear   = 0.0
near depth    = larger value
far depth     = smaller value
depth compare = Greater
front face    = CCW
cull mode     = Back
```

Projection maps camera depth into wgpu's `0..1` NDC convention.

`Greater` is the Reference-profile parity baseline because it matches the captured RuneLite `GL_GREATER` state. `GreaterEqual` may exist only as an explicitly named enhanced/alternative renderer policy and must not be described as exact RuneLite depth-state parity.

Render extraction normalizes GPU-facing triangle winding to CCW exactly once. Semantic mirroring and transform behavior remain governed by `MODEL-BUILD-*` and cannot be replaced by negative-scale GPU transforms.

Reference profile initially realizes authored face bias using the audited RuneLite-style clip adjustment:

```text
clip_position.z += face_bias / 128.0
```

before perspective division.

The semantic `faceBias` value remains separately retained. This formula is replaceable only through a superseding ADR if another realization proves equivalent on fixtures.

## Equal-depth implication

Changing `GreaterEqual` to strict `Greater` is behaviorally significant for truly equal-depth fragments. Ordered emission, authored face bias, and no-depth/reference render modes therefore need explicit fixtures. The renderer must not depend on equality acceptance to repair missing priority/bias behavior.

## Alpha/depth-write boundary

This ADR does not assert that every transparent draw disables depth writes. The captured RuneLite renderer has path/render-mode-specific depth behavior. Reference alpha/depth state must be established by the owning pass/render-mode contract and fixtures rather than by importing a generic conventional-transparency assumption.

## Consequences

Positive:

- reverse-Z precision across large editor scenes;
- strict depth comparison now matches the captured renderer reference;
- authored coplanar ordering remains explicit;
- winding/culling bugs remain observable;
- no reliance on driver-specific polygon offset as semantic repair.

Costs:

- all projection/depth diagnostics must understand reverse-Z;
- equal-depth replay cannot rely on `GreaterEqual` in Reference mode;
- authored bias needs exact near/far and coplanar regressions;
- transparent/no-depth pipelines need explicit path-specific state.

## Alternatives considered

### GreaterEqual reference baseline

Previously selected to tolerate intentional equal-depth replay. Rejected by this amendment as the Reference-profile baseline because the captured RuneLite renderer uses strict `GL_GREATER`. It may remain available only as a separately named non-reference policy if useful.

### Conventional less-than depth

Rejected as the baseline because it diverges from the strongest renderer reference and provides a less suitable precision distribution for the intended large scene.

### Disable culling

Rejected for production rendering because it hides mirrored/winding errors. Diagnostic pipelines may explicitly disable culling.

### Hardware polygon offset for face bias

Rejected as the canonical parity mechanism because the semantic input is an authored byte and the reference applies an explicit clip-depth adjustment.

## Required verification

Implementation must include:

- near/far reverse-Z matrix tests;
- exact strict-equality depth-state test;
- ordinary and mirrored winding fixtures;
- terrain rotation culling fixtures;
- authored-bias equal-depth fixture;
- authored-bias distance/projection fixture;
- picking-depth equivalence test;
- depth diagnostic view;
- renderer-mode tests for any no-depth/transparent exceptions.

## Supersedes

This amendment supersedes the original `GreaterEqual` Reference-profile choice inside ADR-0004. All other accepted reverse-Z decisions remain in force.
