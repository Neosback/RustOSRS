# RustOSRS Documentation Index

Start here.

RustOSRS now has a canonical blueprint/specification system. The older root-level `RUNELITE_*.md` files remain useful research, but they are **not implementation authority**.

## Canonical reading order

For implementation work, read in this order:

1. `docs/blueprint/04-PROJECT-CHARTER.md`
2. `docs/blueprint/05-SYSTEM-ARCHITECTURE.md`
3. `docs/blueprint/06-CRATE-ARCHITECTURE.md`
4. `docs/blueprint/08-PARITY-MODEL.md`
5. `docs/specs/README.md`
6. the owning semantic spec(s) under `docs/specs/`
7. the relevant accepted ADR(s) under `docs/adr/`
8. `docs/verification/PARITY-MATRIX.md`
9. `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`

The canonical blueprint registry is `docs/blueprint/README.md`.

## Authority hierarchy

| Question | Canonical authority |
|---|---|
| What OSRS semantic behavior must be reproduced? | `docs/specs/` |
| What RustOSRS architecture/product decision is accepted? | `docs/adr/` + owning blueprint document |
| What source/revision proves a claim? | `docs/verification/SOURCE-PINS.md` + spec evidence |
| What tests/fixtures prove implementation correctness? | `docs/verification/PARITY-MATRIX.md`, `REFERENCE-FIXTURES.md`, `GOLDEN-SCENES.md` |
| What should be built, and in what order? | `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` |
| What old claims are superseded or unsafe? | `docs/research/README.md` + `docs/blueprint/02-CONTRADICTION-REGISTER.md` |

A root research document cannot override a canonical spec or accepted ADR.

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

`docs/specs/` currently defines 35 atomic semantic contracts spanning:

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

Accepted decisions currently cover:

- reusable `osrs-*` crate boundaries;
- native-first editor policy;
- semantic/renderer/editor ownership boundaries;
- reverse-Z and raster conventions;
- 8x8 zone-compiled hybrid rendering;
- Reference vs Enhanced render profiles;
- eframe/egui editor shell;
- semantic command/transaction history;
- separation of project save, autosave/recovery, and target export.

## Verification

`docs/verification/` is the correctness registry.

Key files:

- `SOURCE-PINS.md` — reproducible source identities and unresolved provenance gates;
- `PARITY-MATRIX.md` — every semantic spec mapped to required verification and current coverage;
- `REFERENCE-FIXTURES.md` — fixture provenance/format/regeneration rules;
- `GOLDEN-SCENES.md` — composed semantic/render test scenes;
- `CI-FUZZ-BENCHMARKS.md` — CI tiers, property tests, fuzzing, failure injection, and benchmark policy.

Exact semantic tests own P0-P2 correctness. Screenshot tolerance begins only at P3 reference visual parity.

## Implementation roadmap

`docs/blueprint/17-IMPLEMENTATION-ROADMAP.md` defines milestones M0-M18.

Important sequencing rule: the first meaningful wgpu viewport is deliberately delayed until exact cache/model/scene/normal/lighting semantics and differential fixtures exist.

The roadmap also keeps unresolved behavior such as `TERRAIN-004` explicitly gated rather than scheduling a guessed implementation as parity-complete.

## Legacy research corpus

The following files predate the canonical blueprint and are retained for source discovery and historical reasoning only:

- `RUNELITE_RENDER_SOURCES.md`
- `RUNELITE_GPU_PIPELINE.md`
- `RUNELITE_SCENE_AND_MATERIALS.md`
- `RUNELITE_RUNTIME_RULES.md`
- `RUNELITE_DEOB_READING_GUIDE.md`
- `RUNELITE_RUST_PORT_NOTES.md`
- `RUNELITE_HARDEST_PARTS.md`
- `RUNELITE_CACHE_STACK.md`

Before using any statement from them, read `docs/research/README.md`.

Known superseded legacy claims include, among others:

- no cross-model normal merging;
- `editor_core`/`editor_render` as canonical crate names;
- mandatory wasm support;
- one universal bridge-adjusted plane;
- generic floor-decoration height lifting;
- `TEXTURE_COUNT = 256` as semantic truth;
- stale `class470` terrain-builder attribution;
- “FileStore is the spec”;
- the old `rs-cache` hybrid recommendation as an already accepted dependency decision.

## Imported/reference material

These are evidence/tooling inputs, not automatically normative contracts:

- `runelite-master/`
- `OpenRune-FileStore-main/`
- `rs-cache-master/`
- `docs/api/`
- `reference-shaders/`
- `reference-fixtures/`
- `tools/runelite-mcp/`
- `tools/deob-harness/`

Their authority and provenance are classified in:

- `docs/blueprint/03-SOURCE-GROUP-INVENTORY.md`
- `docs/verification/SOURCE-PINS.md`

## How to resolve a disagreement

If two documents disagree:

1. identify the owning domain;
2. prefer the canonical semantic spec for OSRS behavior;
3. prefer the accepted ADR for project-owned design;
4. check exact source/fixture provenance;
5. check the contradiction register for known unresolved revision issues;
6. if still unresolved, classify it as research/revision-sensitive and do not implement by intuition.

Do not repair an earlier-layer correctness failure with a renderer or editor workaround.
