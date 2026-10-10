# deob-harness — run the deobfuscated client headless for oracle candidates

`src/Dumper.java` (default package, so package-private deob members are reachable) executes the real engine algorithms and prints `KEY=value` lines.

This harness is a **low-level development adapter**. It no longer contains a machine-specific RuneLite path, downloads dependencies, or writes directly to `reference-fixtures/deob_golden.txt`.

Use the pinned public entry point instead:

```sh
python3 tools/reference-fixtures/regenerate.py \
  --adapter melxin-deob-golden \
  --checkout /path/to/melxin-runelite \
  --bcprov /path/to/bcprov-jdk15on-1.52.jar \
  --candidate /tmp/rustosrs-deob-golden.candidate.txt
```

The public adapter pins `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`. `run.sh` receives that expected commit and validates the checkout HEAD before compilation.

## Candidate-only safety

`run.sh` requires an explicit output path and refuses:

- a source checkout whose HEAD does not match the requested exact commit;
- missing source roots;
- a missing caller-supplied `bcprov` jar;
- an already-existing candidate output;
- any output path inside this repository's `reference-fixtures/` directory.

It does not clone, fetch, download Maven artifacts, or modify the supplied deob checkout.

## What it dumps

The current historical harness emits the same source-derived families used by `deob_golden.txt`:

- `tileShape2D` / `tileRotation2D` — full tables from `Scene` static init;
- `field800/804/802/798/803/805` — wall orientation/offset tables from `Tiles`;
- `tri shape=R rot=R …` — all `SceneTileModel` triangulations plus a sloped-height control;
- `sethsl`, `m817`, `m2086`, `m5263`, `m5264` color sweeps;
- `contour flat/slope` synthetic `Model.contourGround` cases;
- `tolit` synthetic `ModelData.toModel` lighting;
- `nudge …` wall-decoration offset cases;
- `wall` / `decor` / `floor` scene-storage cases;
- `roof` / `walldiag` / `gate2x1` / `cap5` game-object storage cases.

Checkpoint 5 owns indexing/migration of useful historical rows into canonical manifests. Checkpoint 4 does not reinterpret their provenance.

## Build notes

The caller supplies `bcprov-jdk15on-1.52.jar`; it is a compile-only dependency for client TLS classes and is never loaded at harness runtime.

`stubs/netscape/javascript/JSObject.java` handles the deob tree's `JSObject` compile mismatch through `--patch-module`. The reference checkout is not edited.

`javac -sourcepath` compiles only the closure reached by `Dumper.java` from:

- `runescape-client/src/main/java`;
- `injection-annotations/src/main/java`.

Warnings are suppressed because the deob closure contains legacy/deprecation noise.

## Extending

Add sections to `Dumper.main` using only deterministic, side-effect-free deob calls: pure tables, constructors, static functions, or synthetic models. Never touch live `Client` state, networking, credentials, or anything requiring a game session.

Regenerated output must remain a candidate until reviewed and explicitly promoted through the canonical fixture manifest/hash workflow.

## Terrain oracle and cache-backed archives (2026-10)

* `src/TerrainOracle.java` + `run-terrain-oracle.sh` run the real `class264.loadTerrain`,
  `ScriptFrame.method749`, and `class470.method9712`; see `reference-fixtures/terrain/README.md`.
* `src/CacheArchives.java` backs the deob's `AbstractArchive` with a disk cache so the client's own
  loaders read real data. `overlay/BZip2Decompressor.java` shadows the deob's broken hand-rolled
  bzip2 (its block-size constants overflow) with Apache commons-compress; put
  `commons-compress-1.21.jar` on the classpath and `overlay` first on the source path.
* `src/ObjectDecodeProbe.java` decodes every build-241 object definition with the deob's
  `ObjectComposition.decode`. Result: 62,384 of 62,522 decode, but 57,661 leave trailing bytes and
  138 throw, because the January deob predates the build-241 object layout. **The deob is therefore
  not a valid oracle for real object definitions or models on this cache**; use it only with
  synthetic definitions/models (as `Dumper` does) unless the deob is updated to match build 241.
