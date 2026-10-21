# ADR-0020: Authored document snapshots compose the canonical domains

Status: **Accepted**  
Date: **2026-10-20**

## Context

Map transport and building reproduction do not retain authored NPC scenarios or appearance. Project
v2 stores source-relative map diffs and appearance, not a complete EditorDocument. Project files,
reproduction inputs and recovery journals therefore cannot be treated as interchangeable snapshots.

## Decision

The existing EditorSession owns an explicit native authored-content snapshot of its EditorDocument:
the canonical MapDocument, optional ScenarioDefinition and complete ThemePack. A private wire record
composes existing map transport, scenario serde/validation and deterministic theme ZIP transport.
It introduces no new map/scenario/theme authority, coordinates, history or operation executor.

Fresh validated ScenarioDocument and WorkspaceDocument constructors compose these existing domain
values without applying an authored edit. Decoding creates a new isolated EditorSession; it never
replaces a live session or grants an import action. All session/map/scenario counters begin at their
normal fresh-baseline values. History, dirty/available regions, source/project identity, pending
project diffs, live actors, clocks and projection state remain separate envelope obligations.

[The v1 contract](../specs/authored-snapshot.md) defines a versioned content digest over the canonical
map digest, a versioned complete optional scenario digest and the existing validated ThemePack
identity. Scenario spawn/rule/provenance order is preserved wherever the existing canonical values
preserve it. Absent and present-empty scenarios differ; colocated spawns keep distinct IDs. Assets
retain their exact bytes. No hashing or serialization is added to frame/commit/undo loops.

Explicit File-menu or `project snapshot PATH` capture freezes one owned document. ProjectRuntime
retains one background snapshot export independently of project-save/source replacement state.
Replacement cannot release a busy export slot or rewrite frozen contents. The existing export
staging owner publishes synced complete bytes create-only. Snapshot export never marks project
edits saved, changes project paths or changes history. Normal project saves remain unchanged.

`--verify-authored-snapshot FILE` checks bounded input, canonical domain validation, editor appearance
admission, all declared content identities and a canonical encode/decode round trip before app/GPU/
cache initialization. This is authored-content integrity, not intent replay or render verification.

## Consequences

- Signed/full-width map data, complete scenario metadata and appearance assets can be frozen together.
- Canonical validators and nested formats retain their own policies; no second error/value model is added.
- The closed owned wrapper rejects missing/unknown/duplicate fields and unsupported versions/algorithms.
- Asset paths are never extracted to disk. Snapshots and native reads are capped at 128 MiB.
- A snapshot contains current authoritative values, not every source region or unmaterialized project diff.
- Native and headless readers can inspect a fresh baseline without mutating live authored state.
- Source/clock/availability-aware operation envelopes, recovery, projection/image reproduction and
  independent fidelity remain open. Performance measurement/optimization remains paused.

## Alternatives considered

Extending source-relative ProjectFile to masquerade as full-world reproduction would lose scenario
and exact map values and conflate save/replay/recovery semantics. A parallel workspace model or
snapshot history would compete with existing authority. Serializing theme definitions alone would
lose assets; hashing ZIP bytes would confuse compression/container encoding with authored identity.
