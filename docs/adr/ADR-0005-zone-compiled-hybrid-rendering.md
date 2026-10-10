# ADR-0005: Zone-Compiled Hybrid Rendering

Status: **Accepted, clarified by M10 foundation audit**  
Date: 2026-10-07  
Clarification date: 2026-10-10

## Context

The editor needs large-scene performance without turning GPU buffers into source truth. Most map geometry is static for long periods, while some content remains animated, morph-dependent, transparent, priority-sensitive, or temporarily edited.

The imported RuneLite renderer demonstrates that chunk/zone-style static compilation is practical, but its exact face sorting is path-specific. It must not be cited as evidence that all RuneLite GPU geometry receives the software client's full priority-queue algorithm.

RustOSRS deliberately preserves a stronger separation:

- semantic/model truth;
- software/client Reference ordering rules;
- imported RuneLite GPU behavior;
- RustOSRS renderer policy.

## Decision

RustOSRS uses an **8x8 tile zone** as the primary static renderer compilation unit and combines it with dynamic and ordered paths.

### Zone-compiled static path

Used for extracted geometry whose appearance is stable enough to compile into reusable zone artifacts, including ordinary opaque terrain and static loc geometry that does not require camera-dependent ordered treatment.

Static compilation never discards priority, alpha, bias, texture, suppression, identity, or plane provenance needed by a later Reference/diagnostic path.

### Dynamic path

Used when geometry/material preparation changes at runtime but does not require ordered face emission, for example animation frames, morph regeneration, or temporary preview geometry.

### Ordered path

Used when the selected RustOSRS render profile requires camera-relative ordering, including priority-sensitive software-reference behavior and transparency ordering.

For the **RustOSRS Reference profile**, priority-sensitive content may use the exact pinned `Model.method5946` software/client algorithm even where imported RuneLite GPU would instead rely on depth or a simpler depth-only sort. This is an intentional Reference-policy decision, not a claim that RuneLite GPU always behaves that way.

For an explicit **RuneLite-GPU comparison profile**, classification may instead reproduce imported RuneLite render modes and path-specific sorting.

## Imported RuneLite sorting distinction

The imported snapshot exposes render modes `DEFAULT`, `SORTED`, `SORTED_NO_DEPTH`, `UNSORTED`, and `UNSORTED_NO_DEPTH`.

Its `ModelUploader` only enables the priority-queue branch when the caller supplies `prioritySort=true`; the audited `GpuPlugin` does so for `SORTED_NO_DEPTH`. Ordinary dynamic upload passes `false`, static opaque upload does not universally apply priority sorting, and alpha/static work in `Zone` is primarily distance/depth ordered.

This distinction is normative documentation for provenance. It prevents future developers from using "RuneLite does priority sorting" as an overbroad implementation justification.

## Dirty model

Semantic edits publish invalidation. Renderer zone tracking:

1. starts from an immutable semantic generation;
2. invalidates every affected 8x8 storage-plane zone;
3. allows unaffected zones to remain reusable across a newer global generation;
4. rejects stale work for a zone invalidated by a newer generation;
5. uploads/retires GPU resources only after CPU structural correctness is established.

Camera movement, texture animation tick, and ordinary visibility filters do not mutate semantic zones.

## Consequences

Positive:

- bounded rebuild work;
- chunk-aligned spatial ownership;
- renderer artifacts remain reconstructible;
- software/client priority parity can be preserved where required;
- RuneLite-GPU differential behavior can be represented separately rather than conflated;
- static batching cannot erase diagnostic/parity metadata.

Costs:

- ordered/static side metadata is required;
- cross-zone footprints and material dependencies need explicit invalidation;
- Reference and RuneLite-GPU comparison profiles may classify some content differently;
- more lifecycle bookkeeping than a monolithic buffer.

## Required invariants

- static/dynamic/ordered describes renderer stability/policy, not semantic object type;
- suppressed semantic faces remain suppressed in every profile;
- zone compilation cannot change semantic hashes;
- switching renderer profile cannot mutate semantic scene state;
- a future optimization may change scheduling but not the selected profile's observable ordering contract.

## Required verification

- 8x8 keying including negative semantic tile coordinates;
- cross-zone footprint invalidation;
- stale-zone generation rejection;
- partial reuse of unaffected zones across generations;
- static/dynamic/ordered classification with explicit reasons;
- exact software priority fixture for Reference ordered content;
- separate imported RuneLite render-mode fixture before any RuneLite-GPU parity claim;
- semantic hash unchanged by classification/grouping/zone compilation.

## Supersedes

This clarification supersedes any reading of the original ADR that treated imported RuneLite GPU as universal evidence for software priority ordering.
