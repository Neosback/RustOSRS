# Performance, tool UI, offline playtesting and 117HD

Authorized 2026-10-15. Baseline main: `63d2721e3fa30dc2eaecff140d6b27886e4dff2f`.

Stay on main unless explicitly asked otherwise. Preserve Reference semantics and pixels, resolution, viewing distance, current visual settings, transactional editing and native/WASM support. Keep checkpoints focused; update the handoff with actual evidence.

## Current sequencing note

As of 2026-10-16 the user explicitly paused performance measurement/optimization work. Performance tooling may remain in the repository, but checkpoints 1 and 3 below are **not the active development path** until the user resumes them. Current work proceeds through the entity/runtime/playtest source-audit and implementation path in checkpoint 4.

## Sequence and acceptance

1. **Paused.** Extend existing diagnostics with opt-in raw frame/CPU/GPU distributions, build latency, resource accounting and reproducible JSON reports. Run optimized fixed-input baselines with warm-up/repetitions on the M2/8-GB Mac, both profiles, capped and uncapped. Measure idle/animated views, dense/long-distance views, travel/return, editing/rebuilds, water/shadows/interiors. Unavailable counters stay explicitly unavailable. Unified memory counters are never added together.
2. Retain the visual style and egui stack. Reorganize Spline appearance/settings/points/status/actions, share controls with quick HUD, make preview freshness and cancellation explicit, fit palettes to the screen, and audit every tool panel. Verify 240/320/480-point panel widths and multiple display scales. Preserve fitting/shortcuts and one transaction per completed commit.
3. **Paused.** Optimize measured dominant costs, retaining raw before/after samples and identical fixed-camera output. Investigate redraw/picking/extraction, cloning/allocation, uploads/shadows/materials, caches and queues. Preserve dependency-aware invalidation, FIFO freshness, visible animation and current assets. Prove reductions in CPU, GPU and retained/peak memory without invented shared-CI timing thresholds.
4. Add headless `osrs-playtest` plus editor composition around the audited RuneLite/deob runtime split. Use immutable map/collision snapshots, typed commands, deterministic clock/seed/replay and source revision identity. Runtime state owns player/NPC movement, targeting, sequence clocks, var context, temporary actor state, spot-effect lifetimes and projectile simulation. Player/NPC appearance/model assembly resolves reusable base models separately from per-tick pose. Actor-attached spot animations, standalone graphics objects and projectiles share the canonical `SpotAnimationDefinition`/sequence model path. Extend cache/model support for player/NPC definitions, appearance inputs and the cached-model/skeletal animation path before claiming complete actor parity. `osrs-render` receives already resolved/posed instances and never owns packet masks, vars, sequence clocks or projectile physics. CS2/interpreter/widget state is a separate UI runtime concern, not scene-renderer or `MapDocument` authority. Pin the exact player/NPC update-mask dispatchers and recursive interface raster path before claiming update/UI parity. Map edits pause/revalidate; source changes reject stale runtime output. No networking, accounts or full combat/economy.
5. Pin the existing 117HD 1.5.2 corpus by hashes and audit each upstream configuration/shader/data/lifecycle feature. Complete placed/entity lights and portable lossless selection; sky/stars/nebulae/moon/aurora/day-night; reflections/environment coupling; wind/character displacement; material special cases; post/AA/sampling/settings. Independently verify against pinned OSRS/RLHD captures. Backend mechanisms receive explicit equivalent/not-applicable dispositions, not false feature-completion claims.

## Playtest/runtime evidence boundary

The current runtime source study is `docs/research/RUNELITE-RUNTIME-RENDERING.md`. Canonical source-audited contracts are in `docs/specs/actor-runtime-rendering.md`.

The implementation should preserve these separations:

- update/replay input mutates deterministic runtime state;
- runtime state owns movement, clocks, vars, targeting and transient lifetimes;
- definition/model code resolves appearance/morphs and creates posed models without mutating shared bases;
- rendering consumes resolved instances;
- CS2 executes against widget/client UI state and is not the interface rasterizer itself.

Existing legacy-animation M8 coverage does not prove cached-model/skeletal player/NPC parity. Exact player/NPC packet-mask dispatch, complete CS2 opcode routing and recursive interface traversal/raster anchors remain required evidence.

## Gates

Keep architecture guards, strict native/WASM compile/Clippy, workspace tests and semantic parity green. Add focused lifecycle/freshness/undo, resource lifetime, simulation determinism, actor appearance immutability, morph resolution, dual-animation, spot-effect/projectile lifecycle, light overflow/packing, GPU layout and sky/settings tests as those systems are implemented. Optimization-only frames compare unchanged inputs on the same backend. Missing independent visual evidence remains an open acceptance item.

## Evidence

No new baseline or performance improvement is claimed. Performance is currently paused by user direction. Existing reports/captures under ignored `target/performance/` retain only the evidence scope they were originally recorded for.
