import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");
const publicDir = path.join(clientDir, "public");
const rendererDir = path.join(publicDir, "rust-renderer");
const bridgeSource = readFileSync(
    path.join(clientDir, "render", "rust", "RendererPacket.ts"),
    "utf8",
);
const abiMatch = bridgeSource.match(/RUST_RENDERER_ABI_VERSION\s*=\s*(\d+)/);
if (!abiMatch) {
    throw new Error("Could not resolve the TypeScript Rust renderer ABI version");
}
const expectedAbi = Number(abiMatch[1]);

for (const required of [
    path.join(rendererDir, "rustosrs_renderer.js"),
    path.join(rendererDir, "rustosrs_renderer_bg.wasm"),
]) {
    if (!existsSync(required)) {
        throw new Error(
            "Rust renderer web package is missing. Run yarn build:rust-renderer first: " + required,
        );
    }
}

function locateChrome() {
    const candidates = [
        process.env.CHROME_PATH,
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
    ].filter(Boolean);

    for (const candidate of candidates) {
        if (candidate.includes("/") && existsSync(candidate)) {
            return candidate;
        }
        const probe = spawnSync("sh", ["-lc", "command -v " + candidate], {
            encoding: "utf8",
        });
        if (probe.status === 0 && probe.stdout.trim()) {
            return probe.stdout.trim();
        }
    }
    throw new Error(
        "Chrome/Chromium is required for the Rust WASM WebGL2 acceptance test",
    );
}

const acceptanceHtml = [
    "<!doctype html>",
    "<html><head><meta charset=\"utf-8\"><title>Rust renderer acceptance</title></head>",
    "<body><canvas id=\"scene\" width=\"80\" height=\"64\"></canvas>",
    "<script type=\"module\">",
    "const EXPECTED_ABI = __EXPECTED_ABI__;",
    "const resultUrl = '/__rust_renderer_result';",
    "function assert(condition, message) { if (!condition) throw new Error(message); }",
    "function closeTo(actual, expected, tolerance, label) {",
    "  if (Math.abs(actual - expected) > tolerance) throw new Error(label + ': expected ' + expected + ', got ' + actual);",
    "}",
    "function identity() { return new Float32Array([1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]); }",
    "function packFloat11(value) { return Math.max(0, Math.min(0x7ff, 1024 - Math.floor(value * 64 + 0.5))); }",
    "function packVertex(x, y, z, hsl, alpha, priority, textureId = -1) {",
    "  if (textureId >= 1024) textureId = -1;",
    "  const textured = textureId !== -1;",
    "  let packedHsl = hsl;",
    "  if (textured) { packedHsl &= 127; packedHsl |= (textureId & 0x1ff) << 7; }",
    "  const xPos = Math.max(0, Math.min(0x8000, x + 0x4000));",
    "  const yPos = Math.max(0, Math.min(0x8000, -y + 0x4000));",
    "  const zPos = Math.max(0, Math.min(0x8000, z + 0x4000));",
    "  const uPacked = packFloat11(0);",
    "  const vPacked = packFloat11(0);",
    "  const v0 = ((xPos << 17) | ((uPacked & 0x3f) << 11) | vPacked) >>> 0;",
    "  const v1 = (yPos | ((packedHsl & 0xffff) << 15) | (Number(textured) << 31)) >>> 0;",
    "  const v2 = ((zPos << 17) | ((alpha & 0xff) << 9) | ((priority & 7) << 6) | (((textureId >> 9) & 1) << 5) | (uPacked >>> 6)) >>> 0;",
    "  return [v0, v1, v2];",
    "}",
    "function triangleWords() {",
    "  const hsl = (10 << 10) | (4 << 7) | 80;",
    "  return new Uint32Array([",
    "    ...packVertex(-64, 0, -64, hsl, 255, 7),",
    "    ...packVertex(64, 0, -64, hsl, 255, 7),",
    "    ...packVertex(0, 0, 64, hsl, 255, 7),",
    "  ]);",
    "}",
    "function rasterTriangleWords(textureId = -1) {",
    "  const hsl = (10 << 10) | (4 << 7) | 80;",
    "  return new Uint32Array([",
    "    ...packVertex(-64, -64, -64, hsl, 255, 7, textureId),",
    "    ...packVertex(64, -64, -64, hsl, 255, 7, textureId),",
    "    ...packVertex(0, 64, -64, hsl, 255, 7, textureId),",
    "  ]);",
    "}",
    "function readCenter(gl) {",
    "  const pixel = new Uint8Array(4);",
    "  gl.readPixels(Math.floor(gl.drawingBufferWidth / 2), Math.floor(gl.drawingBufferHeight / 2), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);",
    "  return pixel;",
    "}",
    "function assertGlClean(gl, label) {",
    "  const error = gl.getError();",
    "  assert(error === gl.NO_ERROR, label + ' generated WebGL error 0x' + error.toString(16));",
    "}",
    "function eventOnce(target, name, timeoutMs = 4000, preventDefault = false) {",
    "  return new Promise((resolve, reject) => {",
    "    const timeout = setTimeout(() => reject(new Error(name + ' did not fire')), timeoutMs);",
    "    target.addEventListener(name, (event) => {",
    "      if (preventDefault) event.preventDefault();",
    "      clearTimeout(timeout); resolve(event);",
    "    }, { once: true });",
    "  });",
    "}",
    "async function report(payload) {",
    "  await fetch(resultUrl, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload) });",
    "}",
    "async function run() {",
    "  const module = await import('/rust-renderer/rustosrs_renderer.js');",
    "  await module.default();",
    "  const canvas = document.getElementById('scene');",
    "  const renderer = new module.RustWebGlRenderer(canvas);",
    "  assert(renderer.abi_version() === EXPECTED_ABI, 'ABI mismatch in packaged WASM');",
    "  const vertexBuilder = new module.RustVertexBufferBuilder();",
    "  const builderHsl = (10 << 10) | (4 << 7) | 80;",
    "  const builderInts = new Int32Array([0,0,0,builderHsl,255,-1,7, 0,0,0,builderHsl,255,-1,7]);",
    "  const builderUvs = new Float32Array([0,0, 0,0]);",
    "  const builderFlags = new Uint8Array([3,3]);",
    "  const builderIndices = vertexBuilder.push_batch(builderInts, builderUvs, builderFlags);",
    "  assert(builderIndices.length === 2 && builderIndices[0] === 0 && builderIndices[1] === 0, 'Rust vertex builder did not deduplicate batch input');",
    "  assert(vertexBuilder.vertex_count() === 1, 'Rust vertex builder vertex count mismatch');",
    "  const builderPacked = vertexBuilder.packed_vertices();",
    "  const expectedPacked = packVertex(0, 0, 0, builderHsl, 255, 7);",
    "  assert(builderPacked.length === 3 && builderPacked[0] === expectedPacked[0] && builderPacked[1] === expectedPacked[1] && builderPacked[2] === expectedPacked[2], 'Rust vertex builder packed codec mismatch');",
    "  vertexBuilder.clear();",
    "  assert(vertexBuilder.vertex_count() === 0, 'Rust vertex builder clear did not reset state');",
    "  assert(typeof module.build_model_info_texture_data === 'function', 'Rust model-info builder export missing');",
    "  const modelInfoCounts = new Uint32Array([2,1]);",
    "  const modelInfoFields = new Int32Array([",
    "    1,2,0,0,0,3,0,0,65535,",
    "    3,4,8,1,2,1,6,2,0x12345,",
    "    3,4,8,1,2,1,6,2,0x12345,",
    "  ]);",
    "  const builtModelInfo = module.build_model_info_texture_data(modelInfoCounts, modelInfoFields);",
    "  assert(builtModelInfo.length === 64, 'Rust model-info builder padding mismatch');",
    "  assert(builtModelInfo[0] === 2 && builtModelInfo[4] === 4, 'Rust model-info draw offsets mismatch');",
    "  assert(builtModelInfo[8] === 1 && builtModelInfo[9] === (2 | (3 << 14)) && builtModelInfo[11] === 65535, 'Rust model-info first instance mismatch');",
    "  assert(builtModelInfo[12] === (3 | (1 << 14)) && builtModelInfo[13] === (4 | (1 << 14)) && builtModelInfo[14] === 430 && builtModelInfo[15] === 0x2345, 'Rust model-info second instance mismatch');",
    "  const gl = canvas.getContext('webgl2');",
    "  assert(gl, 'WebGL2 context unavailable');",
    "  assertGlClean(gl, 'constructor');",
    "",
    "  const skyA = new Float32Array([0.25, 0.5, 0.75, 1]);",
    "  renderer.set_presentation_enabled(true);",
    "  renderer.set_presentation_msaa_enabled(false);",
    "  renderer.set_presentation_fxaa_enabled(false);",
    "  renderer.begin_static_frame(skyA);",
    "  renderer.present_frame();",
    "  let pixel = readCenter(gl);",
    "  closeTo(pixel[0], 64, 3, 'raw presentation red');",
    "  closeTo(pixel[1], 128, 3, 'raw presentation green');",
    "  closeTo(pixel[2], 191, 3, 'raw presentation blue');",
    "  assertGlClean(gl, 'raw presentation');",
    "",
    "  canvas.width = 96;",
    "  canvas.height = 72;",
    "  renderer.begin_static_frame(new Float32Array([0.1, 0.2, 0.3, 1]));",
    "  renderer.present_frame();",
    "  assert(gl.drawingBufferWidth === 96 && gl.drawingBufferHeight === 72, 'resize did not propagate to WebGL drawing buffer');",
    "  pixel = readCenter(gl);",
    "  closeTo(pixel[0], 26, 3, 'resized presentation red');",
    "  assertGlClean(gl, 'resize presentation');",
    "",
    "  renderer.set_presentation_msaa_enabled(true);",
    "  assert(renderer.presentation_msaa_enabled(), 'MSAA did not enable');",
    "  assert(renderer.presentation_msaa_samples() >= 1, 'MSAA target has no samples');",
    "  renderer.begin_static_frame(new Float32Array([0.2, 0.35, 0.5, 1]));",
    "  renderer.present_frame();",
    "  assertGlClean(gl, 'MSAA resolve');",
    "",
    "  renderer.set_presentation_fxaa_enabled(true);",
    "  assert(renderer.presentation_fxaa_enabled(), 'FXAA did not enable');",
    "  renderer.begin_static_frame(new Float32Array([0.4, 0.3, 0.2, 1]));",
    "  renderer.present_frame();",
    "  pixel = readCenter(gl);",
    "  closeTo(pixel[0], 102, 5, 'FXAA presentation red');",
    "  assertGlClean(gl, 'FXAA presentation');",
    "",
    "  renderer.set_presentation_msaa_enabled(false);",
    "  renderer.set_presentation_fxaa_enabled(false);",
    "  renderer.begin_static_frame(new Float32Array([0, 0, 0, 1]));",
    "  renderer.render_scene_overlay(",
    "    new Float32Array([-0.8,-0.8,0, 0.8,-0.8,0, 0,0.8,0]),",
    "    new Float32Array([1,0,0,1]), identity(), identity(), true,",
    "  );",
    "  renderer.present_frame();",
    "  pixel = readCenter(gl);",
    "  assert(pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40, 'depth-aware Rust scene overlay was not presented');",
    "  assertGlClean(gl, 'scene overlay');",
    "",
    "  renderer.select_static_map(7);",
    "  renderer.set_static_map_state(0, 0, 1, 4, 1, 0);",
    "  renderer.upload_height_map(new Int16Array(16), 4, 1);",
    "  renderer.upload_texture_array(new Uint8Array([255,255,255,255]), 1, 1, 1);",
    "  const materials = new Int8Array(24); materials[3] = 1;",
    "  renderer.upload_materials(materials, 1);",
    "  const water = new Uint8Array(5 * 4); for (let i = 0; i < 5; i++) { water[i*4] = 128; water[i*4+1] = 128; water[i*4+2] = 255; water[i*4+3] = 255; }",
    "  renderer.upload_water_textures(water, 1, 1, 5);",
    "  renderer.upload_water_mask(new Uint8Array(4 * 4 * 4), 4, 1);",
    "  renderer.upload_actor_data(new Uint16Array(16 * 2 * 4), 16, 2);",
    "  const vertices = triangleWords();",
    "  const indices = new Uint32Array([0,1,2]);",
    "  const modelInfo = new Uint16Array(64);",
    "  renderer.upload_geometry(vertices, indices);",
    "  renderer.upload_model_info(modelInfo);",
    "  renderer.set_draw_ranges(new Uint32Array([0,3,1]));",
    "  renderer.begin_static_frame(new Float32Array([0,0,0,1]));",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, new Float32Array([0,0,0,1]), new Float32Array([-1,-1,-1,0]), new Float32Array([0,0]), 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1, 'static WASM draw did not submit');",
    "  assert(renderer.last_submitted_indices() === 3, 'static WASM draw index count mismatch');",
    "  assertGlClean(gl, 'static scene draw');",
    "",    "  // Representative static-scene acceptance beyond a basic draw: roof filtering,",
    "  // LOD selection, animated-loc range patching, Mode-1 ghost redraw and water.",
    "  const acceptanceSky = new Float32Array([0,0,0,1]);",
    "  const acceptanceNoHsl = new Float32Array([-1,-1,-1,0]);",
    "  const acceptancePlayerPos = new Float32Array([0,0]);",
    "  function modelInfoPacket(planeCullLevel = 0, contourGround = 2, level = 0) {",
    "    const packet = new Uint16Array(64);",
    "    packet[0] = 1; // one draw command, first instance starts at texel 1",
    "    packet[4] = (level << 14) & 0xffff;",
    "    packet[5] = (contourGround << 14) & 0xffff;",
    "    packet[6] = (planeCullLevel << 6) & 0xffff;",
    "    packet[7] = 0xffff;",
    "    return packet;",
    "  }",
    "  const emptyU16 = new Uint16Array();",
    "  const emptyU32 = new Uint32Array();",
    "  const emptyU8 = new Uint8Array();",
    "  const staticModelInfo = modelInfoPacket(2);",
    "  renderer.upload_static_passes(staticModelInfo, new Uint32Array([0,3,1]), new Uint8Array([2]), emptyU16, emptyU32, emptyU8);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 0, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 0, 'roof filtering did not suppress plane-2 static range');",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1, 'roof filtering did not restore visible static range');",
    "  assertGlClean(gl, 'roof-plane filtering');",
    "",
    "  renderer.upload_static_lod_passes(emptyU16, emptyU32, emptyU8, emptyU16, emptyU32, emptyU8);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, true, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 0, 'LOD selection ignored empty LOD pass');",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1, 'full-detail pass was lost after LOD selection');",
    "  assertGlClean(gl, 'LOD selection');",
    "",
    "  renderer.clear_draw_ranges();",
    "  renderer.upload_aux_geometry(0, vertices, indices);",
    "  renderer.upload_aux_passes(0, modelInfoPacket(0), new Uint32Array([0,3,1]), new Uint8Array([0]), emptyU16, emptyU32, emptyU8);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1 && renderer.last_submitted_indices() === 3, 'loc auxiliary batch did not submit');",
    "  renderer.patch_aux_draw_ranges(0, false, false, new Uint32Array([0,0,0,1]));",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 0, 'animated-loc zero-range patch did not suppress draw');",
    "  renderer.patch_aux_draw_ranges(0, false, false, new Uint32Array([0,0,3,1]));",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1, 'animated-loc range patch did not restore draw');",
    "  renderer.upload_aux_geometry(0, emptyU32, emptyU32);",
    "  assertGlClean(gl, 'animated loc range patching');",
    "",
    "  renderer.upload_static_passes(modelInfoPacket(0), new Uint32Array([0,3,1]), new Uint8Array([0]), emptyU16, emptyU32, emptyU8);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  renderer.render_active_static_terrain_ghost_pass(identity(), identity(), identity(), 0.2, acceptanceSky, new Float32Array([10,4,80,1]), acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1);",
    "  assert(renderer.last_draw_calls() === 2 && renderer.last_submitted_indices() === 6, 'Mode-1 terrain ghost redraw did not submit exactly one extra terrain pass');",
    "  assertGlClean(gl, 'world-entity ghost redraw');",
    "",
    "  // Use a non-degenerate XY triangle here so identity view/projection actually rasterizes.",
    "  const rasterVertices = rasterTriangleWords(1);",
    "  renderer.upload_geometry(rasterVertices, indices);",
    "  const dryMaterials = new Int8Array(24); dryMaterials[3] = 1;",
    "  renderer.upload_materials(dryMaterials, 1);",
    "  renderer.upload_water_mask(new Uint8Array(4 * 4 * 4), 4, 1);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  renderer.present_frame();",
    "  const dryPixel = readCenter(gl);",
    "  const wetMaterials = new Int8Array(dryMaterials);",
    "  wetMaterials[5] = 1; // MATERIAL_FLAG_WATER",
    "  // Use an intentionally high-contrast, opaque red water material. Keeping",
    "  // normals/specular/duration at zero makes this fixture deterministic on",
    "  // both native GPU drivers and headless SwiftShader.",
    "  wetMaterials[8] = 255; wetMaterials[9] = 0; wetMaterials[10] = 0; wetMaterials[11] = 255;",
    "  wetMaterials[12] = 0; wetMaterials[13] = 0; wetMaterials[14] = 0; wetMaterials[15] = 0;",
    "  wetMaterials[16] = 0; wetMaterials[17] = 0; wetMaterials[18] = 0; wetMaterials[19] = 0;",
    "  renderer.upload_materials(wetMaterials, 1);",
    "  const wetMask = new Uint8Array(4 * 4 * 4);",
    "  for (let i = 0; i < 16; i++) { wetMask[i*4] = 0; wetMask[i*4+1] = 0; wetMask[i*4+2] = 255; wetMask[i*4+3] = 255; }",
    "  renderer.upload_water_mask(wetMask, 4, 1);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 0, 1, 3, false, false, 1, false);",
    "  renderer.present_frame();",
    "  const wetPixel = readCenter(gl);",
    "  const waterDelta = Math.abs(wetPixel[0]-dryPixel[0]) + Math.abs(wetPixel[1]-dryPixel[1]) + Math.abs(wetPixel[2]-dryPixel[2]);",
    "  assert(dryPixel[3] > 0 && wetPixel[3] > 0, 'water acceptance triangle did not rasterize');",
    "  assert(waterDelta > 24, 'water material/mask path did not materially change the presented static pixel');",
    "  renderer.upload_materials(dryMaterials, 1);",
    "  renderer.upload_water_mask(new Uint8Array(4 * 4 * 4), 4, 1);",
    "  assertGlClean(gl, 'water shader path');",
    "",

    "  renderer.upload_dynamic_npc_geometry(vertices, indices);",
    "  renderer.upload_dynamic_gfx_geometry(vertices, indices);",
    "  renderer.upload_dynamic_projectile_geometry(vertices, indices);",
    "  renderer.upload_dynamic_player_geometry(vertices, indices);",
    "  const sky = new Float32Array([0,0,0,1]);",
    "  const noHsl = new Float32Array([-1,-1,-1,0]);",
    "  const playerPos = new Float32Array([0,0]);",
    "  renderer.begin_static_frame(sky);",
    "  renderer.render_active_dynamic_npc_pass(identity(), identity(), identity(), 1, sky, noHsl, playerPos, 20, 15, 0, 1, false, 1, 0, 0, false);",
    "  renderer.render_active_gfx_pass(identity(), identity(), sky, noHsl, playerPos, 20, 15, 0, 1, false, 1, 0, 0, 0, 0, true, true);",
    "  renderer.render_active_projectile_pass(identity(), identity(), sky, noHsl, playerPos, 20, 15, 0, 1, false, 1, 0, 0, new Float32Array([0,0]), 0, 0, true, false);",
    "  renderer.render_active_player_pass(identity(), identity(), identity(), sky, noHsl, playerPos, 20, 15, 0, 1, false, 1, 0, new Int32Array(), 0, false, true, true);",
    "  renderer.render_active_player_pass(identity(), identity(), identity(), sky, noHsl, playerPos, 20, 15, 0, 1, false, 1, 0, new Int32Array([0,1]), 0, true, false, true);",
    "  assert(renderer.last_draw_calls() === 5, 'full dynamic WASM smoke draw count mismatch');",
    "  assert(renderer.last_submitted_indices() === 18, 'full dynamic WASM smoke index count mismatch');",
    "  renderer.present_frame();",
    "  assertGlClean(gl, 'full dynamic scene');",
    "",
    "  const loseContext = gl.getExtension('WEBGL_lose_context');",
    "  assert(loseContext, 'WEBGL_lose_context unavailable for forced recovery acceptance');",
    "  const lost = eventOnce(canvas, 'webglcontextlost', 4000, true);",
    "  loseContext.loseContext();",
    "  await lost;",
    "",
    "  // Production recovery replaces the lost Rust canvas/context instead of",
    "  // depending on webglcontextrestored, which headless SwiftShader may never emit.",
    "  renderer.dispose();",
    "  const recoveryCanvas = document.createElement('canvas');",
    "  recoveryCanvas.width = 64; recoveryCanvas.height = 64; document.body.appendChild(recoveryCanvas);",
    "  const recovered = new module.RustWebGlRenderer(recoveryCanvas);",
    "  assert(recovered.abi_version() === EXPECTED_ABI, 'renderer could not be recreated after forced context loss');",
    "  recovered.set_presentation_enabled(true);",
    "  recovered.begin_static_frame(new Float32Array([0.15,0.25,0.35,1]));",
    "  recovered.present_frame();",
    "  const recoveryGl = recoveryCanvas.getContext('webgl2');",
    "  assertGlClean(recoveryGl, 'context recovery renderer');",
    "  recovered.dispose();",
    "",
    "  await report({ ok: true, abi: EXPECTED_ABI, dynamicDrawCalls: 5, contextRecovery: true });",
    "}",
    "run().catch(async (error) => {",
    "  try { await report({ ok: false, error: error && (error.stack || error.message) ? String(error.stack || error.message) : String(error) }); } catch {}",
    "});",
    "</script></body></html>",
].join("\n").replace("__EXPECTED_ABI__", String(expectedAbi));

function mimeType(filePath) {
    if (filePath.endsWith(".js")) return "text/javascript; charset=utf-8";
    if (filePath.endsWith(".wasm")) return "application/wasm";
    if (filePath.endsWith(".html")) return "text/html; charset=utf-8";
    return "application/octet-stream";
}

let resolveResult;
const resultPromise = new Promise((resolve) => {
    resolveResult = resolve;
});

const server = createServer((request, response) => {
    const urlPath = decodeURIComponent((request.url || "/").split("?")[0]);

    if (request.method === "POST" && urlPath === "/__rust_renderer_result") {
        let body = "";
        request.setEncoding("utf8");
        request.on("data", (chunk) => {
            body += chunk;
            if (body.length > 1024 * 1024) request.destroy();
        });
        request.on("end", () => {
            try {
                resolveResult(JSON.parse(body));
                response.writeHead(204);
                response.end();
            } catch (error) {
                resolveResult({ ok: false, error: "Invalid browser result: " + error });
                response.writeHead(400);
                response.end();
            }
        });
        return;
    }

    if (request.method === "GET" && (urlPath === "/" || urlPath === "/acceptance.html")) {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
        response.end(acceptanceHtml);
        return;
    }

    if (request.method === "GET" && urlPath.startsWith("/rust-renderer/")) {
        const relative = urlPath.slice(1);
        const filePath = path.resolve(publicDir, relative);
        if (!filePath.startsWith(path.resolve(rendererDir) + path.sep) || !existsSync(filePath) || !statSync(filePath).isFile()) {
            response.writeHead(404);
            response.end();
            return;
        }
        response.writeHead(200, {
            "content-type": mimeType(filePath),
            "cache-control": "no-store",
            "cross-origin-opener-policy": "same-origin",
            "cross-origin-embedder-policy": "require-corp",
        });
        response.end(readFileSync(filePath));
        return;
    }

    response.writeHead(404);
    response.end();
});

await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
    server.close();
    throw new Error("Could not determine acceptance server address");
}

const chromePath = locateChrome();
const targetUrl = "http://127.0.0.1:" + address.port + "/acceptance.html";
const chrome = spawn(chromePath, [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--disable-gpu-sandbox",
    "--enable-webgl",
    "--ignore-gpu-blocklist",
    "--enable-unsafe-swiftshader",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--window-size=800,600",
    targetUrl,
], {
    stdio: ["ignore", "pipe", "pipe"],
});

let chromeOutput = "";
for (const stream of [chrome.stdout, chrome.stderr]) {
    stream.on("data", (chunk) => {
        chromeOutput += chunk.toString();
        if (chromeOutput.length > 100000) chromeOutput = chromeOutput.slice(-100000);
    });
}

const earlyExit = new Promise((resolve) => {
    chrome.once("exit", (code, signal) => {
        resolve({
            ok: false,
            error: "Chrome exited before reporting acceptance result (code=" + code + ", signal=" + signal + ")",
        });
    });
});

const timeout = new Promise((resolve) => {
    setTimeout(() => resolve({
        ok: false,
        error: "Timed out waiting for Rust WASM/WebGL2 browser acceptance",
    }), 45000);
});

const result = await Promise.race([resultPromise, earlyExit, timeout]);

try {
    chrome.kill("SIGTERM");
} catch {}
await new Promise((resolve) => server.close(resolve));

if (!result || result.ok !== true) {
    if (chromeOutput.trim()) {
        console.error(chromeOutput.trim());
    }
    throw new Error(result?.error || "Rust WASM/WebGL2 browser acceptance failed");
}

console.log(
    "Rust WASM/WebGL2 browser acceptance passed"
    + " (ABI " + result.abi
    + ", dynamic draws " + result.dynamicDrawCalls
    + ", context recovery " + result.contextRecovery + ")",
);
