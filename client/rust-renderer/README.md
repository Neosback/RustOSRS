# Rust Renderer

This crate is the pure Rust/WASM replacement for the current TypeScript/PicoGL scene renderer.

The migration is deliberately incremental. TypeScript can continue decoding cache, scene, model, animation and game state while Rust takes ownership of GPU rendering behind a narrow versioned packet boundary.

## Current scope

Stage 0 is implemented, Stage 1 static-scene parity is near completion, and Stage 2 live actor rendering is actively landing:

- exact 12-byte packed OSRS vertex codec
- packed-vertex deduplication
- versioned numeric TypeScript/WASM scene packets
- Rust-owned WebGL2 context and shader lifecycle
- Rust-owned VAO, vertex buffer, index buffer and texture resources
- indexed and instanced draw submission
- static model-info, height-map, material, texture-array, water-texture and water-mask uploads
- resident opaque, alpha, LOD opaque and LOD alpha static passes uploaded once per map packet
- explicit Rust `StaticPass`, `StaticGeometryBatch` and per-map GPU resource ownership
- terrain, non-door loc and door static geometry retained and drawn by Rust
- multiple simultaneously resident map squares keyed by map-square id
- one-clear multi-map frame scheduling: opaque maps forward, transparent maps in reverse order
- Rust-owned full-detail/LOD selection with the same live distance threshold inputs
- roof-plane draw-range filtering that preserves original draw IDs
- world-entity transform and opacity parity for static map geometry
- empty alpha passes do not fall back to drawing the full index buffer
- live PicoGL texture/material/water bytes retained as synchronized CPU mirrors for zero-readback WASM upload
- revisioned TypeScript adapter for uploading those exact live global resources into Rust
- Rust-owned live actor-data `RGBA16UI` texture, backfilled on shadow startup and updated only when the existing PicoGL actor checksum/size gate changes
- Rust-owned per-map prebaked NPC packed vertex/index buffers and VAOs, mirrored from the same `SdMapData` bytes
- opt-in Rust NPC shadow draw submission for both prebaked map NPCs and current-frame dynamic fallback geometry, using the exact live TypeScript-selected animation state, actor-data offsets, world-entity transforms and deck/model offsets
- Rust-owned player shader program, reusable finalized-player geometry batch and live opaque/transparent player shadow submission, including remote slot instancing, controlled-player geometry, first-person culling and world-entity transforms
- live Rust spot-animation/GFX shadow submission through a reusable dynamic GPU batch using finalized TypeScript-selected frame geometry and actor offsets
- Rust-owned projectile shader, reusable dynamic projectile batch and live opaque/transparent projectile shadow submission, including sub-tile position offsets, model height offsets and culling state
- incremental loc and door geometry replacement for live object-state changes
- per-frame animated-loc draw-range patching without re-uploading geometry
- resident ground-item geometry with spawn, rebuild and despawn synchronization
- deterministic browser `wasm-bindgen` packaging through `yarn build:rust-renderer`
- opt-in isolated shadow runtime through `?rust-renderer=shadow`
- structural parity diagnostics for resident/visible/mirrored maps, draw-call totals, submitted indices and order-sensitive draw fingerprints
- Mode-1 overlapping world-entity ghost terrain redraw parity, including packed-HSL tint and low-opacity blending
- opt-in static pixel parity capture against an isolated PicoGL reference framebuffer
- native Rust tests, bridge tests, wasm32 compile checks, browser-WASM packaging and shadow-integration CI

The production client still presents the existing PicoGL renderer. With `?rust-renderer=shadow`, Rust runs on a detached canvas sized to the live scene render target and consumes the same decoded map data, global texture/material resources, camera matrices, fog inputs, roof state, render-distance culling, LOD decisions, animated loc state, ground items and world-entity transforms. Shadow failures are isolated and do not replace or corrupt the production rendering path.

For image comparison, use `?rust-renderer=shadow&rust-pixel-parity=1`. The first eligible static frame is captured and comparison repeats every 120 frames by default. Add `&rust-pixel-every=N` to change that interval. The latest shadow diagnostics expose structural parity plus pixel mismatch ratio, maximum channel delta, mean absolute channel delta and RMSE.

Dynamic actor classes can be added independently to the detached Rust shadow with `rust-npc-parity=1`, `rust-player-parity=1`, `rust-gfx-parity=1` and `rust-projectile-parity=1` alongside `?rust-renderer=shadow`. The flags can be combined. These modes deliberately reuse finalized TypeScript-selected simulation/animation state and packed geometry instead of duplicating game logic in Rust. Static-only pixel capture is disabled while any dynamic parity mode is enabled until the PicoGL reference capture includes the same dynamic subset.

The previously identified Mode-1 overlapping world-entity ghost redraw is now mirrored in Rust as a terrain-only blended pass with the exact packed-HSL tint and opacity contract. Stage 2 now owns the live actor-data texture and Rust GPU draw submission for prebaked NPCs, dynamic fallback NPCs, players, spot-animation/GFX and projectiles. TypeScript still owns simulation, animation-frame choice and current geometry construction; Rust consumes finalized packed geometry plus numeric renderer state. The remaining Stage 2 work is representative real-scene validation plus any cross-entity transparency/priority edge cases found by parity testing.

## Why WebGL2 first

Rust is the language migration. WebGL2 remains the graphics API for the first parity phase so we are not simultaneously changing language, GPU API, shader language and scene semantics.

That keeps the current packed vertex format and GLSL behavior usable while we compare the old and new paths. WebGPU can be introduced later as another Rust backend.

## Renderer boundary

Only POD data crosses the TypeScript/Rust boundary:

    TypeScript cache + scene decoding
                |
                | versioned numeric packets
                v
       rustosrs-renderer (WASM)
                |
                v
              WebGL2

No PicoGL object, React object, loader instance, game socket or TypeScript class graph belongs in the Rust renderer ABI.

## Port order

### Stage 1: static scene parity

Port the current main map pass: scene uniforms, model-info packets, opaque/alpha passes, roof filtering, textures, materials, height maps, water masks and current main GLSL semantics. Primary framebuffers/presentation remain a Stage 3 cutover service so static parity can be established safely on the detached shadow canvas first.

Current status: terrain, locs, doors, animated-loc range changes and ground items are mirrored live across multiple resident map squares. Structural parity compares the exact draw totals and an order-sensitive draw fingerprint. An opt-in isolated PicoGL reference framebuffer provides pixel-level comparison against the Rust shadow output.

Mode-1 overlapping world entities are included in the structural oracle and Rust now schedules the same terrain-only ghost redraw in the opaque map order.

Acceptance: identical static scene state + camera produces structurally matching draw sequences and pixel-comparable output between PicoGL and Rust across representative map, roof, LOD, water, animation and world-entity scenes.

### Stage 2: dynamic scene

Move players, NPCs, projectiles, GFX, dynamic transparency and priority ordering. Rust owns the exact live `RGBA16UI` actor-data texture plus GPU draw submission for prebaked NPCs, dynamic fallback NPCs, players, spot-animation/GFX and projectiles. The opt-in shadow flags consume the same finalized TypeScript-selected ranges/geometry, actor offsets and production opaque/transparent ordering. Player slot batching is preserved in Rust rather than expanded into per-player draws. Stage 2 is now implementation-complete at the shadow boundary; remaining work is parity acceptance and correction of any ordering/priority edge cases exposed by representative scenes.

### Stage 3: renderer services

Move renderer-owned picking data, LOD/render-distance filtering, GPU profiler counters, resize/MSAA/FXAA, texture animation and 3D overlay geometry.

### Stage 4: delete PicoGL rendering

Keep an A/B selector until parity fixtures pass. Then make Rust default and remove PicoGL scene resource ownership and DrawBackend.ts.

### Stage 5: move geometry preparation

Port VertexBuffer, SceneBuffer, model face packing, model hashing, terrain/loc mesh construction, culling and draw-list construction. At this point decoded renderer data can remain in Rust memory through GPU upload.


## Known cutover blockers

The Rust renderer must not become the primary canvas until these renderer-owned services are present and parity-tested:

- representative dynamic-scene parity acceptance across NPC/player/GFX/projectile overlap, transparency and priority cases
- GPU picking/interaction framebuffer for object, NPC, player and tile hover/click semantics
- final scene framebuffer ownership, resize/MSAA handling, FXAA and presentation/blit path
- world-space interaction/highlight and 3D overlay geometry that currently depends on the PicoGL scene pipeline
- removal of temporary GLSL duplication between the PicoGL and Rust trees, or a generated/shared shader-source path that prevents semantic drift

Player composition no longer blocks Stage 2 GPU ownership: the live Rust path consumes the finalized packed player geometry already produced by TypeScript. Equipment/identity-kit assembly, recoloring, animation transforms and other geometry preparation remain intentionally in TypeScript until Stage 5. Typed-array transfer overhead in shadow mode is accepted until `SceneBuffer`, `VertexBuffer` and face packing move into Rust memory.

## Out of scope for now

Networking, CS2, widgets, login UI, audio, pathfinding, game simulation and normal React UI are not renderer work and should not be dragged into this port.

## Later UI migration

If the long-term goal is a pure Rust web UI, Leptos is the likely framework you were thinking of. Keep that migration separate from this renderer crate. Lua is useful later as a scripting/content layer, not as the browser rendering engine.