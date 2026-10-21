# Active projection lifecycle contracts

Status: current implementation inventory and regression contract, 2026-10-19.
Baseline: `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, plus the local candidate-rejection
checkpoint, consolidation, actor-publication, terminal-outcome and static-artifact identity checkpoints. See [the handoff](../../DEV_HANDOFF.md) for tested
source boundaries. This is editor lifecycle policy, not a new OSRS semantic specification.

[ARCHITECTURE.md](../../ARCHITECTURE.md), [ADR-0011](../adr/ADR-0011-editor-ownership-and-feature-contracts.md),
[ADR-0005](../adr/ADR-0005-zone-compiled-hybrid-rendering.md), and the owning semantic specs remain
authoritative. [The technical audit](TECHNICAL-ARCHITECTURE-AUDIT.md) describes the larger rollout.
No new scheduler, impact enum or projection registry is introduced. The existing `RegionGeometry`
payload gains exact target provenance and extraction inputs; FIFO requests and completion order stay unchanged.

## Authority, identity and acceptance

- `MapDocument`/`EditorSession` owns edited terrain, heights and placements. Scenario and appearance
  have separate authored domains in the same chronological history. A worker mirror, cache, picker,
  render packet or GPU buffer can be dropped without discarding authored state.
- Semantic revision, source generation, request ID, projection epoch, presentation generation and
  playtest run/tick identity are distinct. A global semantic revision can advance through unrelated
  hydration while an unaffected region ticket stays valid. Diagnostic revisions never gate acceptance.
- Region requests/results are FIFO on one `RegionStreamer`; Region, Empty and Failed terminal events
  consume their original ticket. Cancellation retains old FIFO tickets; replacement of the source
  replaces the worker/receiver and clears its ledger. Do not parallelize this protocol without changing
  how every terminal completion identifies its request. Current region requests are capped at four;
  cancelled requests can still accumulate in the FIFO worker/ledger, so this is not a bounded-channel
  or bounded-total-backlog guarantee.
- Region acceptance requires current request identity, region projection epoch and presentation
  generation. A Region payload additionally checks its worker presentation generation and semantic
  adaptability before comparison, hydration or installation. Empty/Failed rely on the ledger stamp.
- Dirty region/zone sets describe required output **until accepted completion**, including in-flight
  work. Repeated zone edits union; full-region invalidation subsumes zones. A superseded completion
  cannot erase either its original required area or newer pending work.
- Previous accepted rendering/picker data may remain visible during rebuilds. Collision/navigation
  refuses absent, wrong-source/presentation, epoch-stale or in-flight evidence. Visual reuse does not certify semantic freshness.

Region/zone tuples in the active application pipeline describe world x/y areas across planes. The
standalone renderer `RenderZoneKey`/`SemanticGeneration`/`RenderSnapshot` primitives are not substitutes
for the active FIFO ledger and are not claimed to be wired into streamed installation.

## Accepted static and inspection identity

`BuildTracker::finish` now returns the existing issued ticket and stamp instead of discarding the
stamp at acceptance. Semantic adaptation still precedes construction of the private immutable
`RegionArtifactIdentity`. That descriptor binds the original source instance/request, dispatched
semantic revision, producer epoch and presentation generation to the payload's canonical
`TargetProvenance` (profile/digest/cache fingerprint/decoder schema), actual `TerrainPresentation`
and `RegionRenderPath`. Requested HD and actual Reference fallback remain distinguishable.

`RenderRuntime` retains one shared descriptor for each newly uploaded world zone. A scoped refresh
keeps untouched zones' original identities; an empty requested zone removes both its GPU resources
and metadata. An exact Object proof may keep every static zone while a newer complete picker
artifact is installed. A different picker/static identity therefore does not itself prove a bug.

`WorldRuntime::region_installed` consumes the accepted owned `RegionGeometry` and installs picker,
Height/Object proof stamps, animation membership and status from that same payload. Picker metadata
shares the descriptor with newly uploaded zones. Collision reads require its source/presentation and
producer epoch to match current requirements, with no dirty/in-flight work. Resetting a source to
producer epoch zero cannot certify retained clipping from the previous source instance.

The inspector's existing optional trace block shows the last accepted static-zone, picker and animated-loc
identities, with wrapping text and explicit missing-artifact labels. It does not call either artifact
current authored truth. Removal, terminal failure, rejection and source/presentation rebuilds remove
metadata with their corresponding projections. Draw submission still reads only renderer-owned data.

The standalone `RenderSnapshot` owns `RenderMesh` plus a `SceneGrid` hash/serial. That shape does not
represent the active packed `SceneGeometry`, FIFO source lifetime or mixed-generation scoped zones.
The active path extends its existing payload and stamp rather than manufacturing another generation
or pretending its dispatched revision is an exact semantic content hash. Aggregate animation extends
the existing single-flight owner with captured producer descriptors and dispatch context. Preview
identity plus portable reproduction coverage remain separate follow-up work.

## Accepted actor identity

NPC and player worker results now carry `ActorGeometryContext`: exact decoder target provenance,
actual Reference/HD/Reference-fallback path and the worker's existing presentation generation.
Both existing token/stamp and frame-reconciliation gates remain; publication additionally matches
requested, actual and current presentation generations. Presentation is part of the existing request
key, so a same-tick/frame presentation replacement still requests new output. No new freshness counter
or worker completion order is introduced.

Each accepted GPU actor layer retains immutable issued inputs, distinct from the next pending request.
NPC descriptors retain the original stamp, camera/range/limit/vars, selected count and accepted ground
producer descriptors. Full actor snapshots, models, scenes and semantic documents are not retained.
Player descriptors retain its small exact `PlayerFrame`, original request key and ground producer.
Both use the same accepted inspection query for rounded ground height and producer identity; sampling
still permits last accepted picker data during a semantic rebuild. Ordinary simulation/pose lag remains
allowed, and metadata describes the uploaded image rather than relabeling it with a later clock.

Empty actor output, hide and source cleanup retire metadata with GPU data through existing transient
layer operations. Stale arrivals cannot install either geometry or identity. The existing optional
inspector trace observes these descriptors through a borrowed view; it never decides validity.
These are extraction/input identities, not a portable reproduction bundle or actor pose oracle.

## Preview admission audit

Building previews retain the existing source/form/site/plane/revision request key as one immutable
`PreviewInput`, shared by the issued job, its batch and every accepted candidate. Application callers
use the semantic session revision, which includes hydration/eviction, rather than the authored-only
map revision. This is a conservative whole-session boundary: unrelated semantic transitions can
invalidate a build until narrower dependency evidence is established. It is not a portable content hash.

The same input retains definition facts keyed by object ID and loc kind: the optional catalog size
and optional portable kit furniture declaration. The snapshot covers document scenery and every
potential kit scenery asset; revalidation checks these distinct retained IDs, without cloning the map
each frame. Known source sizes override kit hints. Catalog arrival/disappearance or dimension changes
invalidate requests and all admission reads even at the same source/session revision. Label tables
provide names only; they never provide guessed dimensions. The catalog retains dimensions for decoded
definitions without direct models while keeping those definitions out of placement search.

`osrs-edit` owns fallible footprint/overlap queries, preserving existing one-tile structural kinds,
zero-size normalization, orientation and the eight-tile anchor halo. An unresolved scenery size needed
by the conservative overlap search or candidate validation rejects the plan; unrelated distant unknown
objects do not reject it. Both native and synchronous/browser candidates read the same frozen facts.
Conflicting kit furniture declarations reject input capture. This remains conservative: any captured
definition fact or semantic transition can invalidate a preview, including unused kit assets.

`BuildState` validates desired inputs before polling. Invalid kit/plane/site states cancel pending
work and remove previous candidates in synchronous and native-worker paths. A stale batch cannot
retire the current job; a matching batch with inconsistent candidate inputs fails closed. Worker
disconnection retires the slot and reports a failure without a per-frame retry loop. Reroll or another
input change retries. Current-preview reads guard apply/save/candidate selection/viewport drawing,
including form edits later in the same UI frame. Read-only script verification may inspect the prior
plan after commitment; that operation cannot authorize applying or saving it.

The spline fit is a separate mathematical preview: its existing key covers points/cursor, overlay,
plane, options and frozen local extent/source generation. Its Reference-colour appearance also keys
accepted picker `infos_version`. Stale fitted tiles may remain as a faded spatial reference while
replacement work runs. Commit fits the placed points again against current semantic terrain and
availability; it does not treat the cursor preview as a patch authority. Object-placement ghosts are
drawn each frame from the armed catalog entry/orientation, current plane/camera and accepted ground;
they do not own a retained asynchronous GPU artifact. Terrain stroke cancellation/restoration and
theme preview gates retain their existing separate contracts.

The native spline worker retains at most one active fit, one replaceable pending input and one
completed result per interaction runtime. Input replacement immediately frees obsolete queued and
completed data. Clear, cancellation, no-input and source replacement clear the slots and revoke the
active result; they keep the one physical thread so resume/new-source work cannot overlap an old fit.
An active pure fit drains before newer input is consumed; fitting does not hold the mailbox mutex.
The existing 24 ms debounce and fit key remain. An active fit may satisfy an exact key roundtrip,
retiring a duplicate pending input. Owner drop closes slots immediately and wakes an idle thread;
an active thread exits after its current fit without blocking UI shutdown. Panic/terminal detection
uses the actual thread handle before retry, rather than replacing a merely slow worker. Browser
planning stays synchronous. This is a retained-work bound, not a worst-case fit-time/byte budget.

Native building batches retain one physical worker per `BuildState`, one replaceable queued
document/kit/input snapshot and a capacity-one response for the current logical `Job`. The existing
job cancellation flag scopes queue removal: dropping an obsolete ticket cannot remove a newer
request, even with equal preview inputs. Reroll/source/revision/footprint replacement cancels the
old batch, drops queued snapshots and unread results immediately, and lets only the active candidate
finish before consuming the newest request. Cancellation is checked before and after each candidate;
the eight-seed limit, wrapping seed sequence, failure order and candidate ranking are preserved.
Invalid input and synchronous script planning retire native tickets through the same path.

Job completion leaves the physical worker idle for reuse. Owner shutdown closes slots and cancels
the active candidate without a UI-thread join; a thread-exit guard also closes queued responses on
panic so no live ticket waits forever. Terminal failures preserve the existing explicit reroll/input-
change retry policy; the thread handle must be finished before replacement. Explicit synchronous
script generation remains a separate single UI-thread call and may overlap one draining native
candidate. These per-owner bounds do not establish a global pool, worst-case memory or time budget.

Placement review freezes the actual operation, prefab asset, initial global armed gesture, clicked
origin and plane, semantic revision/source, and every queried raw definition-size fact. Modal reads
and approval recheck these inputs. Script operations may start unarmed; hover movement does not change
the clicked target. Source sizes override checked captured sizes; unknown required scenery dimensions
reject canonical fallible planning. Added scenery is checked even with no rotation or neighbors.
Fitted conflict validation is conservative over every existing document object, so distant unresolved
sizes may reject placement. Foundation fit math is unchanged; accepted patches use the normal gateway.

Capture dispatch/admission, selection projection, refinement and save use semantic revision/source.
Hydration and eviction revoke reviews even when authored revision stays unchanged. Names, suggestions
and the analysis definition table remain frozen review evidence. CaptureReview retains one native
worker across cancel/reselection: one active census/cache or membership operation, one replaceable
queued closure and capacity-one replies. Pending tickets retain semantic revision/source; stale inputs
retire before completion. Cancellation drops queued/unread data and revokes active publication, with
active work draining before replacement. Obsolete ticket drop cannot clear a newer equal-key request.
Errors/disconnection retire the logical selection; panic closes queued responses. Explicit reselection
can replace only a terminal thread; no automatic retry. Shutdown closes slots without joining active
work on the UI thread. Synchronous floor/piece corrections retain their existing membership calls;
no census or membership math changed. This is a per-owner work bound, not a time/byte/global-pool budget.

This audit does not certify portable preview reproduction or a general definition-content hash.
Broader worker-mirror replacement remains a separate rollout obligation. Generator algorithms,
semantic history and persistence formats remain unchanged; building clearing and validation fail on
unknown required footprints and use known dimensions.

## Semantic impact routes

The vocabulary is [`osrs-edit::ChangeImpact`](../../crates/osrs-edit/src/patch.rs). Existing producers
and consumers are reused; no new bits are needed for this checkpoint.

| Receipt/input | Current consumer and dependency expansion | Installed outputs affected |
|---|---|---|
| Terrain material/geometry/normals/lighting with exact tile footprint | `WorldRuntime::invalidate_semantic_edit`: radius-5 material halo, Euclidean 8x8 zones | Scoped static extraction/upload; full contextual picker, collision, animation and dependency result |
| Changed height vertices | Accepted complete Height inverse read-sets across resident producers, unioned with any terrain halo; otherwise conservative region policy | Same output families; full contextual scene construction remains required |
| Added/removed objects | Complete accepted Object producer output over resident producers in the conservative 3x3 region ring; compare full pre/post output to narrow static installation | Full candidate build; exact changed static zones only when comparison proves them; other projections refresh fully |
| Collision-affecting terrain flags | Full conservative 3x3 producer-region ring, even with terrain/Height data | Clipping must cover anchored loc footprints beyond a material halo |
| Missing precise footprint or incomplete/stale proofs | Existing `invalidated_regions` conservative expansion | Full affected producer builds; never infer a smaller area from UI selection or geometry bounds |
| Scenario | `NpcRuntime` observes scenario/map revisions and rebuilds/revalidates its own playtest runtime | NPC simulation and actor geometry; no terrain dirty-area fiction |
| Appearance | Appearance history may report `ChangeImpact::NONE`; frame-boundary theme/profile orchestration consumes authored appearance directly | Shader preparation, frame uniforms, profile-dependent rebuild; appearance does not dirty map regions |
| Source/profile/camera/preview changes | Explicit lifecycle paths, not semantic edits | Receiver replacement/presentation rebuild, visible residency, animation schedule, transient preview or viewport reuse |

Commit, paste, prefab/object placement, and semantic replay forward their finalized receipts into
world invalidation. Live terrain preview/cancel additionally mirrors reversible `TileEdit` values
through the legacy adapter; it is not another semantic commit path. An ordinary appearance preview
is not evidence that unsupported terrain/object material authoring is implemented.

## Installed artifact inventory

Paths below are relative to `crates/osrs-editor/src/app/` unless another crate is named.

| Artifact; disposable owner | Source and acceptance identity | Invalidation, rebuild and stale-read policy | Failure/removal policy |
|---|---|---|---|
| Worker tile/loc mirrors; `osrs-world::WorldDefinitions`, `TerrainEdits`, `LocEdits` | Selected cache or virtual source plus ordered editor projections; source lifetime and FIFO edit/build order | Replay from semantic/project state, or transient gesture/cancel values. Cache data alone cannot reconstruct authored overrides | Replace worker on source switch. These mirrors must never become planner/history authority |
| Contextual scene and `RegionGeometry`; `osrs-world::RegionStreamer` | Owned source/result with target provenance, actual extraction settings/path; ledger plus presentation/adaptation gates | 104x104 contextual scene per producer; zone scope limits static extraction, not scene construction | Stale completions discarded; semantically invalid candidate rejected before all installs |
| Static zone GPU buffers and region statistics; `RenderRuntime` / `WorldRuntime` | Shared accepted `RegionArtifactIdentity` per uploaded zone plus original or Object-proven scope | Full replacement or exact zone upload/removal. Last accepted picture can remain while rebuilding; stats follow installed scope | Current Empty/Failed or semantic rejection removes the whole region. Empty requested zones remove prior geometry. GPU resource ownership stays renderer-local |
| Picker, tile/loc info and actor ground samples; `WorldRuntime::InspectionProjection` | Accepted full contextual `RegionInfo`, even after scoped static refresh; shared accepted identity and monotonically changing info version | Entire producer info replaced after accepted build. Picking/ground sampling can read last accepted info during rebuild; never use it as authored truth | Install/removal invalidates old outlines and roof cache version. Current Empty/Failed and semantic rejection remove info entirely |
| Collision snapshots/navigation; same inspection owner, queried through `WorldNavigation` | `RegionInfo.collision` with matching source, presentation and installed/current epoch; no dirty/in-flight work | Exact [COLLISION-001](../specs/collision.md) cells/provenance. Missing/stale/building state yields Unavailable/Updating, so navigation parks | Current Empty/Failed or semantic rejection removes the snapshot; successful accepted info restores readiness |
| Outline triangles and outstanding loc outline requests; inspection owner | Exact source generation + projection epoch + monotonically assigned outline request ID, plus pending key | Cached against accepted info indices. Info replacement/removal clears cache and pending requests; obsolete responses cannot repopulate it | Missing/mismatched pending identity returns no outline; no semantic mutation |
| Roof grids/visibility; `WorldRuntime` roof cache and `RenderRuntime` roof view | Accepted info version and viewed plane; focus queries choose roof IDs | Rebuild grids when version/plane changes; bounded retained grids. View policy is projection-only | Missing flags produce no roof ID; removal/version change invalidates retained geometry interpretation |
| Height inverse read-sets; `HeightDependencyStore` | Producer region, accepted ticket epoch/presentation, completeness | Rebuild from accepted contextual scene. Every required producer must prove the dependency; no stale reads for narrowing | Missing/incomplete/stale/invalid data forces conservative rebuild. Eviction/rebuild/unavailable outcomes remove proof |
| Object static-output baselines/comparison; `ObjectOutputStore` | Exact complete accepted producer output plus accepted ticket epoch/presentation; pending pre-edit baseline | Compare accepted full candidates; successful empty diff may avoid static reupload. Scoped installs refresh the same baseline | Untrusted baseline/candidate gives full install; invalidation cancels pending proof; removal clears retained and pending state |
| Animated-loc membership and aggregate geometry; `WorldRuntime` + existing `AnimationWorker` | One physical FIFO rebuild slot retains immutable `AnimationArtifactIdentity`: accepted nonempty region descriptors, source/presentation, requested cycle/advance, center/radius and smoothing/frozen settings. Nonempty GPU upload retains the same descriptor | Membership, smoothing or freeze changes discard the old completion and force replacement. Playback reconciles before polling. Source replacement removes old worker members. Ordinary clock/camera lag remains allowed; identities describe accepted producer inputs, including last accepted region data during a semantic rebuild | No-op retires pending context while preserving installed identity. Empty output and explicit shared-batch cleanup remove GPU identity. Current Empty/Failed, rejection or presentation/source rebuild hides the batch until survivors rebuild. No new worker protocol or freshness counter |
| NPC catalog/assets; `NpcRuntime` and source receiver | Catalog source receiver lifetime; assets source generation (immutable source-derived data, not authored map revision) | Source switch resets catalog/assets; missing IDs trigger asset requests. Scenario/map revisions drive later runtime reconstruction | Decode errors retained as bounded diagnostics/missing definitions; stale-source assets rejected |
| NPC geometry; `NpcRuntime` + world worker | Exact issued `PlaytestStamp` retires only its own slot. Frame reconciliation checks source/map/scenario/run, info version, camera/range/limit/vars and requested/actual/current presentation. GPU identity retains issued inputs, ground producers and worker decoder target/path | One physical FIFO request; ordinary tick/cycle lag remains allowed. Uploaded identity stays tied to its issued stamp even after a new request. Simulation owns its clock; metadata retains no full snapshot | Empty output, hide and source cleanup remove GPU identity. Hidden results drain; stale geometry, metadata and diagnostics cannot publish or retire a newer slot |
| Player geometry; `PlayerRuntime` + world worker | Monotonic token retires only its slot. Frame reconciliation checks spec/look, info version and requested/actual/current presentation; explicit teleport revokes publication. GPU identity retains exact issued frame, request key, ground producer and worker target/path | One physical FIFO request; ordinary animation/pose lag remains allowed. Later frames request updated geometry without relabeling the installed image. Ground uses accepted picker data; map/history unaffected | Empty output, hide, invalid plane, missing ground and source cleanup remove GPU identity. Hide/teleport discard staged output and revoke the outstanding result, which drains before retry; stale diagnostics cannot publish |
| Cache textures and HD materials; source receiver, palettes, `RenderRuntime` | Active source receiver lifetime, lazy HD load, explicit render path (HD/Reference/fallback) | Recreated per source/profile requirements; shared immutable decoded data | Source receiver replacement prevents old messages crossing into new source. HD failure is explicit; Reference fallback is distinguished from requested HD output |
| Theme shader candidates; `RenderRuntime`, orchestrated by `theme_studio/integration.rs` | Project/source generation + authored edit revision; asynchronous GPU validation; pending candidate ownership | New selection, source/edit change or cancellation supersedes candidate. Restoration follows authored shader definition | Validation failure/cancel prevents activation and records error; rejected definition suppresses endless restore retry |
| Frame targets, draw data, viewport/shadow reuse; `RenderRuntime` / `SceneRenderer` | Effective frame parameters plus renderer-owned upload/target invalidation | Camera movement changes view/residency, not authored edits. Stable resources reused; resize rebuilds targets. Shadow state stays GPU-owned | Removal/upload/theme activation invalidates reuse; device/resource errors are not semantic edit authority |
| Terrain stroke/path and object placement previews; controllers/`InteractionRuntime` | Canonical plan/gesture inputs; spline key covers points/cursor/options/plane/frozen extent/source, Reference appearance adds accepted info version. Object ghost uses the current armed definition/pose and accepted ground each frame | One spline fitting thread, one replaceable pending input/result per interaction runtime, with 24 ms debounce. Stale spline tiles may draw faded. Commit recomputes placed-point fitting and semantic checks | Restore reversible live tile mirror on cancel/failure, preserve history/redo; stale spline results cannot become current. Cancel/source replacement clear retained slots and revoke active results while reusing the draining thread. Owner drop ends it after any active fit |
| Building candidate previews; `BuildState` | Shared immutable source/session/form/site/plane/footprint inputs. Valid desired inputs precede polling; batch/candidate inputs match before admission | One retained native worker, one replaceable queued snapshot and one current logical response slot; at most eight candidates per batch. Current getters guard apply/save/selection/drawing, including catalog changes at the same revision; separate last-preview read is verification-only | Invalid/unknown required inputs retire tickets and queued data. Active native candidates drain before replacement. Stale batches cannot retire current jobs; failure has no automatic retry loop. Synchronous script planning can overlap one draining native candidate; no semantic/history mutation |
| Auxiliary thumbnails, minimap/world-map images, sound renders; asset/audio/source owners | Selected source/receiver lifetime and asset key; these represent cache inputs, not a general revisioned edited-map projection | Rebuild on source/key/request; do not advertise them as current authored document queries | Reset source-bound consumer state on source opening; source receiver isolation is required |

Detached Gallery/Studio and export inspectors compose their own world/render/session lifetimes.
The same acceptance rules apply within each instance; artifact identity is not shared across viewers.
This inventory does not certify every asset-cache memory budget or independent OSRS visual parity.

## Regression evidence

These tests check production owners rather than reimplementing their decisions in a test scheduler.
Test names are searchable in the owning files; cache-dependent parity prerequisites remain explicit.

| Contract | Existing/new executable evidence |
|---|---|
| Current FIFO ticket, epoch rejection, cancellation/replacement, presentation/source lifecycle | `world_runtime/builds/tests.rs`: `current_result_finishes_with_original_scope`, `edit_epoch_rejects_old_result_and_retires_request`, `cancelled_old_result_cannot_consume_newer_request`, presentation/source tests |
| Semantic candidate gate and no project consumption on rejection | `streaming/region_install/tests.rs`: full/scoped rejection, valid hydration/dirty preservation, stale completion, Object narrowing, headless GPU rejection |
| Current Empty/Failed removes full/scoped projections, preserves clean/dirty semantics and pending restoration, suppresses looping and permits valid recovery | `region_install/terminal_tests.rs::gpu_current_empty_and_failed_hide_full_region_preserve_session_and_retry`; shared lifecycle retry/proof/outline/animation regressions |
| Stale terminal outcomes preserve newer installation, pending comparison, request, dirty work, selection and notice | `terminal_tests.rs::gpu_stale_empty_and_failed_preserve_newer_projection_request_and_notice`; BuildTracker source/presentation/epoch tests |
| Collision blocks stale/failed edit builds and recovers only with accepted info | `world_runtime/projection_contract_tests.rs`: `collision_is_unavailable_during_dirty_in_flight_and_failed_scoped_work`; `inspection.rs` collision tests |
| Dirty-zone coalescing, full subsumption, preservation of superseded required areas | `projection_contract_tests.rs`: `dirty_zone_work_coalesces_and_full_work_subsumes_it`, `later_edit_retires_old_scope_and_schedules_the_pending_scope`, `full_build_keeps_full_obligation_after_a_newer_zone_edit` |
| Unrelated hydration revision is observational | `projection_contract_tests.rs`: `unrelated_hydration_revision_does_not_reject_a_current_region_ticket` |
| Membership transition invalidates in-flight animation output | `projection_contract_tests.rs`: `animation_membership_changes_discard_the_older_in_flight_batch`; lifecycle rejection test; GPU rebuild removal regression |
| Exact animation dispatch/member identity, no-op retirement, descriptor lifetime and source reset | `world_runtime/animation/tests.rs`; existing single-flight worker with a synthetic nonempty triangle |
| Smoothing/freeze/source supersession, including toggle roundtrips | `projection_contract_tests.rs::animation_settings_and_source_changes_discard_the_older_in_flight_batch` |
| Animation identity follows nonempty/no-op/empty GPU publication and terminal/rebuild/source cleanup; frame reconciles freeze before polling | `streaming/animation_tests.rs`; synthetic geometry proves lifecycle policy, not sequence math or external visual parity |
| Actor presentation supersession/recovery, exact issued metadata during clock lag, nonempty ground producers, empty/hide metadata release | Expanded actual-GPU `npc_runtime/tests.rs` and `player_runtime/tests.rs`; weak request handles verify installed input retirement |
| Actual actor decoder provenance, generation and requested-HD Reference fallback | Pinned-cache `osrs-world/tests/lumbridge_world_scene.rs::actor_results_capture_real_decoder_target_generation_and_reference_fallback`; independent pose/visual parity remains separate |
| Proof completeness/fallback and exact static comparison | `height_dependencies.rs`, `object_output_store.rs`, `builds/tests.rs`, `invalidation/tests.rs`; affected world/scene parity suites remain separate |
| Outline source/epoch/request and roof-cache version/plane | `inspection.rs` outline tests; `roof_view.rs::cached_roofs_reuse_only_the_same_version_and_plane` |
| Actor request retirement and current-frame acceptance | `npc_runtime/tests.rs::old_npc_stamps_cannot_retire_a_newer_inflight_slot`, `player_runtime/tests.rs::old_tokens_cannot_retire_the_current_player_request`; actual arrival/update GPU lifecycle tests in both owners |
| Actor hide/show, source/revision/run/input supersession, replacement recovery and accepted clock lag | `gpu_npc_publication_reconciles_current_frame_and_allows_simulation_lag`, `gpu_player_publication_reconciles_hide_teleport_look_ground_and_source`; synthetic payloads test installation, not independent model parity |
| Preview cancellation/supersession | `interaction_runtime/spline/tests.rs`: `editing_stales_a_ready_preview_before_the_worker_answers`, cancellation and source-replacement tests; terrain controller cancellation tests |
| Spline burst/result bounds, same-thread cancel/resume/source replacement, key roundtrips, shutdown and terminal retry | `interaction_runtime/spline/plan/worker/tests.rs`; controlled start/release handshakes around actual fitting, current-key admission and weak mailbox retirement |
| Building invalid-input cancellation, stopped worker, same-frame admission, stale batch/current slot, hydration and shared-input retirement | `prefab_runtime/build/tests.rs`; real generator plans, controlled channel ordering and the real native candidate worker |
| Building catalog arrival/size changes, source dimensions over label placeholders, unresolved required sizes, rotated outside anchors and exact undo | Additional `prefab_runtime/build/tests.rs`; canonical query/kit tests in `osrs-edit`; unlisted catalog dimension tests in `osrs-world` |
| Building physical concurrency, queued/completed lifetime, obsolete ticket scope, shutdown/panic, seed wrap/ranking/patch equivalence and synchronous supersession | `prefab_runtime/build/candidates/worker/tests.rs`; paused real candidates and ordinary BuildState admission, same-thread IDs, weak metadata and canonical planning/inverse checks |
| Placement review semantic hydration/eviction, source/asset/gesture/plane/catalog rejection, current gateway commit/undo and unarmed script inputs | `prefab_panel/placement_review/tests.rs`; real EditRuntime and checked portable dimensions |
| Capture admission/save rejection after hydration/eviction at unchanged authored revision, source rejection and redo preservation | `prefab_panel/capture/tests.rs`; existing floor/courtyard/piece round trips remain unchanged |
| Capture physical replacement/cancel/reselection bounds, queued/completed lifetime, old ticket scope, shutdown/panic/retry and stale pending/payload rejection | `prefab_panel/capture/worker/tests.rs`; controlled handshakes, real membership result comparison and same-thread IDs |
| Shared accepted source identity, original dispatch revision, full/scoped/empty-zone installs and Object proof retention | `streaming/region_install/identity_tests.rs::gpu_partial_refresh_keeps_untouched_identity_and_shares_new_identity_with_picker` |
| Stale/rejected/terminal/source/rebuild removal and Euclidean negative-zone lookup | Remaining actual-GPU `identity_tests` cases; synthetic geometry proves publication/resource policy, not external visual parity |
| Retained inspection cannot certify another source/presentation at the same producer epoch | `projection_contract_tests.rs::retained_inspection_from_another_source_or_presentation_cannot_certify_collision` |
| Worker reports actual target/extraction inputs | Pinned-cache `osrs-world` test `region_artifact_retains_exact_source_and_extraction_inputs`; prerequisites are explicit |
| Viewport reuse invalidated by uploads/resize | `render_runtime/reuse.rs` CPU and GPU tests; not independent visual certification |

## Actor publication boundary

`stream()` collects actor completions before the frame reconciles settings, authored revisions and
queued playtest commands. Actor owners therefore stage only a matching completion; their `update()`
validates it against the current host before GPU upload or diagnostics. NPC publication checks the
issued tick stamp at arrival, then the current semantic/run and extraction inputs after reconciliation.
It deliberately permits a requested pose to lag a progressing simulation clock. Exact-current-cycle
acceptance would continually reject useful work while the worker is slower than the actor clock.
Player animation/pose frames likewise may lag, but spec/look, explicit teleport and ground-version
supersession cannot publish old candidates.

Hide clears staged output and its publication permission, while retaining the physical FIFO slot
until completion. This prevents a hide/show from issuing indistinguishable NPC requests for the same
tick stamp. Source replacement resets the owner and receiver lifetime. The original publication
checkpoint changed no worker payload, actor clock, authored transaction or persistence format.
The subsequent identity checkpoint extends the existing Rust result payloads with extraction context.
NPC geometry lifecycle methods live in
`npc_runtime/geometry.rs` to keep the existing parent within its production file cap.

## Terminal region outcomes and retry

The single FIFO ledger gates Region, Empty and Failed events. `Empty` means the worker could not
find the producer's source map, even for a scoped request; a successful extraction with no triangles
is still a Region candidate and keeps the normal scoped upload/removal behavior.

A **current** Empty or Failed hides the entire producer region, including static zones outside a
requested partial scope. The same `WorldRuntime::region_unavailable` operation used for semantic
rejection removes picker/collision/outline data, animated membership, Height/Object proofs and
pending comparisons. Shared animated GPU geometry is hidden until surviving membership rebuilds;
in-flight older animation cannot republish it. Selection clears with the removed picker data.
Empty stays quiet; Failed reports the region and worker error. A stale completion retires its own
FIFO ticket only and cannot change current projections, dirty work, selection or notices.

These terminal outcomes do not unload semantic documents, including clean hydrated baselines.
History, revisions, dirty state and pending project restoration remain untouched. Distance eviction
and source replacement retain their existing independent semantic residency rules. The unavailable
marker suppresses automatic per-frame retry. Rebuild, source replacement, eviction/re-entry or newer
semantic invalidation may retry; a zone edit over unavailable residency escalates to full recovery.
Only a valid accepted candidate can consume pending project restoration work.

## Remaining decisions and gaps

1. **Transient artifact identity:** static zones, inspection, aggregate animation and actor layers now
   retain accepted source/build descriptors; building candidates retain their local request context.
   Complete preview definition-input and portable content-hash/reproduction
   correlation are not covered by this static descriptor. Preserve each owner's clock/lag contract;
   do not invent a second freshness serial or claim integrated `RenderSnapshot` use.
2. **Worker-mirror replacement:** semantic snapshots must preserve preview cancellation, source-height
   override semantics and project restoration before compatibility mirrors can disappear.
3. **GPU resource budgets, bounded queues and scheduling priority:** unbounded channels and full
   contextual CPU builds remain. Performance measurement/optimization is paused; this checkpoint
   changes correctness bookkeeping only and makes no speed/memory improvement claim.

The numeric normal-method fidelity checkpoint is recorded in the technical audit. Broader roof,
raster, texture and independent visual evidence remain open. Continue transient artifact identity
and reproduction from their existing owners. Parallel worker execution and a generic projection
framework are not prerequisites. The dataset exporter composition move and subsequent observed
workspace integration gate are recorded in the technical audit; this lifecycle inventory alone
does not certify those boundaries.

## Checkpoint verification

The initial targeted run reproduced membership and superseded-area failures. All six contract tests
passed after the fixes. Final local focused build gate: strict native all-target/all-feature Clippy
and 379 tests passed, 0 failed, 4 pinned-cache tests ignored; stable source before/after in
`target/verify/gates-20261019-110314/report.json`. GPU logs confirm the presentation rebuild and
semantic rejection paths actually ran. Strict WASM editor Clippy, formatting, architecture/authority/
feature-index guards and changed Markdown links passed. Only verification documentation was updated
afterward. No full-workspace, real-mouse, remote CI or independent visual/performance proof is implied.

The actor-publication checkpoint passed strict native Clippy and **383 tests, 0 failed, 4 ignored**
in `target/verify/gates-20261019-111302/report.json`, with a stable before/after source hash. The first
targeted run explicitly exercised both actual GPU lifecycle paths. Strict WASM editor Clippy,
formatting, architecture/authority/feature-index guards, changed Markdown links and whitespace
checks passed. Only verification documentation changed afterward. See the handoff for the exact
source hash and timings; the 379-test result above applies to the preceding checkpoint.

The terminal-outcome checkpoint passed strict native editor Clippy and **385 tests, 0 failed,
4 ignored**, with stable before/after source in `target/verify/gates-20261019-173825/report.json`.
The two focused terminal tests explicitly exercised actual GPU paths; the final suite also covers
successful empty scoped output preserving other zones. Strict WASM editor Clippy, architecture/
authority/feature-index guards, formatting, changed Markdown links and whitespace checks passed.
Only verification documentation changed afterward. The handoff records exact timings/source hash;
earlier results retain their historical checkpoint scope.

Static/inspection identity checkpoint: strict native editor Clippy and **389 tests passed, 0 failed,
4 ignored**, stable source in `target/verify/gates-20261019-181145/report.json`. Three focused
actual-GPU tests exercised identity sharing/retention and lifecycle cleanup; a fourth CPU regression
covers cross-source/presentation collision. World lib/tests Clippy and strict WASM editor Clippy
passed. Pinned-cache source/extraction metadata and the unchanged Reference pixel golden passed.
Architecture/authority/feature-index checks, formatting, local links and whitespace passed; only
contract/verification documentation changed afterward. At that checkpoint world all-target checks were uncertified
because of the dataset-example Lab dependency issue, repaired by the subsequent composition move. These are internal lifecycle and
Reference regression proofs, not independent visual or performance certification.

Subsequent workspace integration passed strict native all-target/all-feature and WASM Clippy,
plus **1,384 tests, 0 failed, 23 ignored** in `target/verify/gates-20261019-182336/report.json`.
The final full quick stage passed in `target/verify/gates-20261019-183616/report.json` against
the same stable source snapshot. The handoff records exact source identity, prerequisite skips,
the byte-identical exporter relocation and test-only lint repairs. Only documentation changed
after those gates; transient identity and independent external evidence remained open at that checkpoint.

Aggregate animation checkpoint: all 12 focused animation checks passed, explicitly logging all three
actual-GPU cases. Final editor gate passed strict native Clippy and **398 tests, 0 failed, 4 ignored**,
plus Python/architecture/authority/feature-index/source-pin/format checks in
`target/verify/gates-20261019-225952/report.json` (34.4 s), with stable source before/after.
Strict WASM editor Clippy passed (21.4 s); links and whitespace passed. Only documentation changed
afterward. The handoff records the source hash and fixture-clock correction. These are lifecycle/
identity proofs, not a sequence-math, portable reproduction, external visual or performance oracle.

Actor identity checkpoint: strict native editor/world Clippy and **456 tests, 0 failed, 22 ignored**,
plus the quick guard/Python/source-pin/format checks passed in
`target/verify/gates-20261019-231431/report.json` (149.4 s), with stable source before/after.
Strict WASM editor/world Clippy passed (21.8 s). Expanded actual-GPU cases exercised descriptor
retention and release; the separately run pinned-cache actor-context test passed (0.24 s), with
nonempty real default-player output and exact Reference/fallback geometry equality. Existing
prerequisite skips/ignores remain. Only documentation changed after the final gates. See the handoff
for source hash, command and NPC fixture provenance correction; independent pose/visual and portable
reproduction evidence remain separate.

Building preview checkpoint: all eight new admission tests passed, including the real candidate worker,
real-session hydration, current-read guards and stale-batch slot preservation. Final editor gate passed
strict native Clippy and **406 tests, 0 failed, 4 ignored**, plus the guard/Python/source-pin/format
checks in `target/verify/gates-20261019-233653/report.json` (70.9 s), with stable source before/after.
Strict WASM editor Clippy passed (11.9 s) after correcting target-specific tail control flow without
an allowance. The existing generator planning/validation body is byte-identical. Only documentation
changed afterward; the handoff records the hash and initial fixture correction. Footprint-definition,
queue/review, portable reproduction and independent visual/performance obligations remain open.

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
