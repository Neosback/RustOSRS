# 117HD port plan

Status: **Current through 2026-10-15 audit**  
Profile: RustOSRS `HD`, separate from the Reference parity profile

Source basis: vendored 117HD/RLHD resources under `assets/117hd/`, with BSD-2 attribution retained in `THIRD_PARTY.md` and ported shader headers.

Goal: port the intended 117HD look and systems into RustOSRS's own wgpu engine without changing Reference semantic/render authority.

For the project-wide completion program see `docs/PROJECT-STATUS.md`.

## 1. Non-negotiable boundary

117HD is an enhanced/stylized presentation profile, not the Reference oracle.

Rules:

- Reference behavior must remain independently testable and unchanged by HD state;
- semantic world state is shared, presentation state is not semantic authority;
- HD regressions use data/unit tests and fixed-camera visual regression/comparison;
- a visually pleasing HD result cannot justify a Reference semantic/raster workaround.

## 2. Port architecture

RustOSRS owns the semantic and extraction pipeline, so it does not need to reproduce every RuneLite plugin workaround.

Current flow:

```text
cache / semantic scene
        |
        v
     osrs-world
   /             \
Reference       HD extraction
   |             |
   v             v
osrs-render Reference + HD GPU profiles
        |
        v
     osrs-editor
```

The HD profile uses its own material/environment/water/shadow resources and HDR frame path while leaving Reference geometry/presentation contracts independently available.

## 3. Source inventory

The vendored 117HD corpus includes:

- scene, shadow, water, sky, light, post, and utility shaders;
- material/ground-material/tile/model override data;
- areas and environments;
- placed lights;
- water types and sky presets;
- gameval mappings;
- diffuse, normal, height, flow, foam, and related textures.

Rust loaders and resolver tests live primarily in `osrs-hd`, with extraction in `osrs-world` and GPU realization in `osrs-render`.

## 4. Phase status

| Phase | Scope | Current state |
|---|---|---|
| 0 | render profile, HDR/frame graph, material GPU foundation | COMPLETE |
| 1 | 117HD data loaders/resolution | COMPLETE |
| 2 | HD terrain/model materials, normals, blending | COMPLETE baseline + later hardening |
| 3 | environments/fog/directional lighting | COMPLETE baseline |
| 4 | directional shadows/filtering | COMPLETE baseline |
| 5 | water/flow/foam/Fresnel/underwater/caustics | COMPLETE baseline |
| 6 | placed/dynamic point lights | NOT COMPLETE |
| 7 | sky/star/moon/aurora/day-night | NOT COMPLETE |
| 8 | post/settings/wind/remaining presentation | PARTIAL |

Several features originally listed under phase 8 were implemented early, notably parallax/displacement and additional material-flow behavior. Phase numbering therefore describes ownership, not strict chronological order.

## 5. Implemented HD systems

### 5.1 Foundation and data

Implemented:

- `RenderProfile::{Reference, Hd}`;
- HDR offscreen/frame-graph path for HD;
- GPU material storage/bindings and large texture-array support;
- vendored resources and licensing notices;
- loaders/resolvers for materials, ground materials, tile/model overrides, areas, environments, water types, sky presets, placed-light data, and gamevals;
- settings/data expression evaluation required by the imported resource definitions.

Reference pixel regression remains independent of the HD path.

### 5.2 Terrain/model extraction and materials

Implemented:

- HD vertex stream alongside Reference geometry;
- terrain/model HD extraction in `osrs-world`;
- procedural terrain normals;
- ground material/color blending;
- water-depth data;
- material replacement/routing;
- model overrides and supported UV modes;
- diffuse/normal/specular/AO-style material sampling;
- animated-loc HD material routing;
- material flow-map support;
- cutout materials in depth-correct opaque handling where appropriate;
- HD debug views and fixed-camera regression coverage.

### 5.3 Environments and directional lighting

Implemented:

- per-area environment selection;
- environment state transitions/blending;
- ambient and directional lighting;
- fog/ground fog and atmospheric color behavior;
- seasonal/environment controls already represented by current settings;
- lightning-related environment contribution where supported.

Full time-of-day sky integration is still phase 7 work.

### 5.4 Directional shadows

Implemented:

- directional shadow maps;
- terrain shadow map path;
- PCF/PCSS-style filtering options;
- transparency-aware shadow behavior;
- receiver/normal bias behavior;
- edge fade;
- model override shadow controls;
- baked/fake vanilla-shadow hiding where supported;
- shadow-map reuse/caching and stepped light-camera updates.

Remaining visual parity cleanup must not be confused with the missing point-light system.

### 5.5 Water and underwater

Implemented:

- water surface extraction;
- water-type GPU data;
- normal and flow maps;
- foam/shoreline behavior;
- Fresnel response;
- directional-light/specular contribution;
- underwater tint/depth fade;
- underwater flow distortion;
- caustics;
- draw-distance border depth clamping;
- Water settings and regression tests.

Still missing from the complete 117HD water look:

- point-light contribution from phase 6;
- sky/environment reflection integration from phase 7.

### 5.6 Parallax/displacement

Implemented after the phase-5 baseline:

- RLHD-style parallax occlusion/displacement;
- view-angle-dependent displacement layers;
- displacement across the blended material inputs;
- displaced fragment position used for shadow lookup;
- independent editor toggle, default enabled;
- regression coverage for shader ordering and displaced shadows.

This removes the old plan statement that parallax was simply future phase-8 work.

## 6. Remaining work

### Phase 6: placed/dynamic lights

Highest-priority missing visual system.

Required:

- instantiate placed lights from `lights.json` against world/area/object context;
- upload light data efficiently;
- tiled/clustered/compute or otherwise bounded light selection for fragments;
- shadow/normal/material interaction consistent with the chosen 117HD reference behavior;
- begin with static/placed lights if NPC/projectile lifecycle is not yet represented in the editor;
- add deterministic data/selection tests and fixed-camera visual fixtures.

This is especially important for caves/interiors whose intended 117HD environment relies heavily on placed lights.

### Phase 7: sky and time-of-day

Required:

- sky shader/system;
- starfield/nebula behavior as included in supported scope;
- moon;
- aurora;
- seasonal sky behavior;
- time-of-day/day-night transitions;
- environment coupling;
- water/sky reflection integration;
- deterministic time override for regression tests.

### Phase 8: remaining post/settings/presentation

Some phase-8 work already exists, including parallax/displacement and portions of color/settings behavior.

Remaining likely scope:

- complete tone/color/filter behavior intended for the supported profile;
- remaining AA/settings UX decisions;
- wind and character/vegetation displacement;
- remaining model/material edge cases;
- remaining settings sections and persistence parity;
- performance cleanup that preserves visual contracts.

## 7. Known technical risks

### GPU limits

The material texture array is large. Maintain explicit device-limit checks and avoid silently lowering fidelity on unsupported hardware.

### Cross-platform WGSL behavior

Keep shader behavior valid on the project's supported wgpu backends. Do not depend on geometry shaders or backend-specific features without an explicit portability decision.

### Performance

Primary cost is fragment/shadow complexity rather than only draw-call count. Optimize measured bottlenecks, and preserve deterministic/reference-visible behavior while doing so.

### Data synchronization

Keep vendored upstream data reproducibly pinned and diffable. Avoid hand-editing imported source data when a resolver/override layer can own RustOSRS-specific behavior.

### Licensing

BSD-2 attribution and third-party notices must remain with derived shader/data work.

## 8. Verification requirements

Each remaining HD subsystem should have all applicable layers:

1. resolver/data unit tests;
2. deterministic CPU selection/packing tests;
3. GPU layout/pipeline tests where practical;
4. fixed-camera RustOSRS regression images;
5. comparison against 117HD/RLHD reference captures for representative scenes;
6. a Reference-profile regression proving the HD change did not alter Reference output.

HD comparison tolerances must not leak into semantic P0-P2 verification.

## 9. Recommended next HD sequence

Do not start these while another subsystem branch is active.

When the project deliberately returns to 117HD work:

1. phase 6 placed/static lights;
2. phase 6 culling/performance and light/material integration;
3. phase 7 sky + deterministic time model;
4. phase 7 water reflection/environment coupling;
5. phase 8 wind/displacement remainder;
6. phase 8 post/settings completion;
7. final representative-scene comparison and performance pass.

The project-wide roadmap currently recommends finishing the remaining editor runtime ownership extraction and Reference parity/certification work before resuming large 117HD subsystems.
