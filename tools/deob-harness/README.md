# deob-harness — run the deobfuscated client headless for golden data

`src/Dumper.java` (default package, so package-private deob members are
reachable) executes the real engine algorithms and prints `KEY=value` lines.
`run.sh` builds everything and refreshes
`reference-fixtures/deob_golden.txt` (162 lines).

## What it dumps (machine-checked, exit 0)

- `tileShape2D` / `tileRotation2D` — full tables from `Scene` static init
- `field800/804/802/798/803/805` — wall orientation/offset tables from `Tiles`
- `tri shape=R rot=R …` — 52/52 `SceneTileModel` triangulations (shapes 0–12 ×
  rotations 0–3): vertex positions, heights, faces, both color sets, texture
  ids, flat flag — plus one sloped-height case proving midpoint averaging
- `sethsl`, `m817`, `m2086`, `m5263`, `m5264` sweeps — color-function fixtures
- `contour flat/slope` — `Model.contourGround` on synthetic heightmaps
  (slope case hand-verified: bilinear gives −28 exactly)
- `tolit` — `ModelData.toModel` loc-rig lighting on a synthetic triangle
- `nudge …` — 27-row `WallDecoration.method6262` offset table (all orientation
  flags × sample inputs)
- `wall`/`decor`/`floor` — live `Scene` storage for wall types 0–3, decor
  types 4–8, and floor 22: positions, orientation flags, both offset pairs
  (exact `class150` call shapes, config words included)
- `roof`/`walldiag`/`gate2x1`/`cap5` — game-object path storage: 1×1 roof and
  diagonal-wall centers, multi-tile footprint sharing (same object on both
  tiles) with edge masks, and the 5-per-tile capacity gate

## Build notes (the three workarounds, all documented)

1. `bcprov-jdk15on-1.52.jar` (Maven Central, fetched once by `run.sh`):
   compile-only dep for the client's TLS classes, never loaded at runtime.
2. `stubs/netscape/javascript/JSObject.java`: the deob tree's vendored stub
   lacks a `getWindow` overload for its own `Client` type, and `javac`
   resolves that package from JDK module `jdk.jsobject` before any
   sourcepath — so the stub is compiled with `--patch-module` (compile only,
   never at runtime). Reference tree untouched.
3. Closure compilation: `javac -sourcepath` over `runescape-client` +
   `injection-annotations` compiles only what `Dumper` reaches; warnings
   suppressed (`-nowarn`, deprecation noise only).

## Extending

Add sections to `Dumper.main` using only side-effect-free deob calls
(pure tables, constructors, static functions, synthetic models). Never
touch `Client`, networking, or anything requiring a session — the harness
must stay runnable with zero game state. Re-run `run.sh` after any snapshot
update and diff the fixtures.
