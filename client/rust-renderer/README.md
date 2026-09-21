# Rust Renderer

This crate is the pure Rust/WASM replacement for the current TypeScript/PicoGL scene renderer.

The migration is deliberately incremental. TypeScript can continue decoding cache, scene, model, animation and game state while Rust takes ownership of GPU rendering behind a narrow versioned packet boundary.

## Current scope

Stages 0-2 are implemented at the parity boundary, Stage 3 primary presentation is implemented, and Rust primary is now the default runtime:

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
- Rust-owned presentation framebuffer with resize-aware RGBA8/depth attachments
- optional Rust MSAA using the same device `MAX_SAMPLES` policy as the PicoGL scene framebuffer
- Rust FXAA/fullscreen presentation pass and raw resolved blit path
- default Rust-primary mode where Rust owns the visible 3D canvas and all offscreen scene/presentation framebuffer targets; PicoGL retains only its transparent default compatibility/input/UI canvas, while `?rust-renderer=off` explicitly selects the legacy Pico path
- scene-space Rust overlay/highlight primitive used by the parity/primary path
- WebGL context-loss recovery that replaces the lost Rust canvas/context immediately and replays retained static maps, global resources, ground/loc/door state and live actor data without depending on `webglcontextrestored`
- primary-mode PicoGL scene submissions suppressed across the central static/NPC draw helper plus player, GFX and projectile direct draw paths
- source-level Rust/TypeScript ABI alignment check in CI
- native Rust tests, bridge tests, wasm32 compile checks, browser-WASM packaging, headless WebGL2/WASM acceptance and shadow-integration CI

Rust-primary is now the default. Rust owns the visible 3D scene/presentation canvas and all offscreen scene/presentation framebuffer targets while PicoGL is retained only as the transparent compatibility/input/UI canvas. `?rust-renderer=shadow` runs Rust on a detached canvas for parity work, and `?rust-renderer=off` explicitly selects the legacy Pico scene renderer. Primary mode suppresses legacy PicoGL 3D GPU submission while still running the TypeScript traversal needed to finalize the numeric data consumed by Rust. On Rust context loss, the client immediately creates a fresh Rust canvas/context and replays retained renderer state instead of waiting for the lost context to restore.

For image comparison, use `?rust-renderer=shadow&rust-pixel-parity=1`. The first eligible static frame is captured and comparison repeats every 120 frames by default. Add `&rust-pixel-every=N` to change that interval. The latest shadow diagnostics expose structural parity plus pixel mismatch ratio, maximum channel delta, mean absolute channel delta and RMSE.

Dynamic actor classes can be added independently to the detached Rust shadow with `rust-npc-parity=1`, `rust-player-parity=1`, `rust-gfx-parity=1` and `rust-projectile-parity=1` alongside `?rust-renderer=shadow`. The flags can be combined. These modes deliberately reuse finalized TypeScript-selected simulation/animation state and packed geometry instead of duplicating game logic in Rust. Partial dynamic parity combinations use structural diagnostics only. When all four dynamic flags are enabled together with `rust-pixel-parity=1`, the client resolves the completed PicoGL scene before overlays/post-processing and compares it directly with the detached Rust scene, including the MSAA scene path.

The previously identified Mode-1 overlapping world-entity ghost redraw is now mirrored in Rust as a terrain-only blended pass with the exact packed-HSL tint and opacity contract. Stage 2 is complete at the renderer-ownership/parity boundary: Rust owns the live actor-data texture and GPU draw submission for prebaked NPCs, dynamic fallback NPCs, players, spot-animation/GFX and projectiles. TypeScript still owns simulation, animation-frame choice and current geometry construction; Rust consumes finalized packed geometry plus numeric renderer state. Representative crowded-scene captures now belong to the pre-cutover acceptance gate rather than Stage 2 implementation.

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

### Stage 2: dynamic scene — COMPLETE

Rust owns the exact live `RGBA16UI` actor-data texture plus GPU draw submission for prebaked NPCs, dynamic fallback NPCs, players, spot-animation/GFX and projectiles. The opt-in shadow flags consume the same finalized TypeScript-selected ranges/geometry, actor offsets and production opaque/transparent ordering. Player slot batching is preserved in Rust rather than expanded into per-player draws. Structural parity is available for any individual dynamic class, full-dynamic structural parity has a single combined acceptance signal, and full dynamic pixel parity is available when NPC, player, GFX and projectile parity flags are all enabled together.

Representative overlap/transparency/priority captures remain required before the default renderer cutover, but they are now tracked as pre-cutover acceptance rather than unfinished Stage 2 implementation. Any mismatch found there is a parity defect to correct, not a missing renderer class.

### Stage 3: renderer services

Primary presentation is now the default: Rust owns an offscreen color/depth scene target, optional MSAA resolve, FXAA/raw presentation, resize handling, the visible 3D canvas and a depth-aware scene-overlay primitive. The runtime also recreates itself after WebGL context loss and replays retained renderer state. World interaction remains CPU-side through `SceneRaycaster`, so a Rust GPU picking framebuffer is not a cutover requirement for the current client.

The automated Stage 3 browser gate now covers raw presentation, resize, Rust-owned MSAA, FXAA, scene overlays, dynamic draws and forced WebGL context-loss/fresh-context recovery. Representative in-game scene testing remains useful for visual acceptance, but the renderer-service implementation and automated browser acceptance are complete. GPU profiling can move later and does not block visual cutover.

### Stage 4: default Rust + remove PicoGL scene ownership

The A/B selector and an opt-in Rust-primary path already exist. Static/NPC draws routed through `host.draw()` and direct player/GFX/projectile submissions are suppressed on the Pico context while primary mode is active, so Rust is the only 3D GPU submitter in that mode.

Rust-primary is now the default. `DrawBackend.ts` is removed, and primary mode no longer allocates PicoGL scene/MSAA/frame-texture framebuffer targets or Pico FXAA presentation state. Static terrain/loc/door/ground draw ranges are CPU-authoritative; their legacy Pico draw calls, VAOs, vertex/index buffers and model-info textures are deferred until the legacy path is actually materialized. Prebaked NPC Pico resources are likewise deferred, while dynamic NPC, player, GFX and projectile Pico GPU uploads are skipped in Rust-primary and guarded at the low-level upload/cache boundary. Legacy per-map height/water textures are also lazy and are materialized only if the Pico fallback path is entered. Normal Rust-primary operation therefore avoids Pico scene framebuffers, presentation state, draw calls, scene VAOs/buffers/model-info textures, dynamic actor uploads and per-map scene textures. CI now enforces that ownership contract, and the shadow-integration suite exercises the real terrain/loc/door/ground fallback accessors to verify deferred draw calls materialize exactly once when the legacy path is entered. The remaining PicoGL scene programs are intentionally compiled at startup as instant-fallback insurance; making them lazy would turn renderer recovery into an asynchronous shader-compilation event, so they remain compatibility infrastructure until the legacy renderer is retired. Stage 4 implementation is complete at that boundary.

### Stage 5: move geometry preparation

Port VertexBuffer, SceneBuffer, model face packing, model hashing, terrain/loc mesh construction, culling and draw-list construction. At this point decoded renderer data can remain in Rust memory through GPU upload.


## Remaining cutover blockers

Renderer ownership through Stage 4 is implemented. The remaining pre-merge gate is representative visual acceptance rather than another missing renderer class:

- representative full-dynamic parity acceptance across NPC/player/GFX/projectile overlap, transparency, priorities, world entities, water, roofs and animated loc changes
- representative in-game `rust-renderer=primary` visual acceptance across crowded actor overlap, world entities, water, roofs and animated loc changes
- eliminate temporary GLSL semantic duplication with shared/generated shader sources or an equivalent drift-proof build contract as follow-up hardening
- later move `SceneBuffer`, `VertexBuffer`, face packing and dynamic geometry construction into Rust/WASM memory during Stage 5 to remove JS-to-WASM typed-array churn

The automated fallback smoke contract is now covered: deferred terrain/loc/door/ground Pico draw calls remain unmaterialized during primary operation and materialize exactly once through their real legacy accessors. Eager compilation of the fallback-only Pico scene programs is an intentional recovery policy, not an unfinished Stage 4 task.

Player composition no longer blocks GPU cutover: Rust consumes finalized packed player geometry already produced by TypeScript. Equipment/identity-kit assembly, recoloring and animation transforms remain intentionally in TypeScript until Stage 5. World interaction is already CPU-side, so a GPU picking framebuffer is not required for the current cutover.

## Out of scope for now

Networking, CS2, widgets, login UI, audio, pathfinding, game simulation and normal React UI are not renderer work and should not be dragged into this port.

## Later UI migration

If the long-term goal is a pure Rust web UI, Leptos is the likely framework you were thinking of. Keep that migration separate from this renderer crate. Lua is useful later as a scripting/content layer, not as the browser rendering engine.