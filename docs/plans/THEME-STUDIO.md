# Stock HD completion and Theme Studio

Status: **In progress; the full program is not implemented or accepted.** Authorized 2026-10-16.

The target is stock 117HD completion plus reusable editor themes, with a faithful 530-era look
applied to existing OSRS maps. Full 530 world editing, a RuneLite plugin exporter, a shader graph,
and new game runtimes are outside this program.

## Current checkpoint: appearance authoring foundation

Implemented in the working tree:

- `osrs-hd::theme`: versioned definitions, finite numeric values, ordered property-wise rule
  resolution with winning-rule provenance, validated before/after patches and portable ZIP packs.
  Asset bytes share immutable storage across drafts, library entries and undo snapshots.
- Application-owned `EditorDocument` composes the existing `WorkspaceDocument` and appearance.
  The existing generic `PatchHistory` implementation provides chronological map/NPC/theme history;
  `EditRuntime` retains transaction, rollback and finalization authority. Appearance never advances
  map/collision revision or dirties map regions.
- View → Graphics → Theme Studio: duplicate/new/stock choices; native pack import/export and
  immutable content-addressed library entries; live lighting/fog/water/effect previews; ordered
  rectangular area rules with plane ranges; gesture coalescing, cancellation and comparison.
- Engine-owned vertex, surface, water, sky and post WGSL hooks. Naga validates source and hook
  interfaces; candidate pipelines are checked through an asynchronous GPU error scope before
  activation. Vertex hooks also affect shadow casting. Time-dependent vertex hooks invalidate
  shadow reuse. Shader undo/reopen triggers pipeline restoration from authored authority.
- Native project v2 embeds appearance and accepts v1 projects with stock appearance. Theme-only
  edits can be saved. Cache/map export retains its existing semantic payload.
- All 98 pinned RLHD config defaults now have source anchors and complete default bodies. This
  is a source inventory, **not** implementation reconciliation; all 98 dispositions remain pending.

## Remaining checkpoints

1. Reconcile all stock settings against production code and independent evidence. Implement placed,
   object and supported entity lights with bounded/tiled selection; complete stock sky/celestial/
   day-night behavior, reflections, wind, remaining post/AA/material/settings behavior.
2. Add image-material import/preparation, thumbnails, terrain/object/individual-placement matching,
   viewport Edit appearance and inspector provenance. These types already resolve headlessly, but
   their visual projections are not implemented: editor import/project loading rejects such packs
   explicitly rather than silently discarding their meaning.
3. Implement the revision-specific 530 procedural texture/sprite/metadata importer in `osrs-cache`,
   source-derived Java differential tests, standalone extracted assets and explicit reviewed material
   mappings. The 2008 HD card remains disabled; no approximate preset is presented as a recreation.
4. Finish shader parameter controls/file handling, browser pack dialogs/library persistence,
   complete comparison-time coordination, previews/attribution packaging, independent visual
   comparisons and end-user acceptance of the full authoring sequence.

## Sources and acceptance

- RLHD: `/Users/tylercovalt/Documents/RLHD-master`; authoritative pins remain in the existing HD
  source manifest. `scripts/audit_hd_config.py <path>/HdPluginConfig.java` checks defaults/anchors
  against the pinned configuration hash without upgrading feature statuses.
- 530 cache: `/Users/tylercovalt/Desktop/530 rs2 cache/530Cache`; use its archive metadata as the
  canonical read-only import source. The sibling flat copy is a cross-check. Planning compared all
  1,707 index-8, 680 index-9 and one index-26 containers and found identical container payloads
  allowing version trailers. This does **not** verify texture decoding or the full cache.
- 530 client: `/Users/tylercovalt/Desktop/Arios-530-main/Arios-Client`; texture generation includes
  `Class82`/`Class3_Sub28_Sub20` and texture provider `Class102`. Audit Arios modifications separately
  from original-client behavior, including the README's changed fog/render distance.
- Acceptance requires CPU/source differential checks, actual native/WASM gates, GPU regression
  coverage, independent RLHD/530 visual evidence and the complete editor authoring workflow.
  Missing prerequisites, internal screenshots and green unit tests do not certify that acceptance.

See `DEV_HANDOFF.md` for exact baseline, current local verification and uncommitted state.
