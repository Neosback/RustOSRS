# ADR-0006: Reference and Enhanced Render Profiles

Status: **Accepted**  
Date: 2026-10-07

## Context

RustOSRS has two legitimate rendering goals that should not be conflated:

1. reproduce audited/reference behavior closely enough to diagnose semantic and renderer parity;
2. provide a high-quality modern editor viewport that may intentionally improve presentation.

If those goals share one implicit set of settings, visual improvements can hide parity regressions or force the editor to remain artificially limited.

## Decision

`osrs-render` exposes two explicit renderer profiles built on the same semantic scene and renderer architecture.

## Reference profile

Purpose: deterministic parity and regression testing.

Reference profile requirements:

- semantic/model-built HSL/color output remains authoritative;
- no unrequested modern relighting;
- authored face priority, alpha, bias, textures, and UV behavior remain observable;
- animation is driven by explicit deterministic tick input;
- camera/projection inputs are explicit and reproducible;
- MSAA defaults to 1x for screenshot fixtures;
- optional fog, color processing, filtering, or similar presentation behavior is enabled only when the fixture/profile explicitly requests it;
- diagnostic overlays are disabled in golden output unless they are the thing being tested.

## Enhanced editor profile

Purpose: production editor usability and visual quality.

It may enable:

- 4x MSAA when supported;
- anisotropic filtering;
- smoother camera behavior;
- enhanced fog/presentation;
- optional non-destructive lighting presentation;
- higher-quality texture sampling;
- editor overlays and diagnostic visualizations.

Enhanced behavior must not mutate the semantic scene, saved map data, or the reference-profile material/color payload.

## Shared requirements

Both profiles use:

- the same loc/model/terrain semantic scene;
- the same bridge/plane state;
- the same texture/material identities;
- the same face metadata;
- the same static/dynamic renderer architecture;
- the same picking identity mapping.

A feature that requires a different semantic scene representation is not merely a render profile option and requires separate architectural review.

## Consequences

Positive:

- parity remains measurable even as editor visuals improve;
- renderer enhancements do not need to be rejected merely because OSRS did not use them;
- golden tests can lock deterministic settings;
- users can inspect a reference-looking scene and a polished editing view without re-importing/rebuilding semantic content.

Costs:

- some pipelines/material states may need profile variants;
- QA must test both reference and enhanced paths;
- optional enhanced lighting must avoid double-lighting already baked reference colors.

## Alternatives considered

### One renderer mode with many unrelated toggles

Rejected because it makes the actual parity contract difficult to reproduce and reason about.

### Reference-only renderer

Rejected because the editor is allowed to provide modern presentation improvements where semantics remain intact.

### Enhanced-only renderer

Rejected because semantic/rendering regressions would become harder to diagnose and compare with source/reference behavior.

## Constraints/invariants affected

- E2: presentation improvements are explicit;
- E3: parity remains observable;
- D1: semantic scene is independent of renderer profile;
- C1: semantic metadata is retained rather than baked away by enhanced presentation.

## Follow-up work

Checkpoint 6 will decide how the editor exposes profile switching and viewport settings.

Checkpoint 7 will define deterministic golden-scene configuration for reference profile and non-regression tests for enhanced profile.

## Supersedes

None.