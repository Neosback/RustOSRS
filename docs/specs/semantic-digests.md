# Canonical map and transaction digests

Status: editor policy, version 1. This specifies RustOSRS content identity, not an OSRS client rule.
Decision: [ADR-0016](../adr/ADR-0016-canonical-semantic-content-identity.md).
Owner: [osrs-edit::content_digest](../../crates/osrs-edit/src/content_digest.rs).
Tests: [content_digest/tests.rs](../../crates/osrs-edit/src/content_digest/tests.rs).

## Scope

`MapDocument::content_digest_v1()` identifies complete canonical map content. Equal map values
produce equal digests independently of insertion order, object-set hashing seed, platform word size
and host endianness. It includes sparse terrain, explicit absolute height vertices and exact object
occurrences. Default terrain is absent under existing document normalization; unknown height and
explicit zero are different. `Some(0)` is distinct from `None` for an optional semantic ID.

`MapPatch::content_digest_v1()` identifies the exact ordered map transaction. It includes every
operation's before/after values, no-op operations and object addition/removal polarity. Operations
are never sorted, coalesced, applied or validated by hashing. Different transactions can produce the
same resulting map and still have different patch digests. Derived `ChangeImpact` is not encoded.

Both return the existing `osrs_core::provenance::Digest256`; its display is 64 lowercase hexadecimal
characters. Callers must record the algorithm identifier alongside the digest. These methods are
explicit diagnostics/reproduction operations, not commit, undo or per-frame work. They do not clone
a map, but document enumeration sorts object occurrences through the existing canonical query.

Source/cache/profile/build identity, loaded/available regions, revisions, appearance, scenarios,
selection, clocks/seeds, feature/intent payloads, definition dimensions and rendering state are not
map content. A reproduction envelope must retain the inputs it needs separately. [Validated map/patch transport](semantic-transport.md)
is the separate infrastructure codec for these canonical values. A map digest does
not replace a session freshness check, source identity, workspace digest, project file or journal.

## Byte grammar

The digest is SHA-256 of the following concatenated bytes. All integers use fixed-width little-endian
encoding, without alignment/padding. Signed integers use two's complement. A byte is unsigned u8.
No Rust `Hash`, JSON text, debug formatting or machine-native lengths enter the preimage.

| Value | Encoding |
|---|---|
| Position | x:i32, y:i32, source plane:u8 |
| Optional underlay | byte 0 for None; byte 1 followed by id:u32 for Some |
| Optional overlay | byte 0 for None; byte 1 followed by id:u32, shape:u8, rotation:u8 for Some |
| Terrain value | optional underlay, optional overlay, flags:u8 |
| Object occurrence | position, kind:u8, object id:u32, orientation:u8 |
| Collection length | u64, even on 32-bit targets |

Plane, kind, orientation, shape and rotation are already validated canonical values. IDs retain
all 32 bits; world coordinates and heights retain all signed 32 bits. This grammar is a hash preimage,
not a new parser or persisted map/patch serializer. Untrusted imports still need semantic validation.

### Document v1

Algorithm identifier: `rustosrs-map-document-v1` (exported as
`MAP_DOCUMENT_DIGEST_ALGORITHM_V1`). Preimage:

1. UTF-8 algorithm identifier followed by one zero byte.
2. Terrain count, then each position + terrain value sorted by `(x, y, plane)`.
3. Height count, then each position + height:i32 sorted by `(x, y, plane)`.
4. Object count, then each occurrence sorted by `(x, y, plane, kind, id, orientation)`.

These are the canonical document enumeration orders. Sparse terrain defaults and duplicate exact
objects have already normalized through `MapDocument`; the digest adds no new normalization.
An empty document therefore has a 49-byte preimage (domain + zero + three zero u64 counts).

### Patch v1

Algorithm identifier: `rustosrs-map-patch-v1` (exported as
`MAP_PATCH_DIGEST_ALGORITHM_V1`). Preimage:

1. UTF-8 algorithm identifier followed by one zero byte.
2. Operation count as u64.
3. Each operation in original order, using the following tags and payloads.

| Tag | Operation | Payload |
|---|---|---|
| 0 | SetTerrain | position, before terrain, after terrain |
| 1 | SetHeight | position, before:i32, after:i32 |
| 2 | AddObject | object occurrence |
| 3 | RemoveObject | object occurrence |

An empty patch has a 30-byte preimage. A future semantic extension or changed encoding must introduce
a new algorithm/version/API; these v1 methods and bytes remain frozen. Feature provenance belongs
in a replay envelope, as required by [the replay design](../architecture/SEMANTIC-REPLAY-PROJECTION.md).

## Golden vectors and failure signature

The four test vectors were calculated independently with Python `struct.pack` and `hashlib.sha256`
from this grammar. No production Rust encoder generated their expected values.

| Input | Preimage bytes | SHA-256 |
|---|---:|---|
| Empty document | 49 | `cdbefee4f3b86a8c36b1ba25a013cf41296e0ecb209a43d52976f10b0febb8d0` |
| Empty patch | 30 | `1aa689026d259fcb52024246336d5d947016fe7c4a5c2e5b38d1f1900743a7d6` |
| Full-width document fixture below | 114 | `4275357880ccc389af2c5edcadc1eed2976eacf4464d463d669620dcba3fe2d3` |
| Ordered patch fixture below | 106 | `7b87e40f378dda152affe42713e4762ca1241fe2417f1d003aa51d645e1524d0` |

The document fixture contains terrain at (-65,128,p3), underlay Some(0), overlay
Some(id=4294967295,shape=11,rotation=3), flags=255; height -2147483648 at
(-2147483648,2147483647,p2); and two objects at (-1,-64,p1): (kind=0,id=0,orientation=3)
and (kind=22,id=4294967295,orientation=2).

The patch fixture, in order, sets that terrain from default, sets that height from -2147483648 to
2147483647, adds the kind-22 object and removes the kind-0 object. It is an encoding fixture, not a
claim that this patch applies to the document fixture.

Tests pin those bytes, document insertion-order/default/object-set normalization, signed boundaries,
every value/coordinate channel, absent versus zero height, patch order/before-state/polarity/no-ops,
and hash restoration after exact inverse application. Any expected-vector drift is a protocol failure;
never regenerate expectations merely because candidate output changed. These tests establish the
content-identity primitive, not complete replay, crash recovery or external visual reproduction.
