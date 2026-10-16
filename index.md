# RustOSRS Documentation Index

Start here.

RustOSRS has a canonical blueprint/specification system plus a current operational status layer. The older `docs/RUNELITE_*.md` files remain useful research, but they are **not implementation authority**.

## Current implementation status

Before starting implementation work, read:

1. `AGENTS.md` and `ARCHITECTURE.md` for contributor navigation and the current ownership contract;
2. `DEV_HANDOFF.md` for the latest exact branch/SHA/CI boundary;
3. `docs/PROJECT-STATUS.md` for the current completion audit and remaining work;
4. `docs/architecture/FEATURE-MAP.md` for existing tool implementations and tests;
5. `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` for dependency/order and milestone reconciliation.

The project is no longer at the blueprint/M0 entry point. M0-M16 have substantial or complete production implementation; the remaining program is concentrated in architecture extraction, Reference parity/certification closure, 117HD completion, export generalization, and project lifecycle/recovery work.

## Canonical reading order

For implementation work, read in this order:

1. `ARCHITECTURE.md` and `AGENTS.md`
2. `DEV_HANDOFF.md`
3. `docs/PROJECT-STATUS.md`
4. `docs/blueprint/04-PROJECT-CHARTER.md`
5. `docs/blueprint/05-SYSTEM-ARCHITECTURE.md`
6. `docs/blueprint/06-CRATE-ARCHITECTURE.md`
7. `docs/blueprint/08-PARITY-MODEL.md`
8. `docs/specs/README.md`
9. the owning semantic spec(s) under `docs/specs/`
10. the relevant accepted ADR(s) under `docs/adr/`
11. `docs/verification/PARITY-MATRIX.md`
12. `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`

The canonical blueprint registry is `docs/blueprint/README.md`.

## Authority hierarchy

| Question | Canonical authority |
|---|---|
| What is the exact current branch/CI/subsystem boundary? | Live Git state, reconciled with `DEV_HANDOFF.md`; only reported CI evidence certifies CI |
| What is implemented and what remains? | `docs/PROJECT-STATUS.md` |
| What OSRS semantic behavior must be reproduced? | `docs/specs/` |
| What are the current repository ownership/coding boundaries? | `ARCHITECTURE.md` |
| Why was a RustOSRS architecture/product decision accepted? | Accepted `docs/adr/` records; blueprint documents supply target/background context |
| Where are a tool's controller, planner, gateway and tests? | `scripts/editor_features.json` + `docs/architecture/FEATURE-MAP.md` |
| What source/revision proves a claim? | `docs/verification/SOURCE-PINS.md` + spec evidence |
| What tests/fixtures prove implementation correctness? | `docs/verification/PARITY-MATRIX.md`, `REFERENCE-FIXTURES.md`, `GOLDEN-SCENES.md` |
| What should be built, and in what order? | `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` |
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
- morphs, animation ownership, and contouring;
- terrain topology and floor definitions;
- bridge/source/storage/collision/render plane distinctions;
- face priority, alpha, authored bias, textures, and UV inputs;
- coordinate and angular conventions.

Each spec carries evidence status, source pins, required behavior, invariants, failure signatures, and required verification.

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
- M17 and M18 remain active completion umbrellas rather than the earlier M10 entry state.

## Legacy research corpus

`docs/RUNELITE_*.md` files predate the canonical blueprint and are retained for source discovery and historical reasoning only. Before using any statement from them, read `docs/research/README.md`.

Known superseded legacy claims include no cross-model normal merging, one universal bridge-adjusted plane, generic floor-decoration lifting, `TEXTURE_COUNT = 256` as semantic truth, stale terrain-builder attribution, and older dependency/crate proposals.

## Imported/reference material

These are evidence/tooling inputs, not automatically normative contracts:

- `runelite-master/`;
- `OpenRune-FileStore-main/`;
- `rs-cache-master/`;
- `docs/api/`;
- `reference-shaders/`;
- `reference-fixtures/`;
- `tools/runelite-mcp/`;
- `tools/deob-harness/`.

Their authority and provenance are classified in `docs/blueprint/03-SOURCE-GROUP-INVENTORY.md` and `docs/verification/SOURCE-PINS.md`.

## How to resolve a disagreement

If two documents disagree:

1. identify the owning domain;
2. verify live Git state and reconcile `DEV_HANDOFF.md`/`PROJECT-STATUS.md` before relying on operational notes;
3. prefer the canonical semantic spec for OSRS behavior;
4. use `ARCHITECTURE.md` for current ownership/coding constraints and the accepted ADR for its rationale;
5. check exact source/fixture provenance;
6. check the contradiction register for unresolved revision issues;
7. if still unresolved, classify it as research/revision-sensitive and do not implement by intuition.

Do not repair an earlier-layer correctness failure with a renderer or editor workaround.
