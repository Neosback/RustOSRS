# Semantic map and patch transport v1

Status: RustOSRS editor policy. This is a lossless infrastructure protocol, not an OSRS client format.
Decision: [ADR-0017](../adr/ADR-0017-semantic-map-transport.md).
Owner: [semantic_transport.rs](../../crates/osrs-edit/src/semantic_transport.rs) and its private
[value records](../../crates/osrs-edit/src/semantic_transport/values.rs).
Tests: [semantic_transport/tests.rs](../../crates/osrs-edit/src/semantic_transport/tests.rs).
Content identity: [semantic-digests.md](semantic-digests.md).

## API and scope

- `MapDocument::to_snapshot_json_v1` / `from_snapshot_json_v1` transport complete canonical map state.
- `MapPatch::to_patch_json_v1` / `from_patch_json_v1` transport an exact ordered map transaction.
- All four return `Result<_, SemanticTransportError>`; decoders construct new values without changing
  a live map, session, revision, history, dirty flag or file.

Snapshot scope is terrain, explicit heights and exact object occurrences, including the entire signed
world coordinate range. Source/cache/profile/build, region availability, appearance, scenarios and
invocation/clock/definition inputs are separate envelope inputs. This is not a project file, full
workspace snapshot, recovery journal or proof of intent reproduction. No raw snapshot/patch action
is added to an agent/AI/Lab operation surface. Live edits retain their normal gateway.

## Envelope and primitive records

Every envelope contains a required `header` object:

| Field | Snapshot value | Patch value |
|---|---|---|
| `format` | `rustosrs-map-snapshot-v1` | `rustosrs-map-patch-json-v1` |
| `digest_algorithm` | `rustosrs-map-document-v1` | `rustosrs-map-patch-v1` |
| `digest` | canonical document SHA-256 | canonical ordered-patch SHA-256 |

The format constants are `MAP_SNAPSHOT_FORMAT_V1` and `MAP_PATCH_FORMAT_V1`. Digests use exactly 64
lowercase hexadecimal characters and are recomputed after semantic decoding. JSON whitespace, object
key order and snapshot record order are irrelevant to hashing. Patch operation order is significant.
The format and hash algorithm are checked independently; future semantics require explicit versions.

| Record | Required fields |
|---|---|
| Position | `x`:i32, `y`:i32, `plane`:u8 validated as SourcePlane (0–3) |
| Terrain value | `underlay`:u32 or null; `overlay`:overlay object or null; `flags`:u8 |
| Overlay | `id`:u32, `shape`:u8 validated 0–11, `rotation`:u8 validated 0–3 |
| Terrain row | `position`:position object, `value`:terrain value |
| Height row | `vertex`:position object interpreted as HeightVertexCoord, `value`:i32 |
| Object occurrence | `id`:u32, `position`:position object, `kind`:u8 validated 0–22, `orientation`:u8 validated 0–3 |

All fields are required, including nullable terrain channels. Integers must be JSON integers in their
full declared range; there are no sentinel IDs, narrowing casts, guessed heights or omitted channel
defaults. Optional `Some(0)` remains distinct from null. Raw position fields are private wire data and
immediately construct canonical MapTile/SourcePlane/MapPosition or HeightVertexCoord. IDs reuse the
canonical osrs-core ID wrappers. Unknown fields at every object level and duplicate JSON keys reject
the input rather than being dropped. The parser also rejects trailing/malformed/truncated JSON.

## Snapshot

Required top-level fields are `header`, `terrain`, `heights`, `objects`. All three collections are
arrays of the corresponding records. Empty collections are explicit arrays. An empty snapshot is:

```json
{
  "header": {
    "format": "rustosrs-map-snapshot-v1",
    "digest_algorithm": "rustosrs-map-document-v1",
    "digest": "cdbefee4f3b86a8c36b1ba25a013cf41296e0ecb209a43d52976f10b0febb8d0"
  },
  "terrain": [],
  "heights": [],
  "objects": []
}
```

Terrain coordinates, height coordinates and exact object occurrences must each be unique within
their collection. Duplicate records reject even when their values agree, or when sparse normalization
would remove a default tile. Distinct object occurrences can share an anchor. Snapshot terrain rows
with an explicit default tile normalize through `MapDocument::from_parts`; they are not retained in
canonical output. An absent height stays absent; an explicit height of zero changes content identity.
The encoder emits canonical document enumeration order from the existing owner.

## Ordered patch

Required top-level fields are `header`, `operations`. Each operation is a closed object with `op`:

| `op` | Additional required fields |
|---|---|
| `set_terrain` | `position`:position, `before`:terrain value, `after`:terrain value |
| `set_height` | `vertex`:height-vertex position, `before`:i32, `after`:i32 |
| `add_object` | `placement`:object occurrence |
| `remove_object` | `placement`:object occurrence |

Operation order, repeated positions, no-ops, before/after values and polarity remain exactly as given.
Unlike snapshot duplicate entries, repeated patch operations are legal transport data. They may be
inapplicable to a particular map; applicability is checked by the existing semantic validator/gateway,
not by transport. `MapPatch::new` derives ChangeImpact from decoded operations. A supplied `impact`
field is an unknown-field error. No feature/tool provenance is inserted into MapPatch.

## Errors and evidence

Errors distinguish malformed JSON/schema, unsupported format/algorithm, invalid digest, invalid
canonical values, duplicate terrain/height/object and content-digest mismatch. Invalid canonical
values retain a typed cause with their collection and zero-based row/operation index. Duplicate errors
retain the exact canonical coordinate/occurrence. Failed decode produces no partly usable map/patch.

Tests decode hand-authored full-width/signed data with the independently calculated digest vectors,
round-trip complete maps and all four operations, preserve zero/absence and sparse normalization,
reject malformed numeric domains/schema/duplicates/versions/hash tampering, and prove unchanged
ordered patches commit/undo through normal history with exact inverse and redo preservation on
invalid application. These prove transport integrity and semantic value round trips, not full
portable editor reproduction, recovery, workspace identity or external visual fidelity.
