# Technical architecture audit and rollout

Status: local projection correctness, normal-fixture execution, accepted static/inspection/animation/actor artifact identity, building/placement/capture admission and per-owner worker bounds, canonical map/patch digests/transport, successful/rejected-building reproduction and composed authored-content snapshots are implemented; broader rollout remains open.
Audit date: 2026-10-20. Baseline: `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`.

The [target specification](RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md) already existed at the baseline;
the supplied attachment was byte-identical. This audit is subordinate to semantic specs,
[ARCHITECTURE.md](../../ARCHITECTURE.md), and accepted ADRs. It is not release certification.
Live Git state and the [handoff](../../DEV_HANDOFF.md) control implementation/verification facts.

## Findings against the target

| Target area | Observed implementation and evidence | Remaining boundary |
|---|---|---|
| Authority, transactions and history (§§3–5, 17–18) | `EditorSession`, `EditRuntime`, descriptor-driven replay and one chronology are established; [authority audit](AUTHORITY-AUDIT.md), `check_authority.py`, session/gateway/replay tests | Transitional preview/worker mirrors remain disposable; no history rewrite is needed |
| Precise projection (§§7–8) | `world_runtime/invalidation.rs`: radius-5 terrain expansion, accepted Height read-sets, Object output comparisons, collision-region fallback; invalidation/build tests | Scoped output still builds the full contextual scene; dependencies outside proofs must retain fallback |
| Async freshness (§§8, 12) | Single FIFO `RegionStreamer`, editor `BuildTracker` request ledger, projection epochs and presentation generations; cancellation/replacement tests | This proves the accepted FIFO contract, not out-of-order worker readiness; source/revision observations must not silently become acceptance policy |
| Region installation (§§1, 3, 23) | Baseline `StreamEvent::Region` reported semantic adaptation failure and continued to GPU/picker/animation/dependency installation | First checkpoint below gates every candidate install on successful semantic adaptation |
| Rendering/resource lifetime (§§9–11, 16, 22) | Zone uploads, capacity-based buffer reuse, Reference frame reuse, snapshot/generation primitives, resource accounting and timestamp queries exist | Static/inspection/animation/actors retain accepted provenance; preview/repro identity remains open. Paged allocation is unimplemented, not automatically required |
| Fidelity (§§2, 15–16, 26) | Extensive semantic/reference fixtures and oracle tooling; [parity matrix](../verification/PARITY-MATRIX.md) distinguishes independent proof | External visual oracle and remaining raster, alpha, texture, normal/roof edge proof remain open |
| Memory/performance (§§6–7, 12–13, 20–21) | Distance-based eviction, dirty semantic retention, bounded in-flight region count, instrumentation and comparison scripts exist | No proven general memory-pressure budget or representative calibrated baseline; message channels themselves are unbounded |
| Persistence/reproduction (§§14, 19–20, 23) | Project diffs, staged export, Lab replay, canonical map/patch transport, building success/rejection replay and complete current map/scenario/appearance snapshots exist | Source/availability/clock-aware workspace operation envelopes, other features, projection reproduction and recovery remain open; save, journal and replay stay separate |
| Navigation/build (§§24–26, 29) | Feature registry, dependency/privacy/file-size ratchets, authority guards, unified verification and WASM checks exist | Several operational/migration documents lag code; source checks do not prove behavior or parity |
| Automation/MCP (§§18, 27–28) | Shared planners, scripts, feature discovery, headless Lab operations and building reproduction verification provide foundations | No complete editor-wide headless operation contract or thin MCP adapter; preserve normal mutation/trust boundaries |

Implementation presence is not an assertion that every required acceptance test passed at the audit
HEAD. In particular, internal regression images are not independent client visual evidence.

## Checkpoint 1: semantic candidate rejection

Application composition lives in `streaming/region_install.rs`; the semantic adapter remains
`session::adapt_region_map`. FIFO retirement and existing freshness gates precede adaptation.
Only successful adaptation may resolve Object comparison scope, hydrate the session or publish
candidate outputs/project restoration work. `PreservedDirty` remains a successful synchronization
outcome and cannot overwrite authored state.

On adaptation failure, `WorldRuntime::region_unavailable` (originally `reject_region_candidate`) clears the entire region's residency,
picker/outlines, animation membership, dependency proofs and pending Object comparison, including
for scoped requests. The renderer removes every static zone of the region. Because animated locs
share one aggregate batch, rejection clears that batch temporarily and discards an already-in-flight
old animation completion. The next forced animation build contains only surviving regions.

The unavailable residency marker includes rejected candidates and suppresses a per-frame request
loop. Rebuild, source replacement, eviction/re-entry and a later semantic invalidation permit retry.
A tile edit over an unavailable region requests full recovery, since no static zones remain to
support a partial refresh. The error notice names the region and adaptation error. Rejection does
not modify semantic documents, history/revisions/dirty state, or pending project diffs.

There is no public worker-protocol or project-format change. Animation scheduling was moved into
its existing runtime's `animation.rs` module to keep the production parent under its file-size cap.

## Checkpoint 2: active projection contracts

[The lifecycle inventory](PROJECTION-CONTRACTS.md) records semantic-impact consumers and map/viewport
artifacts, including the difference between last accepted visual data and current collision proof.
It maps each owner to its source, acceptance identity, dependency expansion, rebuild/removal policy
and existing or new regression evidence. Actor supersession and terminal-failure presentation are
addressed in the following checkpoints; integrated artifact identity remains open.

Cross-owner regressions reproduced two correctness defects: membership replacement could publish
an older in-flight animation batch, and superseding a zone build could lose its original required
area. The fixes reuse the existing single-flight animation discard mechanism and retain dirty
region/zone obligations until a current terminal completion. Full work subsumes zones, but a newer
zone edit still advances the epoch of an in-flight full build. Full presentation/source rebuilds
also clear the shared animated GPU layer. No worker protocol, scheduling priority, semantic history
or conservative dependency fallback changed; no performance improvement is claimed.

## Checkpoint 3: actor completion supersession

Player/NPC completions are staged until actor owners reconcile the frame's actual settings,
authored revisions, queued restart commands and accepted ground version. Only the matching issued
player token or NPC snapshot stamp can retire an in-flight slot. Hiding revokes installation while
allowing the physical request to drain; old geometry and diagnostics cannot restore a hidden layer.
NPC camera, range, draw limit and vars join the extraction key and publication checks. Explicit
player teleport revokes pending output. Ordinary requested animation/tick lag is allowed so ongoing
simulation does not starve rendering. No clock or worker protocol changed.

The new arrival/update regressions use actual transient GPU uploads with synthetic geometry. They
exercise stale slot protection, hide/show, source, map/scenario/run changes, extraction inputs,
replacement recovery and ordinary clock advancement. They do not certify model extraction or
independent visual parity. Terminal worker-failure policy is implemented in the following checkpoint.

## Checkpoint 4: terminal region cleanup and retry

Current Empty/Failed completions pass the existing FIFO/epoch/presentation gate, then share semantic
rejection's complete unavailable-region cleanup. Both full and scoped outcomes hide the whole
producer, clear static and shared animated GPU geometry, and remove picker/collision/outlines,
animation membership, dependency proofs and pending Object comparison. The aggregate animation
worker rebuilds surviving membership. Empty stays quiet; Failed reports its coordinate and error.

Empty represents absent producer source, not a successful scoped extraction with zero triangles.
It no longer unloads even a clean semantic document. Neither terminal outcome changes the complete
semantic session or consumes pending project diffs. Existing unavailable residency suppresses
per-frame retry, while rebuild/source replacement/eviction/re-entry/new semantic invalidation can
retry. A later zone edit requests full recovery. Stale terminal results cannot clear newer projections,
requests, required work, selection or notices. No worker protocol, public API or persistence change.

Native GPU regressions cover Empty/Failed × full/scoped × clean/dirty sessions, actual removal of
zones beyond scoped output, valid recovery/project restoration, obsolete presentation/epoch results
and completions without a ticket. Successful zero-triangle scoped output keeps unaffected zones.
Existing CPU lifecycle/proof/outline/animation tests exercise the same
shared removal operation. Fidelity evidence is the next rollout phase; integrated render identity
and independent visual parity remain unproven.

## Checkpoint 5: canonical normal parity gate

The canonical inventory runner still classified base/merge normals as evidence-only despite the
production M7 implementation and hand-listed M7 parity tests. Its normal adapters now execute the
existing `osrs-core::normals` owner, preserve nullable/allocated output shapes and compare exact
normalized values. All ten normal manifests are semantic; downgrade and corrupt-expectation tests
prevent skipping those comparisons. Malformed models still pass through canonical model validation.
The existing plane adapter moved unchanged to `runner/planes.rs`, keeping the runner below 500 lines.
No production algorithm, dependency boundary, fixture schema or rendering behavior changed.

A [pinned Java method adapter](../../scripts/normal_oracle/README.md) independently replays the five
historical normal fixtures and generates five edge cases: mixed signed face normals, two-vertex
matches, flat-neighbor exclusion, signed XYZ translation and duplicate matching-pair accumulation.
Original method bodies compile unchanged in a storage-only shell; source hashes and complete adapter
identity are pinned. Candidate output cannot overwrite the accepted fixture tree. Offline Python
safety tests run in the shared verification definition; Java remains an explicit prerequisite.

This closes an inventory-execution gap and expands exact P1 numeric evidence. It does not establish
scene-neighbor traversal, roof/raster/texture closure or independent visual parity. Source revision
235 is declared explicitly and is not relabeled as build-241 client proof. The next grounded rollout
boundary remains world-to-render artifact identity; broader fidelity proof stays open in parallel.

## Checkpoint 6: accepted static/inspection artifact identity

The existing `BuildStamp` now survives accepted FIFO completion. `RegionGeometry` carries canonical
source `TargetProvenance` and its actual `TerrainPresentation`; its existing `RegionRenderPath`
distinguishes Reference, HD and fallback. After semantic adaptation, a private immutable descriptor
binds those inputs to the issued request/source/revision/producer epoch/presentation generation.
The application still accepts results through the same FIFO gate; revision remains dispatch context,
not a new acceptance serial or a semantic content-hash claim.

Static uploads and picker data share that descriptor. Partial updates preserve the identities of
untouched zones; removed empty zones drop metadata with resources. Exact Object comparison can
retain older static artifacts while updating complete inspection. `WorldRuntime::region_installed`
consumes one accepted owned payload for inspection, dependencies, animation membership and status.
Height/Object proof stamps come from the captured identity rather than reconstructing it at install.
Collision additionally refuses retained inspection from another source/presentation, including an
old source at the same zero producer epoch after replacement.

The inspector shows those last-accepted identities in its existing optional trace block. Renderer
upload/residency operations moved to `render_runtime/regions.rs`; no `EditorApp` field/method or
architecture allowance was added. GPU ownership and immutable draw inputs remain unchanged.
Standalone `RenderSnapshot`/`SemanticGeneration` describe another existing mesh/scene-hash shape;
they cannot stand in for this packed FIFO producer flow without losing identity meaning. No second
generation, new crate, shader/pass, semantic mutation, persistence format or ordering change was added.

Three actual-GPU regressions cover sharing/retention, empty zones, Object proof, stale/rejected/
terminal/rebuilt/replaced source paths and negative coordinates. A CPU query test covers retained
cross-source/presentation clipping. The real pinned-cache builder verifies exact source/extraction
metadata, and the Reference pixel golden remains unchanged. Aggregate animation, actor/preview
correlation and portable reproduction hashes remain open, as does independent external visual proof.

## Checkpoint 7: dataset exporter composition ownership

The world example failed compilation at its `osrs_lab::LearningSplit` call, while the architecture
matrix correctly forbids that upward world-to-Lab dependency. The exporter composes cache/world
reads, census semantics and Lab split policy; `osrs-editor` already owns that composition boundary
and declares every required dependency.

`building_dataset.rs` and its three modules moved byte-identically to `osrs-editor/examples`.
The old source locations are removed, with no adapter or duplicate policy. All four SHA-256 values
match before/after relocation; both package manifests and `Cargo.lock` remain unchanged. The format,
CLI arguments, exporter ID, canonical geographic split, source/weak-label evidence and read-only map
behavior are preserved. Current reproduction instructions now select `-p osrs-editor`.

This repairs the source blocker that kept the earlier world all-target verification open. Test and
command evidence below define the actual certification scope; a source move alone is not a claim
of a successful full corpus export or an independent parity oracle. The local OpenRune snapshot has
no installed `gamevals.dat` or LIVE cache, so a real export needs a complete external installation.

## Checkpoint 8: aggregate animated-loc artifact identity

The existing single-flight animation worker remains the ordering boundary. `WorldRuntime` captures
immutable dispatch context alongside that sole request: accepted nonempty producer descriptors,
source/presentation, requested cycle and advancement, center/radius, smoothing and frozen state.
Membership descriptors share their existing region identities and an immutable map; they retain no
document, scene, model or GPU resources. Changes copy descriptor membership only while an older
artifact still retains it; ordinary animation requests share the map.

The accepted owned animation packet carries geometry and this descriptor together into the existing
renderer. Nonempty upload retains the same identity; no-op completion drains pending context without
relabeling the installed picture. Empty output and existing shared-batch cleanup retire metadata with
resources. The optional existing inspector trace exposes bounded dispatch and inspected-region input
details. This is observational source/build identity, not a portable semantic hash.

A new regression reproduced pre-smoothing-change publication. Smoothing and freeze transitions now
revoke in-flight publication, including toggle roundtrips, while FIFO drains before replacement.
The production frame reconciles playback before polling. Source replacement also removes old worker
members before new-source dispatch. Ordinary clock/camera lag and last accepted producer data during
semantic rebuild retain their existing stale-read policy. No sequence math, cadence, worker message,
counter, persistence, history, dependency or architecture allowance changes were introduced.

Worker and actual-GPU regressions cover nonempty output, exact descriptors, no-op/empty distinction,
retention/lifetime, source reset and cleanup. Actor/preview identity, portable reproduction/recovery,
independent visual parity and the remaining target architecture are still open. Performance remains
paused. Verification results below and the handoff define the tested source boundary.

## Checkpoint 9: accepted actor extraction identity

NPC and player results extend their existing Rust payloads with exact decoder target provenance,
actual Reference/HD/fallback path and the worker's current presentation generation. The existing
path vocabulary and generation are reused. Worker actor extraction is grouped in private
`streaming/actors.rs`; request variants, FIFO ordering and actor pose algorithms remain unchanged.

New actual-GPU cases reproduced stale actor publication after presentation changed while old picker
data was retained. Requested, returned and current presentation must now agree, alongside the existing
token/stamp and current-frame acceptance checks. Presentation joins each existing request key, allowing
same-tick/frame replacement without a new serial. Ordinary simulation/pose lag remains valid.

Accepted GPU layers retain immutable issued request descriptors plus actual worker context. NPCs keep
the original stamp/filter inputs/selected count and ground producer descriptors, without retaining
full actor snapshots, models or scenes. Player input is the existing small `PlayerFrame` plus its key
and ground descriptor. The duplicate actor height readers converge on the existing inspection owner's
accepted-ground query, preserving coordinate/rounding and last-accepted-read policy. Empty output,
hide and source cleanup retire metadata with the image. Diagnostics observe a borrowed descriptor
view and do not participate in acceptance.

Expanded lifecycle cases exercise descriptor identity during lag, presentation recovery, nonempty
ground membership and weak-handle release. Pinned-cache extraction verifies actual decoder context
and explicit Reference fallback. This does not provide a self-contained reproduction bundle or
independent actor pose/visual oracle. Preview identity and the broader architecture remain open;
performance stays paused. Verification evidence follows below and in the handoff.

## Checkpoint 10: building preview admission and retained request inputs

Three deterministic regressions reproduced stale building publication after an invalid kit/plane,
stale synchronous candidates after invalid inputs, and a disconnected worker remaining pending.
`BuildState` now validates desired inputs before observing results, clears obsolete derived state,
distinguishes an empty channel from disconnection, and reports terminal failure without retry loops.

The existing tuple request key is retained as immutable `PreviewInput`, shared by job/batch/candidates.
Source/revision/form/site/plane matching protects apply, save, candidate selection and viewport reads,
including edits after the panel's refresh in the same frame. An obsolete batch cannot retire a
current job; inconsistent candidates cannot publish. The separate prior-preview read supports only
read-only post-commit script verification. Both native candidate batches and synchronous/browser
planning reuse the existing semantic generator and validation; the main BuildState file shrinks.

Building callers use the existing semantic session revision, including hydration/eviction, because
authored map revision does not identify streaming changes to generator inputs. Tests exercise real
session hydration without history, then regenerate from the changed terrain. This conservatively
invalidates on unrelated semantic transitions; narrower dependency evidence remains future work.

The preview audit records existing spline fit/source keys, picker-version appearance and fresh
commit-time fitting; object ghosts are per-frame projections. It does not add a parallel preview
scheduler or duplicate planner. Complete building catalog/footprint dependency identity, spline queue
bounds, placement-review inputs and portable preview reproduction remain open. Performance stays
paused, and the broader target architecture is incomplete. See the projection contracts and handoff
for the exact verified boundary.

## Checkpoint 11: building definition and footprint inputs

Building requests now retain optional catalog dimensions and portable kit furniture declarations,
keyed by object ID and loc kind, alongside their existing source/session/form/site identity. Source
definitions win over kit hints; label-table placeholders no longer mask actual dimensions during
validation. Catalog arrival or dimension changes revoke all apply/save/selection/drawing admissions
and supersede in-flight batches at the same semantic revision. Facts retain no document/catalog/model.

The existing `osrs-edit` footprint and overlap queries now have fallible entry points. Their loc-kind,
rotation, zero-size and eight-tile anchor-halo behavior remains shared with the infallible callers.
Unknown sizes required by clearing or candidate validation reject planning; distant unknown scenery
does not. Kit dependency enumeration covers all potential asset fields, deduplicates ID/kind and
rejects conflicting normalized furniture declarations. The world catalog keeps decoded no-model
definition sizes without adding those definitions to placement search. No new counter, worker,
planner, authored mutation route or persistence schema is introduced.

This closes the local building dimension-dependency gap identified in checkpoint 10. The snapshot is
conservative over document scenery and potential kit assets, and the overlap halo remains its existing
contract. Portable reproduction/content identity, spline queue bounds, placement-review inputs,
independent visual fidelity and the broader target remain open. Performance work remains paused.

## Checkpoint 12: bounded spline preview ownership

The existing spline `PlanWorker` now owns one physical thread, one replaceable pending input and
one completed result per interaction runtime. Owned slots replace the unbounded request/result
channels. A mutex protects only transfers; pure fitting happens outside it. The 24 ms debounce,
canonical fit/key bodies and semantic commit-time refit remain unchanged. Obsolete queued inputs
and completed output retire at replacement; stale active results cannot publish.

Clear, cancellation, no-input and source replacement clear retained slots while keeping the draining
thread. Resume/new-source requests cannot spawn overlapping fits. Exact key roundtrips can reuse
the active fit and retire redundant queued input. Owner drop clears/closes slots and wakes an idle
thread; an active fit finishes and exits without UI-thread joining. A stopped/panicked worker can
retry only after its thread handle reports terminal. Browser planning remains synchronous.

Seven deterministic tests pause actual fits to prove burst replacement, inactive-consumer result
retention, key roundtrips, public cancellation/resume, repeated source changes, shutdown retirement
and panic recovery. Prior spline tests, fit/key/make bodies and semantic commit code are unchanged.
This closes the spline retained-work gap from checkpoint 11, not a global worker-memory or latency
budget. Building candidate batches still need a physical concurrency audit; placement-review input
dependencies, portable reproduction/recovery and external fidelity remain open. Performance stays
paused; no allocation-throughput or frame-time claim is implied.

## Checkpoint 13: bounded building candidate ownership

`BuildState` now retains its native candidate worker across logical jobs. One replaceable queued
snapshot sits behind the existing job receiver/cancellation flag; production responses have capacity
one. Reroll/source/revision/footprint replacement cancels old work and releases queued/unread output
immediately. Only one background candidate runs per owner, with cancellation before and after each
candidate, followed by the newest request. Eight-seed wrapping, failure order, ranking and pure
planning remain unchanged. No new freshness counter or shared worker framework is introduced.

The logical ticket uses a weak queue link and its existing cancellation flag to remove only its own
pending request; an obsolete ticket cannot remove a newer equal-input request. Owner drop closes
slots and lets the active candidate drain without joining the UI thread. A thread-exit guard closes
queued senders on panic, preserving terminal failure reporting and explicit reroll/input-change
retry. A new physical worker requires a terminal thread handle. Synchronous scripts still generate
one candidate on the UI thread, potentially overlapping one draining native candidate; their result
cannot be overwritten by the retired native batch.

Seven tests pause actual candidate planning to prove a 64-input replacement burst on one thread,
queued/completed metadata retirement, invalid-input cancellation, obsolete ticket scope, shutdown,
panic/retry, wrapping-seed failure/ranking/patch equivalence and synchronous supersession. Existing
admission and prefab tests remain unchanged. This closes the native batch concurrency gap from
checkpoint 12, not global worker-pool/byte/latency budgets. Placement-review dependencies, portable
reproduction/recovery, editor-wide headless/MCP and independent fidelity remain open; performance
measurement/optimization stays paused.

## Checkpoint 14: reviewed placement and capture inputs

Placement review retains semantic session revision, source, actual operation/asset, initial armed
state, plane, frozen clicked origin and queried raw dimension facts. Reads and approval reject changes,
including hydration/eviction or catalog arrival at unchanged authored revision. Script placement may
begin unarmed; live mouse movement does not alter the frozen operation. Product planning uses fallible
canonical prefab entry points: source sizes win over checked captured metadata, and unknown required
scenery sizes reject planning. Added scenery is checked even without rotation or existing neighbors.
The same placement body backs explicit-size legacy APIs. Fitted conflict validation conservatively
queries every existing document object; a distant unresolved size can reject it. Foundation fitting
is byte-identical; no semantic gateway, history, fitting algorithm or persistence format change.

Capture dispatch/admission, projection, refinement and save now use semantic revision/source checks.
Frozen names, suggestions and definition tables remain review evidence. Real-session tests prove
hydration/eviction invalidate both reviews without consuming redo or changing authored state; placement
also proves current gateway commit/exact undo, gesture/asset/catalog rejection and unarmed script input.
Capture analysis and membership rebuild still spawn per request, so retained-work/concurrency bounds
remain the next boundary. Portable reproduction/recovery, headless/MCP, external fidelity and paused
performance remain open.

## Checkpoint 15: bounded capture work

`CaptureReview` retains one native worker across cancel/reselection. Initial census/cache analysis
and asynchronous membership rebuilds use one active operation, one replaceable queued immutable
closure and capacity-one replies. Replacement drops obsolete queued snapshots and unread output;
active work drains before the newest request runs. Logical jobs retain existing semantic revision/
source and a weak queue link; their cancellation scope cannot remove a newer equal-key request.
Pending input changes revoke work before completion. Selection errors and disconnected workers
retire refining/open state rather than leave a permanent spinner. Cancel precedes modal rebuild/save.

Shutdown closes slots without a UI-thread join. A thread-exit guard closes queued responses on panic;
explicit reselection replaces only a terminal thread, with no automatic per-frame retry. Seven
controlled tests prove burst/reselection bounds, real membership result equivalence, queued/completed
lifetime, old-ticket scope, shutdown/panic/retry and stale-input/payload rejection. Analysis statements
match the baseline after indentation normalization and membership remains byte-identical. No census,
planner/history/gateway/persistence or debt-allowance change. These are per-owner work bounds, not a
global pool or worst-case time/memory budget. Portable reproduction/recovery, headless/MCP, external
fidelity and paused performance remain separate obligations.

## Checkpoint 16: canonical map and transaction content identity

[ADR-0016](../adr/ADR-0016-canonical-semantic-content-identity.md) adopts explicit v1 digest methods
on existing `MapDocument` and `MapPatch`, returning the existing core `Digest256`. The
[byte contract](../specs/semantic-digests.md) freezes separate SHA-256 domains, fixed little-endian
fields/u64 counts, optional tags, canonical map enumeration and exact ordered patch operations.
Signed world coordinates/heights and all u32 IDs remain exact. Unknown height differs from zero;
optional Some(0) differs from None. Source/profile/build, availability, appearance/scenario,
intent/seed/clock and definition/rendering inputs remain separate envelope obligations. No new
semantic model/history, admission counter or hashing in frame/commit/undo paths.

Four tests pin independently calculated Python struct/hashlib vectors, insertion-order/default/
object normalization, all map value/coordinate channels, patch ordering/before-state/polarity/no-ops
and exact inverse hash restoration. The existing workspace sha2 version is reused; the lockfile
adds only the edit dependency edge. This supplies a reproduction primitive, not a bundle serializer,
headless verifier, workspace hash or recovery journal. Those remain open alongside external fidelity,
editor-wide headless/MCP and paused performance.

## Checkpoint 17: lossless map and patch transport

[ADR-0017](../adr/ADR-0017-semantic-map-transport.md) adopts explicit v1 JSON encode/decode methods
on existing MapDocument/MapPatch. [The closed transport schema](../specs/semantic-transport.md)
uses private records, canonical constructors and the existing v1 digest protocols. It preserves full
signed world coordinates/heights, u32 IDs, nullable channels and source planes. Snapshot duplicates
reject even if equal; sparse default terrain normalizes normally and missing heights stay absent.
Patch order/no-ops/polarity/before-state are exact; impact is derived through MapPatch::new.

Decoders reject malformed ranges/domains, unknown fields/operations, duplicate JSON keys, absent
nullable channels, unsupported versions/algorithms and invalid/mismatched hashes. Value errors retain
typed causes with collection/operation indices. Seven tests decode independent hand-authored vectors,
exercise all error classes and preserve normal history commit/undo/redo behavior. Inapplicable patches
remain subject to ordinary validation even with correct transport integrity. There is no mutation,
source/history bypass, public alternate model, agent raw-write action or dependency change. Project,
export and recovery formats remain separate. A real planner-input reproduction envelope/headless
verifier is the next boundary; these codecs alone do not establish portable editor replay.

## Checkpoint 18: frozen building reproduction

[ADR-0018](../adr/ADR-0018-building-reproduction.md) adopts the first concrete product reproduction
contract on the existing `PrefabRuntime::BuildState`. [The versioned envelope](../specs/building-reproduction.md)
retains complete signed map transport, actual base/candidate forms, compiled-kit source, canonical
footprint facts, source provenance and recorded editor build context. Explicit current-preview capture
feeds one retained native background export. The existing staging owner publishes synced complete
bytes create-only; it never replaces an existing report. Product UI and `prefab reproduce PATH` share
that operation. No new EditorApp behavior owner, generalized replay framework or raw agent write path.

`osrs-editor --reproduce-building FILE` starts before app/GPU/cache creation, validates bounded closed
input and invokes the same `build/plan.rs::plan`. Exact building/apply/result hashes must match;
commit/undo/redo pass through the existing EditRuntime in an isolated EditorSession. Source metadata
requires the world owner's existing current-source gate and opened project profile. Independent
project/world counters remain independent. Recorded source/compiler context is separate from Git
identity, an executable signature and external cache authentication; cross-build MATCH is reported
honestly. Source stamping has independent hash and actual Cargo new-file invalidation evidence.

A real-cache capture exposed unordered HashSet clearing output: the generated building hash matched,
while apply patch hashes varied across fresh processes. Canonical spatial queries now share the
existing document placement key. Query membership, footprint/halo/rotation rules, generation math and
final object sets are preserved. Dense reconstruction and cross-region query regressions cover the
ordering boundary. Original failing bundle/expectations remain evidence; a fresh 35 MB capture now
matches exact 505-operation patches and result maps in three fresh processes. Current-build checks
also preserve MATCH across the source-watch-only build change and reject the original old expectation
with a stable corrected digest. Automatic functional-run frame statistics do not resume performance.

This contract captures successful admitted candidates only. All five modes/seven stages, drawn
sketches, locks, repair areas and wrapping seed offsets retain actual inputs. Failed/unsatisfiable
batches need typed frozen outcome evidence on the same owner; their current display strings cannot
stand in for that contract. Workspace/scenario/appearance, projection settling/images, recovery,
other feature operations/MCP, global budgets and independent visual fidelity remain open.

## Checkpoint 19: exact rejected-attempt reproduction

[ADR-0019](../adr/ADR-0019-rejected-building-reproduction.md) adopts a separate
[failed-attempt envelope](../specs/building-failure-reproduction.md) on the existing building owner.
The planner assigns a typed stage at each rejection boundary, retaining the original message and
exact opaque lower-owner diagnostic cause. No display-string parsing or new lower-crate error model.
Exact evidence equality deliberately detects diagnostic drift across builds rather than treating
every failure as equivalent. Successful v1 bundles and planner algorithms retain their contracts.

Both synchronous and native batch paths retain failed seed/form/request identity. Eight native
attempts share one immutable input; no per-failure map/kit snapshot is retained. Existing source,
revision, form/site/plane and definition admission gates govern failure capture and UI reads.
Mismatched completion payloads cannot publish failed evidence. Clear/replacement retires records;
an explicit frozen export may finish independently using the existing bounded export/staging owner.

The same headless command dispatches original bytes to a closed schema, restores canonical frozen
inputs and invokes the same planner. MATCH requires the actual recorded rejection and exact unchanged
initial/result map digests. It reports `outcome: rejected` with zero operations and no patch digest;
there is no fabricated patch, EditRuntime transaction, undo/redo or projection for failed planning.
Unexpected success and stage/message/cause changes fail explicitly. Default product/script capture
uses selected success or the first current failure; per-seed report actions and `failure=SEED`
also capture failures in mixed batches while preserving the selected success.

This reproduces one actual attempt, not mathematical/global solver unsatisfiability or an aggregate
batch outcome. Pre-planner admission failures and worker cancellation/panic/disconnection are not
relabeled as planner rejection. Workspace/projection reproduction, recovery, other operation/MCP
contracts, independent visual fidelity and global resource budgets remain open; performance is paused.

## Checkpoint 20: composed authored-content snapshots

[ADR-0020](../adr/ADR-0020-authored-document-snapshot.md) adopts a versioned
[authored snapshot](../specs/authored-snapshot.md) on the existing EditorSession/EditorDocument.
Private records compose canonical map transport, validated optional scenario values and complete
ThemePack ZIP transport. Fresh checked lower-domain constructors preserve existing authority;
decoding builds an isolated new session without a live import or manufactured history transaction.
Complete assets and ordered metadata remain exact. Map, scenario, appearance and composite content
identities are distinct; absence and a present empty scenario cannot collide. Independent byte/hash
vectors and full-width/Unicode round trips pin the protocol without another workspace value model.

The native File-menu action and `project snapshot PATH` freeze one explicit document, then serialize
and create-only publish through ProjectRuntime and the existing export staging owner. The job survives
source/project replacement outside source-bound persistence reset. Busy capture cannot spawn another
snapshot export; completion/disconnection retires once. Snapshot capture never marks project edits
saved or changes save paths/history. The headless checker validates bounded files and canonical
round trips before initializing an app/GPU/cache. It executes no authored operation or projection.

The snapshot retains all current authoritative map/scenario/appearance values, not all source regions,
pending project changes, source/project identity, availability, history or simulation observations.
Decoded counters and lifecycle sets are fresh baselines. It is a building block for operation
envelopes and recovery, not a completed portable editor replay, project replacement or journal.
Existing building v1 success/failure formats remain map-scoped. Workspace-context/operation envelopes,
render/picker settling/images, recovery, MCP, independent fidelity and global budgets remain open;
performance measurement/optimization remains paused.

## Dependency-ordered continuation

1. **Installation correctness:** close the first checkpoint with failure, freshness, recovery and
   successful-path tests. Do not start projection optimization before this boundary is verified.
2. **Projection contracts:** inventory impact consumers and mirrors; record authority, identity,
   invalidation, rebuild, stale-read and failure policy for every installed artifact. Preserve FIFO
   until a deliberately adopted replacement protocol supports another completion order.
3. **Fidelity evidence:** extend existing bridge/collision, normal, roof, priority/alpha, depth-bias
   and texture fixtures; obtain independent visual evidence. Label semantic proof, internal visual
   regression and external visual parity separately.
4. **World-to-render identity:** reconcile existing snapshot/generation primitives with active
   installation; trace artifacts to source, spatial scope, profile and dependency generation.
   Drawing must not query mutable semantic authority.
5. **Measurement, then optimization — deferred:** performance work remains explicitly paused in
   [the existing plan](../plans/PERFORMANCE-UI-PLAYTEST-HD.md). After an explicit resumption, establish
   fixed-input distributions and memory classes before choosing allocation/scheduling/packing/upload
   changes. Paged allocation is a measured candidate, not a prerequisite.
6. **Reproduction and recovery:** extend building success/rejection reproduction to other features
   and workspace domains, separately from crash journals. Reproduce declared inputs
   and recover authored state without trusting disposable caches.
7. **Headless operations, then MCP:** reuse feature IDs, planners, queries and transaction policy;
   prove UI/automation capability equivalence before adding a protocol-only adapter.

These later boundaries require their own grounded implementation plans. This checkpoint does not
implement or certify them. Remain on `main`; branch/worktree creation and publication require user
authorization. Do not alter unrelated working-tree files or implicitly resume paused performance work.

## Verification evidence

The initial structural audit passed architecture, authority and feature-index checks plus 50 Python
guard tests (27 architecture, 12 editor-ownership, 11 authority). It did not run Rust, GPU, external
oracles or remote CI. Checkpoint verification and any unavailable/skipped acceptance evidence are
recorded in the handoff with the actual tested source boundary.

The final editor build stage passed native all-target/all-feature Clippy and 373 tests (0 failed,
4 pinned-cache tests ignored), with a stable source hash before/after:
`target/verify/gates-20261019-093734/report.json`. The synthetic headless GPU regression explicitly
exercised full/scoped static and animated removal and preservation of pending project work. Strict
WASM editor Clippy passed after narrow target-gating/type fixes to pre-existing building-tool
compile failures and removal of unused history-cleanup helpers; no lint/debt allowances were widened.
Production/test sources were unchanged after verification; handoff/audit evidence was updated afterward.
No full workspace, real-mouse, remote CI or independent visual-oracle certification is claimed.


Checkpoint 2: the six added contract tests passed after first reproducing the two lifecycle defects.
The final focused build gate passed strict native Clippy and 379 tests (0 failed, 4 pinned-cache
prerequisites ignored), with stable source identity in
`target/verify/gates-20261019-110314/report.json`. Strict WASM editor Clippy also passed. GPU logs
confirm presentation rebuild removes both static and aggregate animated output. See
[the projection contracts](PROJECTION-CONTRACTS.md) for remaining gaps; the original checkpoint's
373-test result above retains its historical scope.

Actor-publication checkpoint: strict native editor Clippy and **383 tests passed, 0 failed, 4 ignored**,
stable source before/after in `target/verify/gates-20261019-111302/report.json`. Strict WASM editor
Clippy, formatting and architecture/authority/feature-index guards also passed. The first targeted
run exercised both actual actor GPU lifecycle paths. Only verification documentation changed
later; see the handoff for precise scope and timings. This does not close external parity or the
remaining terminal-failure/artifact-identity boundaries.

Terminal cleanup checkpoint: strict native editor Clippy and **385 tests passed, 0 failed, 4 ignored**;
stable source before/after in `target/verify/gates-20261019-173825/report.json`. The two focused
terminal tests explicitly exercised actual GPU paths. Strict WASM editor Clippy, architecture/
authority/feature-index guards, formatting, changed Markdown links and whitespace checks passed.
Only verification documentation changed afterward. See the handoff for precise scope and timings;
no independent visual, remote CI or performance certification is implied.

Normal-gate checkpoint: final focused native reference build passed strict all-target/all-feature
Clippy and **88 tests, 0 failed, 0 ignored**, stable source in
`target/verify/gates-20261019-175615/report.json` (49.5 s). The preceding quick checks passed,
including six new offline oracle-safety tests; its subsequent build caught test-only `expect`
assertions, corrected without allowances. The final Rust tests cover invalid-model context as well
as exact comparison/downgrade cases. Pinned Java method replay matched all ten normal cases with
JDK 24.0.2 (`--release 17`). Formatting, changed Markdown links, verbatim plane-adapter comparison
and whitespace checks passed. Only verification documentation/wording changed afterward. No
full-workspace, remote CI, independent visual or performance certification is claimed.

Static-artifact checkpoint: final focused editor build passed strict all-target/all-feature native
Clippy and **389 tests, 0 failed, 4 pinned-cache tests ignored**, stable source in
`target/verify/gates-20261019-181145/report.json` (73.7 s). World library/integration-test Clippy and
strict WASM editor Clippy passed separately (10.5 s / 53.2 s). All three focused GPU identity cases
explicitly exercised actual uploads/removal; the initially missing Object-proof fixture setup was
corrected without changing normal full-replacement behavior. Real pinned-cache metadata and
Reference pixel-golden tests passed; no accepted golden was rewritten. Architecture/authority/
feature-index guards, formatting, changed Markdown links and whitespace checks passed. Only
contract/verification documentation changed after the final Rust gate. At that checkpoint the world dataset
example's upward Lab reference prevented all-target world certification; that focused gate
excluded it. Checkpoint 7 repairs its composition ownership. No full-workspace, remote CI, independent visual or performance claim.

Dataset composition/integration checkpoint: strict native workspace/all-target/all-feature Clippy,
strict WASM Clippy and **1,384 tests passed, 0 failed, 23 ignored** in the full build stage
(`target/verify/gates-20261019-182336/report.json`, 749.4 s). Final full quick checks passed
(`target/verify/gates-20261019-183616/report.json`, 4.3 s). Both reports record the same stable
source hash, `e6927444bae34a44a00791020405364f14c9bdd5b1820108f9531488d6a0b38f`.
The first integration attempt exposed 31 existing test-only `unwrap` violations; explicit panic
closures retain assertions and expected values without lint allowances, and the production WFC
repair prefix is unchanged. Relocation hashes, unchanged manifests/lockfile and the new CLI's
expected missing-root error are recorded under `target/verify/dataset-ownership-nf5jk_pk/`.
Existing prerequisite skips/ignores remain. Only documentation changed after the final gates.
This is local workspace integration evidence; full corpus export, independent external visual
parity, remote CI and performance remain outside its scope.

Aggregate animation checkpoint: all 12 targeted animation checks passed, with explicit execution
logs for all three new actual-GPU paths. Final focused editor gate passed strict native Clippy,
**398 tests, 0 failed, 4 pinned-cache tests ignored**, and Python/architecture/authority/feature-index/
source-pin/format checks in `target/verify/gates-20261019-225952/report.json` (34.4 s). Stable source:
`13d4088273cb1f30b1e0b42b82fef914c79a3c2f53ec1d9708a799869389f0e8`.
Strict WASM editor Clippy passed (21.4 s); links and whitespace passed. The first frame test's
synthetic clock was ahead of the app clock; the corrected zero-baseline fixture leaves production
timing semantics unchanged. Existing prerequisite skips/ignores remain. Only documentation changed
after the final gates. These lifecycle/input-identity tests do not certify sequence-frame math,
portable reproduction, external visual parity, remote CI or performance.

Actor identity checkpoint: the expanded actual-GPU cases passed with explicit execution logs.
The nonempty NPC fixture uses its decoder's real cache fingerprint after the existing mixed-cache
asset guard rejected its earlier synthetic binding. No guard or assertion was weakened. Final
editor/world gate passed strict native all-target/all-feature Clippy and **456 tests, 0 failed,
22 ignored**, plus architecture/authority/feature-index/Python/source-pin/format checks in
`target/verify/gates-20261019-231431/report.json` (149.4 s). Stable source before/after:
`83971fd882452f3fd15f2cc48df626ecdf7ea009431b8d72b3b292194cc91869`.
Strict WASM editor/world Clippy passed (21.8 s). The pinned-cache actor-context test passed explicitly
(0.24 s), including nonempty real default-player geometry and exact equality of Reference versus
requested-HD fallback output. Ordinary prerequisite ignores/skips remain. Links/whitespace passed;
only documentation changed afterward. These are source/identity/lifecycle and internal fallback
proofs; independent actor pose/HD visual parity, remote CI, reproduction and performance remain open.

Building preview checkpoint: eight added admission tests passed, including controlled completion order,
real-session hydration and the real candidate worker. After using the existing validated 3300,3300
fixture site, the three intended regressions failed before the fix. Final editor gate passed strict
native Clippy and **406 tests, 0 failed, 4 pinned-cache tests ignored**, plus architecture/authority/
feature-index/Python/source-pin/format checks in `target/verify/gates-20261019-233653/report.json`
(70.9 s). Stable source:
`77ffd589e414db90d8af415437b4288df59e36ff90853561cbcb7a924a9c885b`.
Strict WASM editor Clippy passed (11.9 s) after target-specific tail control flow was corrected;
no lint allowance was added. The generator planning/validation body remains byte-identical.
Existing skips/ignores remain. Only documentation changed afterward. See the preview audit for
definition-footprint and queue/review gaps; this does not certify full preview reproduction,
independent visual parity, remote CI or performance.

Building footprint checkpoint: the focused `osrs-edit`/`osrs-world`/`osrs-editor` gate passed strict
native Clippy, guards/Python/source-pin/format checks and **765 tests, 0 failed, 22 ignored** in
`target/verify/gates-20261019-235448/report.json` (276.3 s), with stable source before/after. Strict
WASM Clippy for the three crates passed (52.4 s). The explicit pinned-cache catalog check passed
(0.36 s): unlisted decoded object 66 retains its actual 2x2 dimensions, matching the canonical loader
while remaining absent from search. Tests cover catalog arrival/size changes at the same revision,
source dimensions over label placeholders, unknown required footprints, rotated outside anchors,
exact undo and superseded metadata release. Existing prerequisite skips/ignores remain; the separate
cache run actually exercised the normally runtime-skipped catalog check. Only documentation changed
afterward. The handoff records commands/hash and fixture correction. Spline queue/review contracts,
portable reproduction, independent visual evidence and paused performance work remain separate.

Spline worker checkpoint: all 20 spline tests passed, including seven new controlled worker tests.
The focused editor gate passed strict native Clippy, architecture/authority/feature/Python/source-pin/
format checks and **417 tests, 0 failed, 4 ignored** in
`target/verify/gates-20261020-021754/report.json` (69.4 s), with stable source before/after. Strict
WASM editor Clippy passed (11.5 s). Existing fit/key/make bodies, prior spline tests and semantic
commit code are byte-identical to `HEAD`; no lint/debt allowance was added. Only documentation
changed afterward. The handoff records the source hash and controlled shutdown-test correction.
Building batch concurrency remains distinct: drop currently cancels between candidates but can
leave an active candidate running while a replacement batch starts. Global worker budgets,
placement-review dependencies, portable reproduction/recovery and external fidelity remain open;
performance measurement/optimization stays paused.

Building worker checkpoint: all 35 prefab-runtime tests passed, including seven new controlled
worker tests. The focused editor gate passed strict native Clippy and guard/Python/source-pin/
format checks with **424 tests, 0 failed, 4 ignored** in
`target/verify/gates-20261020-023257/report.json` (33.6 s), with stable source before/after. Strict
WASM editor Clippy passed (11.4 s). Existing admission tests and canonical planning/history paths
remain unchanged; no fixture/lint/debt allowance change. Only documentation changed afterward.
The handoff records source hash, bounded native ownership, the synchronous overlap limit and next
review/capture revision-dependency audit. Global budgets, portable reproduction/recovery,
editor-wide headless/MCP and independent fidelity remain open; performance stays paused.

Review/capture checkpoint: focused native `osrs-edit`/`osrs-editor` gate passed strict Clippy,
guards/Python/source pins/formatting and **727 tests, 0 failed, 4 ignored** in
`target/verify/gates-20261020-025408/report.json` (146.6 s). Strict WASM Clippy for both crates passed
(30.3 s) with the same source hash. All 23 focused semantic prefab cases passed; existing expectations
were preserved. Foundation byte proof is saved with the gate. The handoff records exact source identity
and conservative dependency limits. Only documentation changed after verification; existing
prerequisite skips remain. No full-workspace, remote CI, independent visual, portable reproduction
or performance claim.

Capture-worker checkpoint: focused editor gate passed strict native Clippy and guards/Python/
source pins/formatting with **436 tests, 0 failed, 4 ignored** in
`target/verify/gates-20261020-030236/report.json` (72.1 s). Strict WASM editor Clippy passed (11.9 s)
with the same source hash. The capture filter passed 24 tests, including all 11 prefab capture cases;
seven new worker tests use controlled handshakes. The gate directory retains unchanged analysis/
membership proof. Documentation and the prefab planner registry/index correction followed the Rust
gates; cheap checks cover those changes. Existing prerequisite skips remain. No full-workspace,
remote CI, independent visual, portable reproduction or performance claim.

Canonical digest checkpoint: focused osrs-edit gate passed strict native Clippy and guards/Python/
source pins/formatting with **302 tests, 0 failed, 0 ignored** in
`target/verify/gates-20261020-030938/report.json` (64.5 s). Strict WASM Clippy for osrs-edit and
downstream osrs-editor passed (40.1 s) at the same source hash; `wasm-source.json` records it. Four
independent-vector/channel/order/inverse tests passed. Only documentation changed afterward;
links/whitespace checked. Existing Python prerequisite skips remain; no full-workspace, remote CI,
portable editor replay/recovery, external visual or performance certification claim.

Lossless transport checkpoint: focused osrs-edit gate passed strict native Clippy and guards/Python/
source pins/formatting with **309 tests, 0 failed, 0 ignored** in
`target/verify/gates-20261020-032021/report.json` (76.1 s). Strict WASM Clippy for osrs-edit and
downstream osrs-editor passed (31.1 s) at the same source; `wasm-source.json` records the boundary.
Seven transport tests passed, including indexed errors and duplicate operation keys. Existing tests/
fixtures and lint/debt limits were preserved. Only documentation changed afterward; Markdown targets,
empty-snapshot example and whitespace checked. Existing Python prerequisite skips remain. No full
workspace, remote CI, complete replay/recovery, independent visual or performance claim.

Building reproduction checkpoint: 15 focused reproduction/source-identity tests and the canonical
spatial-order regression passed. Final affected osrs-edit/osrs-editor gate passed strict native
all-target/all-feature Clippy, guards/Python/source pins/formatting and **761 tests, 0 failed,
4 pinned-cache tests ignored** in `target/verify/gates-20261020-072444/report.json` (83.7 s).
Strict WASM Clippy for both crates passed (68.5 s), with identical native/WASM source before/after:
`d762710c82b401ca7a329678c5fbf22f66a1a05b93ceeb2bd0fe719e171dd04f`.
`wasm-source.json` records the boundary. Independent source-stamp and actual Cargo directory-watch
checks passed. The handoff records real pinned-cache capture, three-process MATCH, current-build
cross-context verification and preserved old-bundle mismatch evidence. Existing expected fixtures
and lint/debt baselines were preserved; sha2 reuses the pinned workspace dependency (no package
version changes). Only documentation changed after final gates; links/whitespace checked. Existing
prerequisite skips remain. This proves the declared successful-building semantic operation, not
full-workspace/remote CI, failed-batch replay, recovery, independent visuals or performance closure.

Rejected-attempt checkpoint: all 25 focused reproduction/source tests passed, including ten new
failure cases and a real mixed success/failure batch. Final focused editor gate passed strict native
all-target/all-feature Clippy, guards/Python/source pins/formatting and **461 tests, 0 failed,
4 pinned-cache tests ignored** in `target/verify/gates-20261020-075716/report.json` (90.5 s).
Strict WASM editor Clippy passed (13.7 s), at the same stable native/WASM source:
`4ec79275bac08250b0f0266bdee5bc07a9937c87d1e528613f8f93084d14b18d`.
The handoff records independent source-stamp proof and real pinned-cache capture: three fresh
processes reproduce the actual candidate-validation rejection with zero operations and unchanged map;
three more preserve the previous successful real-cache bundle's exact patch/result; changed expected
cause fails nonzero. Existing fixtures/dependencies/lint baselines were preserved. Only documentation
changed after final gates; links/whitespace checked. Existing prerequisite skips remain. No
full-workspace/remote CI, aggregate/global unsatisfiability, workspace/projection/recovery,
independent visual or performance closure is claimed.

Authored snapshot checkpoint: ten focused snapshot/export tests and the canonical checked-baseline
constructor test passed. Final affected osrs-edit/osrs-editor gate passed strict native all-target/
all-feature Clippy, guards/Python/source pins/formatting and **782 tests, 0 failed, 4 pinned-cache
tests ignored** in `target/verify/gates-20261020-090750/report.json` (157.3 s). Strict WASM Clippy for
both crates passed (30.6 s), at the same stable source before/after:
`0acf0243bef66f6967b08caca1a4dcb43cab0fbc6f3167ac7a30efeece95e36a`.
Independent source-stamp proof agrees. The handoff records real product capture with an authored NPC,
three-process live and asset-bearing fixture MATCH, independent scenario/theme/assets/composite hash
checks, rejected changed digest, and preserved legacy building-format replay. Existing fixture
expectations/dependencies/lint baselines were preserved. Only documentation changed afterward;
links/whitespace checked. Existing prerequisite skips remain. No full-workspace/remote CI,
source/clock/availability-aware operation replay, recovery, independent visual or performance closure.
