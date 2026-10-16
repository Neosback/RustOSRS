# Initial clipping oracle

Requires Python 3.11+, JDK 17+ and the exact external RuneLite mirror pinned in `sources.json`.
The source declares game revision 235; RustOSRS's selected cache is 241. This verifies clipping
primitives and ordinary top-level map initialization, not complete target-client movement parity.

```sh
python3 scripts/collision_oracle/regenerate.py --runelite-root /path/to/RuneLite-melxin --check
cargo test --locked -p osrs-scene collision
```

Without `--check`, regeneration writes `crates/osrs-scene/tests/fixtures/collision-deob.txt`.
Review fixture changes alongside driver/source-pin changes. The Rust inventory test requires all
94 cases, and compares every corresponding 104×104 grid through a SHA-256 of x-major big-endian
u32 flags. Additional Rust tests own bridge/provenance/coverage cases, not extra Java oracle claims.

The driver compiles the unmodified `CollisionMap.java` with five original mapping annotations in a
temporary directory. Five context-source pins cover terrain loading, floor projection, loc placement
and top-level/expanded world selection; they are verified but not compiled into this isolated driver.
No external source files, annotation stubs or cache assets are copied into this repository.

The canonical contract and prerequisite-gated build-241 integration command are in
[`collision.md`](../../docs/specs/collision.md).
