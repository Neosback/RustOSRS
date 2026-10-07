# ADR-0004: Reverse-Z and Canonical Raster Conventions

Status: **Accepted**  
Date: 2026-10-07

## Context

RustOSRS needs one stable raster convention for depth precision, authored face bias, culling, picking, diagnostics, and future renderer tests.

The imported RuneLite GPU path uses reverse-Z and explicitly preserves authored per-face depth bias. That is strong renderer evidence, but the choice is still project-owned renderer policy rather than OSRS semantic behavior.

The semantic specifications also require mirrored model winding and face bias to remain observable rather than being hidden by double-sided rendering or arbitrary polygon offset.

## Decision

RustOSRS uses reverse-Z as the baseline renderer convention.

Baseline state:

```text
depth format  = Depth32Float (when supported by the selected surface path)
depth clear   = 0.0
near depth    = larger value
far depth     = smaller value
depth compare = GreaterEqual
front face    = CCW
cull mode     = Back
```

Projection maps camera depth into wgpu's `0..1` NDC convention.

Render extraction normalizes GPU-facing triangle winding to CCW exactly once. Semantic mirroring and transform behavior remain governed by `MODEL-BUILD-*` and cannot be replaced by negative-scale GPU transforms.

Reference profile initially realizes authored face bias using the audited RuneLite-style clip adjustment:

```text
clip_position.z += face_bias / 128.0
```

before perspective division.

The semantic `faceBias` value remains separately retained; this formula is replaceable only through a superseding ADR if a better wgpu-native realization proves equivalent on fixtures.

## Consequences

Positive:

- strong depth precision across a large editor scene;
- one consistent depth convention for visible rendering and picking;
- authored coplanar ordering has an explicit implementation path;
- no dependence on driver-specific polygon offset as semantic repair;
- winding/culling bugs become visible instead of being hidden by two-sided rendering.

Costs:

- all projection helpers and depth diagnostics must understand reverse-Z;
- third-party camera/projection helpers cannot be adopted without checking their NDC convention;
- authored bias needs exact regression fixtures;
- transparent pass state must be designed around reverse-Z depth testing.

## Alternatives considered

### Conventional less-than depth

Rejected as the baseline because it provides worse precision distribution for the intended large-scene editor and diverges from the strongest renderer reference.

### Disable culling

Rejected for production rendering because it hides mirrored/winding errors and draws semantically back-facing geometry.

Diagnostic pipelines may explicitly disable culling.

### Hardware polygon offset for face bias

Rejected as the canonical parity mechanism because the semantic input is an authored byte and the reference applies an explicit clip-depth adjustment. Driver/rasterizer offset behavior is not an adequate substitute without proof.

## Constraints/invariants affected

- architecture invariant E1: semantic metadata and renderer strategy remain distinct;
- E2: presentation decisions are explicit;
- E3: parity remains observable;
- C4: semantic integer work remains untouched until extraction.

## Follow-up work

Implementation and verification must include:

- near/far reverse-Z matrix tests;
- ordinary and mirrored winding fixtures;
- terrain rotation culling fixtures;
- authored-bias equal-depth fixture;
- picking-depth equivalence test;
- depth diagnostic view.

Verification ownership is detailed in Checkpoint 7.

## Supersedes

None.