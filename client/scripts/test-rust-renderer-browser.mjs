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
    "function rsTrig(angle) {",
    "  const radians = angle * ((360 / 2048) * (Math.PI / 180));",
    "  return [(65536 * Math.sin(radians)) | 0, (65536 * Math.cos(radians)) | 0];",
    "}",
    "function referenceBasicRotate(x, y, z, angle) {",
    "  const [sin, cos] = rsTrig(angle);",
    "  const nextX = (sin * z + cos * x) >> 16;",
    "  const nextZ = (cos * z - sin * x) >> 16;",
    "  return [nextX, y, nextZ];",
    "}",
    "function referenceLegacyRotate(x, y, z, tx, ty, tz, ox, oy, oz) {",
    "  let vx = x - ox; let vy = y - oy; let vz = z - oz;",
    "  const angleX = (tx & 255) * 8; const angleY = (ty & 255) * 8; const angleZ = (tz & 255) * 8;",
    "  if (angleZ !== 0) { const [sin, cos] = rsTrig(angleZ); const temp = (sin * vy + cos * vx) >> 16; vy = (cos * vy - sin * vx) >> 16; vx = temp; }",
    "  if (angleX !== 0) { const [sin, cos] = rsTrig(angleX); const temp = (cos * vy - sin * vz) >> 16; vz = (sin * vy + cos * vz) >> 16; vy = temp; }",
    "  if (angleY !== 0) { const [sin, cos] = rsTrig(angleY); const temp = (sin * vz + cos * vx) >> 16; vz = (cos * vz - sin * vx) >> 16; vx = temp; }",
    "  return [(vx + ox) | 0, (vy + oy) | 0, (vz + oz) | 0];",
    "}",
    "function sampleContourHeight(heights, width, depth, vx, vz) {",
    "  const rx = vx & 127; const rz = vz & 127; const tx = vx >> 7; const tz = vz >> 7;",
    "  if (tx < 0 || tz < 0 || tx + 1 >= width || tz + 1 >= depth) return undefined;",
    "  const at = (x, z) => heights[x * depth + z];",
    "  const h0 = (at(tx,tz) * (128-rx) + at(tx+1,tz) * rx) >> 7;",
    "  const h1 = (at(tx,tz+1) * (128-rx) + at(tx+1,tz+1) * rx) >> 7;",
    "  return (h0 * (128-rz) + h1 * rz) >> 7;",
    "}",
    "function referenceContour(type, param, xs, ys, zs, used, base, bw, bd, above, aw, ad, sceneX, sceneHeight, sceneZ, denominator, minY, maxY) {",
    "  const out = new Int32Array(xs.length); const deltaY = maxY - minY;",
    "  for (let i = 0; i < xs.length; i++) {",
    "    const isUsed = i < used; const vx = (xs[i] + sceneX) | 0; const vz = (zs[i] + sceneZ) | 0;",
    "    const baseHeight = sampleContourHeight(base,bw,bd,vx,vz); const aboveHeight = sampleContourHeight(above,aw,ad,vx,vz);",
    "    if (type === 1) { if (baseHeight !== undefined) out[i] = ys[i] + baseHeight - sceneHeight; }",
    "    else if (type === 2) {",
    "      const yRatio = denominator === 0 ? 0 : (((ys[i] << 16) / denominator) | 0);",
    "      if (yRatio < param) { if (baseHeight !== undefined && param !== 0) out[i] = ys[i] + ((baseHeight-sceneHeight) * (param-yRatio)) / param; else if (param === 0) out[i] = ys[i]; } else out[i] = ys[i];",
    "    }",
    "    else if (type === 3) { if (!isUsed) continue; if (baseHeight === undefined) { out[i] = ys[i]; continue; } let delta = baseHeight-sceneHeight; const limit = Math.abs(param); if (limit > 0) delta = Math.max(-limit, Math.min(limit, delta)); out[i] = ys[i] + delta; }",
    "    else if (type === 4) { if (!isUsed) continue; if (aboveHeight !== undefined) out[i] = ys[i] + aboveHeight - sceneHeight + deltaY; }",
    "    else if (type === 5) { if (!isUsed) continue; if (baseHeight !== undefined && aboveHeight !== undefined) { const deltaHeight = baseHeight - aboveHeight; out[i] = (((((ys[i] << 8) / deltaY) | 0) * deltaHeight) >> 8) - (sceneHeight - baseHeight)); } }",
    "  }",
    "  return out;",
    "}",
    "function assertF32BitsEqual(actual, expected, label) {",
    "  assert(actual.length === expected.length, label + ': length mismatch');",
    "  const a = new Uint32Array(actual.buffer, actual.byteOffset, actual.length); const e = new Uint32Array(expected.buffer, expected.byteOffset, expected.length);",
    "  for (let i=0;i<a.length;i++) assert(a[i] === e[i], label + ': f32 bit mismatch at ' + i + ', 0x' + a[i].toString(16) + ' vs 0x' + e[i].toString(16));",
    "}",
    "function uvMatrix(p,m,n,rotation,scaleX,scaleY,scaleZ) {",
    "  const fs = new Float32Array(9); let a=1, b=0, c=m/32767, d=-Math.sqrt(1-c*c), e=1-c; const len=Math.sqrt(p*p+n*n); if(len!==0){a=-n/len;b=p/len;}",
    "  fs[0]=c+a*a*e; fs[1]=b*d; fs[2]=b*a*e; fs[3]=-b*d; fs[4]=c; fs[5]=a*d; fs[6]=a*b*e; fs[7]=-a*d; fs[8]=c+b*b*e;",
    "  const r=new Float32Array(9); c=Math.cos(rotation*0.024543693); d=Math.sin(rotation*0.024543693); r[0]=c;r[1]=0;r[2]=d;r[3]=0;r[4]=1;r[5]=0;r[6]=-d;r[7]=0;r[8]=c;",
    "  const o=new Float32Array(9);",
    "  o[0]=r[0]*fs[0]+r[1]*fs[3]+r[2]*fs[6]; o[1]=r[0]*fs[1]+r[1]*fs[4]+r[2]*fs[7]; o[2]=r[0]*fs[2]+r[1]*fs[5]+r[2]*fs[8];",
    "  o[3]=r[3]*fs[0]+r[4]*fs[3]+r[5]*fs[6]; o[4]=r[3]*fs[1]+r[4]*fs[4]+r[5]*fs[7]; o[5]=r[3]*fs[2]+r[4]*fs[5]+r[5]*fs[8];",
    "  o[6]=r[6]*fs[0]+r[7]*fs[3]+r[8]*fs[6]; o[7]=r[6]*fs[1]+r[7]*fs[4]+r[8]*fs[7]; o[8]=r[6]*fs[2]+r[7]*fs[5]+r[8]*fs[8];",
    "  o[0]*=scaleX;o[1]*=scaleX;o[2]*=scaleX;o[3]*=scaleY;o[4]*=scaleY;o[5]*=scaleY;o[6]*=scaleZ;o[7]*=scaleZ;o[8]*=scaleZ; return o;",
    "}",
    "function uvDirection(u,v,direction) { if(direction===1){const t=u;u=-v;v=t;} else if(direction===2){u=-u;v=-v;} else if(direction===3){const t=u;u=v;v=-t;} const out=new Float32Array(2);out[0]=u;out[1]=v;return [out[0],out[1]]; }",
    "function uvCyl(vertex,center,scales,scaleZ,direction,speed) { const x=vertex[0]-center[0],y=vertex[1]-center[1],z=vertex[2]-center[2]; const a=x*scales[0]+y*scales[1]+z*scales[2],b=x*scales[3]+y*scales[4]+z*scales[5],c=x*scales[6]+y*scales[7]+z*scales[8]; let u=Math.atan2(a,c)/6.2831855+0.5;if(scaleZ!==1)u*=scaleZ;return uvDirection(u,b+0.5+speed,direction); }",
    "function uvDominant(a,b,c) { const aa=Math.abs(a),bb=Math.abs(b),cc=Math.abs(c); if(bb>aa&&bb>cc)return b>0?0:1;if(cc>aa&&cc>bb)return c>0?2:3;return a>0?4:5; }",
    "function uvPlanar(vertex,center,type,scales,direction,speed,uOffset,vOffset) { const x=vertex[0]-center[0],y=vertex[1]-center[1],z=vertex[2]-center[2]; const a=x*scales[0]+y*scales[1]+z*scales[2],b=x*scales[3]+y*scales[4]+z*scales[5],c=x*scales[6]+y*scales[7]+z*scales[8]; let u,v; if(type===0){u=a+speed+0.5;v=-c+vOffset+0.5;}else if(type===1){u=a+speed+0.5;v=c+vOffset+0.5;}else if(type===2){u=-a+speed+0.5;v=-b+uOffset+0.5;}else if(type===3){u=a+speed+0.5;v=-b+uOffset+0.5;}else if(type===4){u=c+vOffset+0.5;v=-b+uOffset+0.5;}else{u=-c+vOffset+0.5;v=-b+uOffset+0.5;} return uvDirection(u,v,direction); }",
    "function uvSphere(vertex,center,scales,direction,speed) { const x=vertex[0]-center[0],y=vertex[1]-center[1],z=vertex[2]-center[2]; const a=x*scales[0]+y*scales[1]+z*scales[2],b=x*scales[3]+y*scales[4]+z*scales[5],c=x*scales[6]+y*scales[7]+z*scales[8]; const len=Math.sqrt(a*a+b*b+c*c); return uvDirection(Math.atan2(a,c)/6.2831855+0.5,Math.asin(b/len)/3.1415927+0.5+speed,direction); }",
    "function referenceComplexUv(type,direction) {",
    "  const verts=[[0,0,0],[128,64,32],[-64,96,160]]; const center=[32,48,80]; const sx=512,sy=256,sz=768,p=10000,m=12000,n=-8000,rotation=33,speed=64/256;",
    "  let mx,my,mz;if(type===1){my=64/sy;if(sx===0){mx=1;mz=1;}else if(sx<=0){mx=-sx/1024;mz=1;}else{mx=1;mz=sx/1024;}}else if(type===2){mx=64/sx;my=64/sy;mz=64/sz;}else{mx=sx/1024;my=sy/1024;mz=sz/1024;}",
    "  const scales=uvMatrix(p,m,n,rotation,mx,my,mz); let pairs;",
    "  if(type===1){ const scaleZ=sz/1024;pairs=verts.map(v=>uvCyl(v,center,scales,scaleZ,direction,speed));const half=scaleZ/2;if((direction&1)===0){if(pairs[1][0]-pairs[0][0]>half)pairs[1][0]-=scaleZ;else if(pairs[0][0]-pairs[1][0]>half)pairs[1][0]+=scaleZ;if(pairs[2][0]-pairs[0][0]>half)pairs[2][0]-=scaleZ;else if(pairs[0][0]-pairs[2][0]>half)pairs[2][0]+=scaleZ;}else{if(pairs[1][1]-pairs[0][1]>half)pairs[1][1]-=scaleZ;else if(pairs[0][1]-pairs[1][1]>half)pairs[1][1]+=scaleZ;if(pairs[2][1]-pairs[0][1]>half)pairs[2][1]-=scaleZ;else if(pairs[0][1]-pairs[2][1]>half)pairs[2][1]+=scaleZ;} }",
    "  else if(type===2){ const v0=verts[0],v1=verts[1],v2=verts[2],dx1=v1[0]-v0[0],dy1=v1[1]-v0[1],dz1=v1[2]-v0[2],dx2=v2[0]-v0[0],dy2=v2[1]-v0[1],dz2=v2[2]-v0[2],vx=dy1*dz2-dy2*dz1,vy=dz1*dx2-dz2*dx1,vz=dx1*dy2-dx2*dy1; const st=uvDominant((vx*scales[0]+vy*scales[1]+vz*scales[2])/(64/sx),(vx*scales[3]+vy*scales[4]+vz*scales[5])/(64/sy),(vx*scales[6]+vy*scales[7]+vz*scales[8])/(64/sz)); pairs=verts.map(v=>uvPlanar(v,center,st,scales,direction,speed,32/256,-48/256)); }",
    "  else { pairs=verts.map(v=>uvSphere(v,center,scales,direction,speed));if((direction&1)===0){if(pairs[1][0]-pairs[0][0]>0.5)pairs[1][0]--;else if(pairs[0][0]-pairs[1][0]>0)pairs[1][0]++;if(pairs[2][0]-pairs[0][0]>0.5)pairs[2][0]--;else if(pairs[0][0]-pairs[2][0]>0.5)pairs[2][0]++;}else{if(pairs[1][1]-pairs[0][1]>0.5)pairs[1][1]--;else if(pairs[0][1]-pairs[1][1]>0.5)pairs[1][1]++;if(pairs[2][1]-pairs[0][1]>0.5)pairs[2][1]--;else if(pairs[0][1]-pairs[2][1]>0.5)pairs[2][1]++;}}",
    "  return new Float32Array([pairs[0][0],pairs[0][1],pairs[1][0],pairs[1][1],pairs[2][0],pairs[2][1]]);",
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
    "  assert(typeof vertexBuilder.push_model_faces === 'function', 'Rust direct model builder method missing');",
    "  const directModelBuilder = new module.RustVertexBufferBuilder();",
    "  assert(typeof directModelBuilder.set_texture_id_map === 'function', 'Rust texture map setter missing');",
    "  assert(typeof directModelBuilder.used_texture_ids === 'function', 'Rust used-texture query missing');",
    "  directModelBuilder.set_texture_id_map(new Int32Array([7]), new Int32Array([3]));",
    "  const directModelIndices = directModelBuilder.push_model_faces(",
    "    new Int32Array([0,128,0]), new Int32Array([0,0,128]), new Int32Array([0,0,0]),",
    "    new Int32Array([0]), new Int32Array([1]), new Int32Array([2]),",
    "    new Int32Array([0x1234]), new Int32Array([0x2345]), new Int32Array([-1]),",
    "    new Float32Array(), new Int32Array([0,255,11,-1,7]),",
    "    10,20,30, 0,0,0,0, true,",
    "  );",
    "  assert(JSON.stringify(Array.from(directModelIndices)) === JSON.stringify([0,1,2]), 'Rust direct model builder index output mismatch');",
    "  assert(directModelBuilder.vertex_count() === 3, 'Rust direct model builder vertex count mismatch');",
    "  assert(JSON.stringify(Array.from(directModelBuilder.used_texture_ids())) === JSON.stringify([7]), 'Rust model texture residency tracking mismatch');",
    "  assert(typeof directModelBuilder.push_terrain_tile === 'function', 'Rust direct terrain builder method missing');",
    "  const directTerrainBuilder = new module.RustVertexBufferBuilder();",
    "  directTerrainBuilder.set_texture_id_map(new Int32Array([7]), new Int32Array([3]));",
    "  const directTerrainIndices = directTerrainBuilder.push_terrain_tile(",
    "    new Int32Array([128,256,128]), new Int32Array([0,0,0]), new Int32Array([256,256,384]),",
    "    new Int32Array([0]), new Int32Array([1]), new Int32Array([2]),",
    "    new Int32Array([0x1234]), new Int32Array([0x2345]), new Int32Array([0x3456]),",
    "    new Int32Array([7]), 128,256, -128,-256,",
    "  );",
    "  assert(JSON.stringify(Array.from(directTerrainIndices)) === JSON.stringify([0,1,2]), 'Rust direct terrain builder index output mismatch');",
    "  assert(directTerrainBuilder.vertex_count() === 3, 'Rust direct terrain builder vertex count mismatch');",
    "  assert(JSON.stringify(Array.from(directTerrainBuilder.used_texture_ids())) === JSON.stringify([7]), 'Rust terrain texture residency tracking mismatch');",
    "  assert(typeof module.skin_skeletal_vertices === 'function', 'Rust skeletal skinning export missing');",
    "  const skeletalMatrix = new Float32Array([1,0,0,0, 0,1,0,0, 0,0,1,0, 5,7,-9,1]);",
    "  const skeletalResult = module.skin_skeletal_vertices(",
    "    new Int32Array([10]), new Int32Array([-20]), new Int32Array([30]),",
    "    new Uint32Array([0,1]), new Int32Array([0]), new Int32Array([255]), skeletalMatrix,",
    "  );",
    "  assert(JSON.stringify(Array.from(skeletalResult)) === JSON.stringify([15,-27,39]), 'Rust skeletal skinning axis/translation mismatch');",
    "  assert(typeof module.apply_legacy_transforms === 'function', 'Rust legacy transform export missing');",
    "  const legacyResult = module.apply_legacy_transforms(",
    "    new Int32Array([10,30]), new Int32Array([20,40]), new Int32Array([0,0]),",
    "    new Int8Array([0]), new Uint16Array([0x1234]),",
    "    new Uint32Array([0,2]), new Int32Array([0,1]),",
    "    new Uint32Array([0,1]), new Int32Array([0]),",
    "    new Int32Array([0,0,0,0, 3,256,128,128, 1,5,-3,7, 5,2,0,0]),",
    "    new Uint32Array([0,1,2,3,4]), new Int32Array([0,0,0,0]),",
    "    0,0,0,",
    "  );",
    "  assert(JSON.stringify(Array.from(legacyResult.slice(0,14))) === JSON.stringify([20,30,0,0,2,1,1, 5,17,7,45,37,7,16]), 'Rust legacy transform batch mismatch');",
    "  assert(typeof module.contour_vertices_y === 'function', 'Rust contour transform export missing');",
    "  const contourResult = module.contour_vertices_y(",
    "    new Int32Array([64]), new Int32Array([10]), new Int32Array([64]), 1, 1, 0,",
    "    new Int32Array([0,128,128,256]), 2,2, new Int32Array(), 0,0,",
    "    0,0,0, -1,-20,20, true,",
    "  );",
    "  assert(JSON.stringify(Array.from(contourResult)) === JSON.stringify([138]), 'Rust contour transform mismatch');",
    "  assert(typeof module.compute_model_uvs === 'function', 'Rust model UV export missing');",
    "  const uvResult = module.compute_model_uvs(",
    "    new Int32Array([0,128,0]), new Int32Array([0,0,128]), new Int32Array([0,0,0]),",
    "    new Int32Array([0]), new Int32Array([1]), new Int32Array([2]),",
    "    new Int16Array([7]), new Int8Array([-1]), new Int8Array(),",
    "    new Int16Array(), new Int16Array(), new Int16Array(),",
    "    new Int32Array(), new Int32Array(), new Int32Array(),",
    "    new Int8Array(), new Int8Array(), new Int32Array(), new Int32Array(), new Int32Array(),",
    "  );",
    "  assert(JSON.stringify(Array.from(uvResult)) === JSON.stringify([0,0,1,0,0,1]), 'Rust type-0 UV mapping mismatch');",
    "  for (const type of [1,2,3]) {",
    "    for (const direction of [0,1,2,3]) {",
    "      const rust = module.compute_model_uvs(",
    "        new Int32Array([0,128,-64]), new Int32Array([0,64,96]), new Int32Array([0,32,160]),",
    "        new Int32Array([0]), new Int32Array([1]), new Int32Array([2]), new Int16Array([7]), new Int8Array([0]), new Int8Array([type]),",
    "        new Int16Array([10000]), new Int16Array([12000]), new Int16Array([-8000]),",
    "        new Int32Array([512]), new Int32Array([256]), new Int32Array([768]),",
    "        new Int8Array([33]), new Int8Array([direction]), new Int32Array([64]), new Int32Array([32]), new Int32Array([-48]),",
    "      );",
    "      assertF32BitsEqual(rust, referenceComplexUv(type,direction), 'Rust UV parity type ' + type + ' direction ' + direction);",
    "    }",
    "  }",
    "  assert(typeof module.transform_vertices_basic === 'function', 'Rust basic transform export missing');",
    "  const basicTransformResult = module.transform_vertices_basic(",
    "    new Int32Array([10]), new Int32Array([20]), new Int32Array([30]), 4, 5,-3,7,",
    "  );",
    "  assert(JSON.stringify(Array.from(basicTransformResult)) === JSON.stringify([15,17,37]), 'Rust basic transform mismatch');",
    "  const basicRotationAngles = []; for (let angle = 0; angle < 2048; angle += 32) basicRotationAngles.push(angle); basicRotationAngles.push(2047);",
    "  for (const angle of basicRotationAngles) {",
    "    const rust = module.transform_vertices_basic(new Int32Array([1234]), new Int32Array([-567]), new Int32Array([891]), 3, angle,0,0);",
    "    const expected = referenceBasicRotate(1234,-567,891,angle);",
    "    assert(JSON.stringify(Array.from(rust)) === JSON.stringify(expected), 'Rust basic rotation parity mismatch at angle ' + angle);",
    "  }",
    "  for (const t of [0,1,17,31,64,95,127,128,159,191,223,255]) {",
    "    const ty = (t * 3) & 255; const tz = (t * 5) & 255;",
    "    const rust = module.apply_legacy_transforms(",
    "      new Int32Array([123]), new Int32Array([-45]), new Int32Array([67]),",
    "      new Int8Array(), new Uint16Array(),",
    "      new Uint32Array([0,1]), new Int32Array([0]),",
    "      new Uint32Array([0]), new Int32Array(),",
    "      new Int32Array([2,t,ty,tz]), new Uint32Array([0,1]), new Int32Array([0]),",
    "      11,-7,5,",
    "    );",
    "    const expected = referenceLegacyRotate(123,-45,67,t,ty,tz,11,-7,5);",
    "    assert(JSON.stringify(Array.from(rust.slice(7,10))) === JSON.stringify(expected), 'Rust legacy rotation parity mismatch at transform ' + t);",
    "  }",
    "  const contourXs = new Int32Array([64,160]); const contourYs = new Int32Array([-20,10]); const contourZs = new Int32Array([64,160]);",
    "  const contourBase = new Int32Array([0,64,128, 128,192,256, 256,320,384]);",
    "  const contourAbove = new Int32Array([300,364,428, 428,492,556, 556,620,684]);",
    "  for (const [type,param] of [[1,0],[2,40000],[3,25],[4,0],[5,0]]) {",
    "    const rust = module.contour_vertices_y(contourXs,contourYs,contourZs,2,type,param,contourBase,3,3,contourAbove,3,3,0,50,0,-64,-20,40,false);",
    "    const expected = referenceContour(type,param,contourXs,contourYs,contourZs,2,contourBase,3,3,contourAbove,3,3,0,50,0,-64,-20,40);",
    "    assert(JSON.stringify(Array.from(rust)) === JSON.stringify(Array.from(expected)), 'Rust contour parity mismatch for type ' + type + ': ' + JSON.stringify(Array.from(rust)) + ' vs ' + JSON.stringify(Array.from(expected)));",
    "  }",
    "  assert(typeof module.mirror_model_geometry === 'function', 'Rust mirror geometry export missing');",
    "  const mirrorResult = module.mirror_model_geometry(",
    "    new Int32Array([10,-20]), new Int32Array([0,1]), new Int32Array([2,3]),",
    "  );",
    "  assert(JSON.stringify(Array.from(mirrorResult)) === JSON.stringify([2,2,-10,20,2,3,0,1]), 'Rust mirror geometry mismatch');",
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
    "  assert(typeof module.build_model_faces === 'function', 'Rust model-face builder export missing');",
    "  const faceColors3 = new Int32Array([10,-2,20,30,40]);",
    "  const faceAlphas = new Int8Array([0,0,1,-1,0]);",
    "  const facePriorities = new Int8Array([1,2,3,4,5]);",
    "  const faceLayers = new Uint8Array([0,1,2,3,4]);",
    "  const faceTextures = new Int16Array([-1,-1,-1,-1,7]);",
    "  const allFaces = module.build_model_faces(faceColors3, faceAlphas, facePriorities, faceLayers, faceTextures, new Int32Array([7]), -1);",
    "  assert(JSON.stringify(Array.from(allFaces)) === JSON.stringify([0,255,1,0,-1, 2,255,3,2,-1, 3,3,4,3,-1, 4,255,5,4,7]), 'Rust model-face builder all-face semantics mismatch');",
    "  const opaqueFaces = module.build_model_faces(faceColors3, faceAlphas, facePriorities, faceLayers, faceTextures, new Int32Array([7]), 0);",
    "  assert(JSON.stringify(Array.from(opaqueFaces)) === JSON.stringify([0,255,1,0,-1, 2,255,3,2,-1]), 'Rust model-face opaque filtering mismatch');",
    "  const transparentFaces = module.build_model_faces(faceColors3, faceAlphas, facePriorities, faceLayers, faceTextures, new Int32Array([7]), 1);",
    "  assert(JSON.stringify(Array.from(transparentFaces)) === JSON.stringify([3,3,4,3,-1, 4,255,5,4,7]), 'Rust model-face transparent filtering mismatch');",
    "  assert(typeof module.build_draw_list === 'function', 'Rust draw-list builder export missing');",
    "  const preparedDrawList = module.build_draw_list(new Uint32Array([0,6,1,0, 24,12,3,2]));",
    "  const preparedRanges = preparedDrawList.flat_ranges();",
    "  const preparedPlanes = preparedDrawList.planes();",
    "  assert(JSON.stringify(Array.from(preparedRanges)) === JSON.stringify([0,6,1,24,12,3]), 'Rust draw-list ranges mismatch');",
    "  assert(JSON.stringify(Array.from(preparedPlanes)) === JSON.stringify([0,2]), 'Rust draw-list plane metadata mismatch');",
    "  preparedDrawList.free();",
    "  assert(typeof module.hash_model_geometry === 'function', 'Rust model-hash export missing');",
    "  const modelHashA = module.hash_model_geometry(new Int32Array([1]), new Int32Array([2]), new Int32Array([3]), new Int32Array([4]), new Int32Array([5]), new Int32Array([6]), new Int32Array([-1]));",
    "  const modelHashB = module.hash_model_geometry(new Int32Array([1]), new Int32Array([2]), new Int32Array([3]), new Int32Array([4]), new Int32Array([5]), new Int32Array([7]), new Int32Array([-1]));",
    "  assert(modelHashA !== modelHashB, 'Rust model hash did not react to geometry changes');",
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
    "  // Keep this fixture untextured (material 0) so it isolates water/material shading",
    "  // from texture-array layer selection.",
    "  const rasterVertices = rasterTriangleWords();",
    "  // Submit both windings: this fixture validates water/material shading, not culling policy.",
    "  const rasterIndices = new Uint32Array([0,1,2, 0,2,1]);",
    "  renderer.upload_geometry(rasterVertices, rasterIndices);",
    "  renderer.upload_static_passes(modelInfoPacket(0), new Uint32Array([0,6,1]), new Uint8Array([0]), emptyU16, emptyU32, emptyU8);",
    "  renderer.upload_texture_array(new Uint8Array([255,255,255,255]), 1, 1, 1);",
    "  const dryMaterials = new Int8Array(24); dryMaterials[3] = 1;",
    "  renderer.upload_materials(dryMaterials, 1);",
    "  renderer.upload_water_mask(new Uint8Array(4 * 4 * 4), 4, 1);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  // Render after the static map load-fade window. At currentTime === timeLoaded",
    "  // the production shader intentionally fogs the scene fully to the sky color.",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 2, 1, 3, false, false, 1, false);",
    "  assert(renderer.last_draw_calls() === 1 && renderer.last_submitted_indices() === 6, 'water acceptance dry triangle was not submitted');",
    "  renderer.present_frame();",
    "  const dryPixel = readCenter(gl);",
    "  const wetMaterials = new Int8Array(dryMaterials);",
    "  wetMaterials[5] = 1; // material 0, row 1.g = MATERIAL_FLAG_WATER",
    "  // Use an intentionally high-contrast, opaque red water material. Keeping",
    "  // normals/specular/duration at zero makes this fixture deterministic on",
    "  // both native GPU drivers and headless SwiftShader.",
    "  wetMaterials[8] = 255; wetMaterials[9] = 0; wetMaterials[10] = 0; wetMaterials[11] = 255;",
    "  renderer.upload_materials(wetMaterials, 1);",
    "  const wetMask = new Uint8Array(4 * 4 * 4);",
    "  for (let i = 0; i < 16; i++) { wetMask[i*4] = 0; wetMask[i*4+1] = 0; wetMask[i*4+2] = 255; wetMask[i*4+3] = 255; }",
    "  renderer.upload_water_mask(wetMask, 4, 1);",
    "  renderer.begin_static_frame(acceptanceSky);",
    "  renderer.render_active_static_map_pass(identity(), identity(), identity(), 1, acceptanceSky, acceptanceNoHsl, acceptancePlayerPos, 20, 15, 2, 1, 3, false, false, 1, false);",
    "  renderer.present_frame();",
    "  const wetPixel = readCenter(gl);",
    "  const waterDelta = Math.abs(wetPixel[0]-dryPixel[0]) + Math.abs(wetPixel[1]-dryPixel[1]) + Math.abs(wetPixel[2]-dryPixel[2]);",
    "  assert(dryPixel[0] + dryPixel[1] + dryPixel[2] > 24, 'water acceptance dry triangle did not cover the sampled pixel: ' + JSON.stringify(Array.from(dryPixel)));",
    "  assert(dryPixel[3] > 0 && wetPixel[3] > 0, 'water acceptance triangle did not rasterize');",
    "  assert(waterDelta > 24, 'water material/mask path did not materially change the presented static pixel: dry=' + JSON.stringify(Array.from(dryPixel)) + ', wet=' + JSON.stringify(Array.from(wetPixel)));",
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
