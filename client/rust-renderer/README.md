# Rust Renderer

This crate is the pure Rust/WASM replacement for the current TypeScript/PicoGL scene renderer.

The migration is deliberately incremental. TypeScript can continue decoding cache, scene, model, animation and game state while Rust takes ownership of GPU rendering behind a narrow versioned packet boundary.

## Current scope

Stage 0 is implemented and Stage 1 static-scene parity is actively landing:

- exact 12-byte packed OSRS vertex codec
- packed-vertex deduplication
- versioned numeric TypeScript/WASM scene packets
- Rust-owned WebGL2 context and shader lifecycle
- Rust-owned VAO, vertex buffer, index buffer and texture resources
- indexed and instanced draw submission
- static model-info, height-map, material, texture-array, water-texture and water-mask uploads
- resident opaque and alpha static passes uploaded once per map packet
- Rust-owned opaque/alpha frame scheduling
- roof-plane draw-range filtering that preserves original draw IDs
- empty alpha passes do not fall back to drawing the full index buffer
- native Rust tests, bridge tests and WASM compile CI

The production client still uses the existing renderer. The next integration milestone is an A/B runtime path that can feed the same decoded map data and camera state to PicoGL and Rust without changing scene construction.

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

Port the current main map pass: scene uniforms, model-info packets, opaque/alpha passes, roof filtering, textures, materials, height maps, water masks, current main GLSL semantics, framebuffers and presentation.

Acceptance: identical scene packet + camera produces pixel-comparable output between PicoGL and Rust.

### Stage 2: dynamic scene

Move players, NPCs, projectiles, GFX, ground items, animated locs, actor data, transparency and priority ordering.

### Stage 3: renderer services

Move renderer-owned picking data, LOD/render-distance filtering, GPU profiler counters, resize/MSAA/FXAA, texture animation and 3D overlay geometry.

### Stage 4: delete PicoGL rendering

Keep an A/B selector until parity fixtures pass. Then make Rust default and remove PicoGL scene resource ownership and DrawBackend.ts.

### Stage 5: move geometry preparation

Port VertexBuffer, SceneBuffer, model face packing, model hashing, terrain/loc mesh construction, culling and draw-list construction. At this point decoded renderer data can remain in Rust memory through GPU upload.

## Out of scope for now

Networking, CS2, widgets, login UI, audio, pathfinding, game simulation and normal React UI are not renderer work and should not be dragged into this port.

## Later UI migration

If the long-term goal is a pure Rust web UI, Leptos is the likely framework you were thinking of. Keep that migration separate from this renderer crate. Lua is useful later as a scripting/content layer, not as the browser rendering engine.