# Authored document snapshot v1

Status: native editor policy adopted by [ADR-0020](../adr/ADR-0020-authored-document-snapshot.md).
Owner: existing [EditorSession](../../crates/osrs-editor/src/session/edit_session.rs) and
[private snapshot composition](../../crates/osrs-editor/src/session/authored_snapshot.rs).
Tests: [codec/identity](../../crates/osrs-editor/src/session/authored_snapshot/tests.rs),
[export lifecycle](../../crates/osrs-editor/src/app/project_runtime/native/snapshot/tests.rs).

## Product and checker

File → **Save authored snapshot…** captures the current map, NPC scenario and appearance into a new
file. `project snapshot PATH` invokes the same owner from native scripts, including paths with spaces.
The source must be open and the world must own the document. This action does not mark the project
saved or change its save path. Existing files are never replaced. A bounded background operation
serializes/publishes the explicitly frozen document; later edits or project/source replacement cannot
rewrite it. A second capture waits for the current export's result to retire.

Verify without UI, GPU, installed cache, file mutation or live-session replacement:

```sh
cargo run --locked -p osrs-editor -- --verify-authored-snapshot /absolute/path/snapshot.json
```

MATCH reports scope `map-scenario-appearance`, four content identities and object/spawn/asset counts.
The checker validates the snapshot, creates an isolated baseline and verifies a canonical encode/
decode round trip. Wrong usage, malformed/oversized/non-file input, unsupported formats, invalid
domains or digest mismatch emit JSON FAILED and nonzero process status.

## Closed wrapper

| Required field | Contract |
|---|---|
| `format` | `rustosrs-authored-snapshot-v1` |
| `algorithm` | `rustosrs-authored-document-sha256-v1` |
| `identity` | Closed required map/scenario/appearance/authored digest record |
| `map_json` | Complete unchanged [canonical map snapshot JSON](semantic-transport.md) text |
| `scenario_json` | Complete compact JSON of the optional canonical ScenarioDefinition |
| `appearance_zip` | JSON byte array of complete canonical ThemePack ZIP transport |

Scenario text is required: `null` is an absent scenario, not a missing field or a present empty
scenario. ScenarioDefinition and every spawn, behavior, waypoint, identity, location, source field
and authored seed retain their existing serde representation and canonical validation. Spawn order
is preserved; same-location spawns are distinct when IDs differ. Invalid identity duplication,
facing, location, patrols, planes or scenario version/source metadata reject through canonical owners.

ThemePack uses its existing deterministic ZIP writer and checked reader, including asset hashes,
entry/path/expansion limits and definition validation. Assets remain complete immutable bytes and
are never extracted to disk. Editor appearance admission still rejects unsupported material/selector
features. This does not activate shaders or prove GPU compilation. Ordered rules/provenance remain
ordered. Theme manifests/definitions and scenario nested values retain their canonical decoder
policies; the new closed wrapper does not claim stricter policies for every existing nested type.

Map and scenario text remain intact until their actual decoders see them, including duplicate keys.
All four declared identities must be lowercase 64-digit SHA-256 values and must match recomputed
canonical content. The input cap is 128 MiB of UTF-8 JSON; decode checks before parsing and native
reading checks regular-file kind/size before allocation, then bounds reading again against growth.
Encode rejects after serialization if it exceeds the cap. A valid larger document/pack is rejected,
never truncated. This transport cap does not prove a whole-process memory or solver-time budget.

## Versioned identity grammar

All hashes use SHA-256. Digest strings are lowercase hex; composite preimages use raw 32-byte values.

1. `map` is the existing MapDocument `content_digest_v1()` identity.
2. `scenario` hashes UTF-8 `rustosrs-authored-scenario-v1`, one zero byte, then compact serde JSON of
   the complete optional ScenarioDefinition, in canonical declared field order. `None` contributes
   bytes `null`; a definition contributes the ordinary object, without another option tag. UTF-8
   strings and fixed-width typed integers preserve all declared data. No sort or source-name rewrite.
3. `appearance` is the existing validated ThemePack identity: complete definition and ordered asset
   path/checksum manifest, with checksums verified against every asset's bytes. ZIP compression,
   timestamps and container byte layout are not authored identity.
4. `authored` hashes UTF-8 `rustosrs-authored-document-v1`, one zero byte, then raw `map`, `scenario`,
   `appearance` digest bytes in exactly that order (96 bytes after the domain separator).

Independent Python/hashlib vectors pin empty-map/absent-scenario/stock-appearance identities and a
full-width Unicode scenario with all behavior variants. Existing map digest vectors and theme
semantics remain authoritative. A protocol/domain/serialization change affecting these bytes needs
a new explicit version, not silently updated expectations.

## Content and authority boundary

Decoding validates all domains before creating a fresh EditorSession. Map/scenario/authored counters
are zero; history, redo, dirty and available-region observations are empty. No transaction occurred.
The live session is never reset or replaced, and its history/dirty/save state remains unchanged.

Snapshots contain every current authoritative authored value in EditorDocument. They do not assert
that all source regions have loaded, apply pending project diffs, or include unopened source/cache
bytes. Source/profile/build/project identity, availability, pending project restoration, simulation
positions/queues/clocks, UI state, selection, worker/GPU caches and history remain separate concerns.
Authored content equality does not establish source freshness or authorize stale work.

This is a baseline transport for future reproduction envelopes. Existing building v1 success and
failure bundles remain map-scoped. Full operation replay, projection settling/images, recovery
journals, editor-wide headless/MCP and independent OSRS visual fidelity are not implied by MATCH.
