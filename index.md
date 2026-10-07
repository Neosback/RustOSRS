# Index — Map of All Project Documents

Start here. The workspace reference set answers three questions: **what to
port** (file lists), **how it works** (mechanics), and **what's proven**
(executed fixtures). Everything cross-links; this page is the hub.

## Documents

| Document | Answers | Read when |
|---|---|---|
| `RUNELITE_RENDER_SOURCES.md` | The complete file list (Groups A–F), per-file extract + artifact + Rust crate | Scoping work, assigning modules, checking "did we miss a file" |
| `RUNELITE_GPU_PIPELINE.md` | How upload → sort → stream → draw works, uniform table, config features | Implementing `editor_render` |
| `RUNELITE_SCENE_AND_MATERIALS.md` | What each scene/material contract means and which fields mutate | Implementing `editor_core` scene + placement |
| `RUNELITE_RUNTIME_RULES.md` | R1–R27: every unwritten engine rule with pins + formulas | Implementing any behavior; settling "why does it do that" |
| `RUNELITE_DEOB_READING_GUIDE.md` | Deob readability verdicts + cleaned pseudocode + var maps | Porting construction math (shapes, contour, HSL, lighting) |
| `RUNELITE_RUST_PORT_NOTES.md` | Crate map, Rust structs, GL→wgpu table, camera, app shell, defaults, testing, scope | Writing any Rust code |
| `RUNELITE_HARDEST_PARTS.md` | 10 riskiest items ranked, research tasks with acceptance criteria | Planning order of work |
| `RUNELITE_CACHE_STACK.md` | FileStore-vs-rs-cache verdict, opcode tables, port order for `editor_cache` | Cache/decoder work |
| `docs/api/` (49 pages + `README.md`) | Per-class API reference, generated from the snapshot | Looking up any method/field mid-implementation |
| `reference-shaders/` | All 22 GPU shaders + regions table for WGSL porting | Writing shaders |
| `reference-fixtures/deob_golden.txt` | 162 lines of executed engine output (tables, triangulations, fixtures) | Unit-test oracles, settling disputes |
| `tools/runelite-mcp/` | MCP server (API/shader/doc/source queries) + `gen_api_docs.py` | Live reference inside an agent session |
| `tools/deob-harness/` | Headless deob runner (`run.sh` + `Dumper.java` + stubs) | Regenerating fixtures, testing new claims |

## Reading orders

**First-time implementer:** `RUNELITE_RENDER_SOURCES.md` §0 → §10 (build
order) → `RUNELITE_RUST_PORT_NOTES.md` §1–§3 (scaffold + state) →
`RUNELITE_HARDEST_PARTS.md` work order → per-module deep dives as needed.

**Verifying a claim:** `index` rule list below → `RUNELITE_RUNTIME_RULES.md`
rule → `reference-fixtures/deob_golden.txt` executed line (or re-run
`tools/deob-harness/run.sh`).

**Adding a feature:** check `RUNELITE_RUNTIME_RULES.md` §G (remaining gap?)
→ relevant Group F contract → `editor_app` shell notes (§6/§10 of port notes).

## Rule index (R1–R27, one line each)

- R1 loc-type→layer dispatch table · R2 wall edge flags · R3 L-corner dual
  models · R4 decor standoff vectors · R5 diagonal-decor recenter · R6
  footprint center + height sampling · R7 type-gated model selection · R8
  mirror rule · R9 transform pipeline order · R10 dynamic-vs-static branch ·
  R11 slope shading · R12 11×11 underlay blur · R13 overlay rules · R14
  bridge-plane heights · R15 config word + tag · R16 replace-by-layer +
  caches · R17 no normal welding · R18 no occluder culling on GPU · R19 no
  shadows/specular/sounds/collision-in-render · R20 light-constant scales ·
  R21 reverse-Z + authored bias · R22 baked-vs-per-frame split · R23 wall-type
  catalog · R24 decor-type catalog · R25 roofs as typed game objects · R26
  tile capacity + edge masks · R27 item pile slots.
