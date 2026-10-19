# RustOSRS Documentation Index

Start here.

RustOSRS has a canonical blueprint/specification system plus a current operational status layer. The older `docs/RUNELITE_*.md` files remain useful research, but they are **not implementation authority**.

## Current implementation status

Before starting implementation work, read:

1. `AGENTS.md` and `ARCHITECTURE.md` for contributor navigation and the current ownership contract;
2. `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md` for the preferred long-term technical architecture, performance, memory, projection, rendering, diagnostics, automation, and AI design target;
3. `DEV_HANDOFF.md` for the latest exact branch/SHA/CI boundary;
4. `docs/PROJECT-STATUS.md` for the current completion audit and remaining work;
5. `docs/architecture/FEATURE-MAP.md` for existing tool implementations and tests;
6. `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` for dependency/order and milestone reconciliation.

The technical architecture specification is a target design document, not a substitute for live implementation state, OSRS semantic specifications, or accepted ADRs.

The project is no longer at the blueprint/M0 entry point. M0-M16 have substantial or complete production implementation; the remaining program is concentrated in architecture extraction, Reference parity/certification closure, entity/runtime/playtest work, 117HD completion, export generalization, and project lifecycle/recovery work.

## Canonical reading order

For implementation work, read in this order:

1. `ARCHITECTURE.md` and `AGENTS.md`
2. `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md` for target direction
3. `DEV_HANDOFF.md`
4. `docs/PROJECT-STATUS.md`
5. `docs/blueprint/04-PROJECT-CHARTER.md`
6. `docs/blueprint/05-SYSTEM-ARCHITECTURE.md`
7. `docs/blueprint/06-CRATE-ARCHITECTURE.md`
8. `docs/blueprint/08-PARITY-MODEL.md`
9. `docs/specs/README.md`
10. the owning semantic spec(s) under `docs/specs/`
11. the relevant accepted ADR(s) under `docs/adr/`
12. `docs/verification/PARITY-MATRIX.md`
13. `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`

The canonical blueprint registry is `docs/blueprint/README.md`.

The authorized [Theme Studio and stock-HD completion program](docs/plans/THEME-STUDIO.md) records
the current appearance-authoring checkpoint and the remaining stock/530 implementation and evidence.

## Authority hierarchy

| Question | Canonical authority |
|---|---|
| What is the exact current branch/CI/subsystem boundary? | Live Git state, reconciled with `DEV_HANDOFF.md`; only reported CI evidence certifies CI |
| What is implemented and what remains? | `docs/PROJECT-STATUS.md` |
| What OSRS semantic behavior must be reproduced? | `docs/specs/` |
| What are the current repository ownership/coding boundaries? | `ARCHITECTURE.md` |
| What is the preferred long-term technical architecture and performance/memory design target? | `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md`, subordinate to semantic specs and accepted ADRs |
| Why was a RustOSRS architecture/product decision accepted? | Accepted `docs/adr/` records; blueprint documents supply target/background context |
| Where are a tool's controller, planner, gateway and tests? | `scripts/editor_features.json` + `docs/architecture/FEATURE-MAP.md` |
| What source/revision proves a claim? | `docs/verification/SOURCE-PINS.md` + spec evidence |
| What tests/fixtures prove implementation correctness? | `docs/verification/PARITY-MATRIX.md`, `REFERENCE-FIXTURES.md`, `GOLDEN-SCENES.md` |
| What should be built, and in what order? | `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` plus an explicitly authorized current plan |
| What old claims are superseded or unsafe? | `docs/research/README.md` + `docs/blueprint/02-CONTRADICTION-REGISTER.md` |

A root research document cannot override a canonical spec or accepted ADR. A historical milestone note cannot override current implementation/evidence status.

## Canonical blueprint

`docs/blueprint/` contains:

- evidence and documentation inventory;
- contradiction/open-question register;
- project charter;
- system and crate architecture;
- architecture invariants;
- parity model;
- semantic source audits;
- renderer architecture;
- GPU data/pass contracts;
- editor architecture;
- document/transaction/persistence model;
- editor tool/interaction model;
- verification architecture;
- dependency-ordered implementation roadmap.

See `docs/blueprint/README.md` for the complete registry and checkpoint status.

## Canonical semantic specifications

`docs/specs/` defines atomic semantic contracts spanning:

- loc placement and wall/decor behavior;
- model selection, mirroring, winding, transforms, and ownership;
- normal generation, cross-model normal reconciliation, and lighting;
- morphs, loc animation ownership, and contouring;
- player/NPC actor runtime ownership, appearance/model assembly, dual sequence systems, spot animations, graphics objects, projectiles, and CS2/widget runtime boundaries;
- terrain topology and floor definitions;
- bridge/source/storage/collision/render plane distinctions;
- face priority, alpha, authored bias, textures, and UV inputs;
- coordinate and angular conventions.

Each spec carries evidence status, source pins, required behavior, invariants, failure signatures, and required verification. `docs/specs/actor-runtime-rendering.md` is intentionally source-audited/`DERIVED` until its required executable entity/UI fixtures exist.

## Architecture decisions

`docs/adr/` records project-owned choices separately from OSRS semantics.

Accepted decisions cover reusable `osrs-*` crate boundaries, native-first editor policy, semantic/renderer/editor ownership, reverse-Z/raster conventions, 8x8 zone rendering, Reference vs enhanced profiles, eframe/egui, semantic command history, and distinct project-save/recovery/export workflows.

## Verification

`docs/verification/` is the correctness registry.

Key files:

- `SOURCE-PINS.md` - reproducible source identities and provenance gates;
- `PARITY-MATRIX.md` - semantic/render contracts mapped to evidence and current proof state;
- `REFERENCE-FIXTURES.md` - fixture provenance/format/regeneration rules;
- `GOLDEN-SCENES.md` - composed semantic/render test scenes;
- `CI-FUZZ-BENCHMARKS.md` - CI tiers, property tests, fuzzing, failure injection, and benchmark policy.

Exact semantic tests own P0-P2 correctness. Screenshot tolerance begins only at P3 visual parity. Internal RustOSRS screenshots are regression evidence, not an independent external oracle.

## Implementation roadmap

`docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` defines the historical M0-M18 dependency model and reconciles it with the current implementation.

Important current facts:

- `TERRAIN-004` is implemented and differential-tested against the pinned client;
- a headless/reference wgpu renderer and editor viewport exist;
- editor semantic authoring and verified export exist;
- Reference parity closure and external visual proof remain incomplete;
- entity/runtime/playtest work is now source-audited but not implemented as a complete runtime;
- M17 and M18 remain active completion umbrellas rather than the earlier M10 entry state.

## Current RuneLite source studies

[Actor, animation, projectile and interface runtime](docs/research/RUNELITE-RUNTIME-RENDERING.md) traces the checked-in `RuneLite-melxin` Player/Actor/NPC, appearance, dual-animation, spot-effect, graphics-object, projectile, CS2/interpreter and Widget paths. The promoted source-audited contracts live in `docs/specs/actor-runtime-rendering.md`.

[Tile appearance and paths](docs/research/TILE-APPEARANCE-AND-PATHS.md) traces the pinned client's tile pipeline, maps it to current code, documents the exact Eiken path contract and ranks subsequent appearance improvements. It is research; the semantic specifications remain authoritative.

[RuneLite, deobfuscated-client and 117HD finds (2026-10-18)](docs/research/RUNELITE-DEOB-FINDS-2026-10-18.md) records how the client and RuneLite decide which roofs to draw, what the editor does about it, and the data in those sources that can label buildings. Research; code wins.

[Procgen learning](docs/plans/PROCGEN-LEARNING.md) lists what the building generator's WFC cannot see, the labelled building dataset that now exists (`building_dataset` example) and the phased plan for learned patterns and small offline models. A plan, not an accepted design.

## Legacy research corpus

`docs/RUNELITE_*.md` files predate the canonical blueprint and are retained for source discovery and historical reasoning only. Before using any statement from them, read `docs/research/README.md`.

Known superseded legacy claims include no cross-model normal merging, one universal bridge-adjusted plane, generic floor-decoration lifting, `TEXTURE_COUNT = 256` as semantic truth, stale terrain-builder attribution, and older dependency/crate proposals.

## Imported/reference material

These are evidence/tooling inputs, not automatically normative contracts:

- `RuneLite-melxin/`;
- `rs-cache-master/`;
- `docs/api/`;
- `reference-shaders/`;
- `reference-fixtures/`.

Historical paths such as `runelite-master/`, `OpenRune-FileStore-main/` and `tools/deob-harness/` may be absent from the current tree. Do not cite an absent checkout as live evidence. Their historical authority/provenance remains described in Git history and the source/verification documents.

## How to resolve a disagreement

If two documents disagree:

1. identify the owning domain;
2. verify live Git state and reconcile `DEV_HANDOFF.md`/`PROJECT-STATUS.md` before relying on operational notes;
3. prefer the canonical semantic spec for OSRS behavior;
4. use `ARCHITECTURE.md` for current ownership/coding constraints and the accepted ADR for its rationale;
5. use `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md` for the preferred target architecture only when it does not conflict with higher authority;
6. check exact source/fixture provenance;
7. check the contradiction register for unresolved revision issues;
8. if still unresolved, classify it as research/revision-sensitive and do not implement by intuition.

Do not repair an earlier-layer correctness failure with a renderer or editor workaround.
