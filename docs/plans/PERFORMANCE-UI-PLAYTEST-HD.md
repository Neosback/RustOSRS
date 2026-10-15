# Performance, tool UI, offline playtesting and 117HD

Authorized 2026-10-15. Baseline main: `63d2721e3fa30dc2eaecff140d6b27886e4dff2f`.

Stay on main unless explicitly asked otherwise. Preserve Reference semantics and pixels, resolution, viewing distance, current visual settings, transactional editing and native/WASM support. Keep checkpoints focused; update the handoff with actual evidence.

## Sequence and acceptance

1. Extend existing diagnostics with opt-in raw frame/CPU/GPU distributions, build latency, resource accounting and reproducible JSON reports. Run optimized fixed-input baselines with warm-up/repetitions on the M2/8-GB Mac, both profiles, capped and uncapped. Measure idle/animated views, dense/long-distance views, travel/return, editing/rebuilds, water/shadows/interiors. Unavailable counters stay explicitly unavailable. Unified memory counters are never added together.
2. Retain the visual style and egui stack. Reorganize Spline appearance/settings/points/status/actions, share controls with quick HUD, make preview freshness and cancellation explicit, fit palettes to the screen, and audit every tool panel. Verify 240/320/480-point panel widths and multiple display scales. Preserve fitting/shortcuts and one transaction per completed commit.
3. Optimize measured dominant costs, retaining raw before/after samples and identical fixed-camera output. Investigate redraw/picking/extraction, cloning/allocation, uploads/shadows/materials, caches and queues. Preserve dependency-aware invalidation, FIFO freshness, visible animation and current assets. Prove reductions in CPU, GPU and retained/peak memory without invented shared-CI timing thresholds.
4. Add headless osrs-playtest plus editor composition: immutable map/collision snapshots, typed commands, deterministic clock/seed/replay, player movement, NPC routes/animations, projectiles/graphics/attached effects/appearances. Keep scenarios versioned and separate from map/project formats. Extend cache/model/animation support as required. Map edits pause/revalidate; source changes reject stale runtime output. No networking, accounts or full combat/economy.
5. Pin the existing 117HD 1.5.2 corpus by hashes and audit each upstream configuration/shader/data/lifecycle feature. Complete placed/entity lights and portable lossless selection; sky/stars/nebulae/moon/aurora/day-night; reflections/environment coupling; wind/character displacement; material special cases; post/AA/sampling/settings. Independently verify against pinned OSRS/RLHD captures. Backend mechanisms receive explicit equivalent/not-applicable dispositions, not false feature-completion claims.

## Gates

Keep architecture guards, strict native/WASM compile/Clippy, workspace tests and semantic parity green. Add focused lifecycle/freshness/undo, resource lifetime, simulation determinism, light overflow/packing, GPU layout and sky/settings tests. Optimization-only frames compare unchanged inputs on the same backend. Missing independent visual evidence remains an open acceptance item.

## Evidence

No new baseline or performance improvement is claimed yet. Reports and captures are generated under the ignored `target/performance/` directory; durable summaries record exact commits/settings/hashes and link retained evidence.
