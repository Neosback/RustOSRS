# ADR-0015: Reviewed building ownership and tool-driven procedural Lab

Status: Accepted

## Decision

`osrs-scene::building_census::description` infers building relationships from canonical placements,
definitions and known terrain coverage. Relationships retain occurrence identity and evidence.
An inferred floor, roof component or picker group does not own edited semantics.

`osrs-edit::prefab` persists reviewed sparse floor and support masks independently of object
anchors. Prefab v1 retains rectangle semantics; v2 may carry building provenance, relationships,
dimensions and human labels. Exact placement and fitted relocation are separate planner entry
points. Fitting rejects unresolved support rather than guessing. Every placement is one patch
through the existing gateway and history.

Native Prefab Studio reuses an isolated stage, renderer and ordinary editing controllers.
History-free initialization uses a transactional gateway baseline; authored history starts empty.
The world document stays parked. The pristine stage floor is excluded when saving.

`osrs-lab` depends only on core, edit and scene, plus serialization. It owns a headless document,
history, private target and tool action trace. Agent actions use planners; they cannot submit raw
patches, paste a target prefab or invoke a target generator. Snapshot/diff APIs are infrastructure
boundaries, never agent actions. The editor projects the Lab document and parks the authored
Studio runtime until Lab exit. Visible-target curriculum and private-target benchmark observations
are distinct. Local benchmark artifacts remain trusted host input, not a security sandbox for an
untrusted process with filesystem access.

Learned catalogs and role models are deterministic, source-stamped data loaded through kits.
Observed compatibility restricts socket compatibility; equal sockets alone do not imply an observed
pair. Human corrections remain authoritative. Unknown, weak and inferred labels are not promoted
to confirmed training labels. Evaluation groups keep geographic and authored-family variants together.

## Consequences

Portable semantics retain WASM compatibility. Native cache-backed capture and isolated Studio
are the initial authoring surface. Renderer caches, visibility groups and training feedback cannot
mutate the authoritative map. New path tasks must move only pure fitting code out of the editor
and retain its existing oracle tests; this decision does not duplicate that fitter in the Lab.
