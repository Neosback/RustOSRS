import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(here, "..");

// These legacy PicoGL shaders are frozen parity references while the Rust
// renderer is the primary scene backend. If one changes, update the matching
// Rust shader first, review parity, then deliberately refresh this manifest.
const expectedGitBlobSha1 = {
    "render/shaders/main.vert.glsl": "4d396ed90953cffbef608d264b27806bf4669f27",
    "render/shaders/main.frag.glsl": "63fc12e2359b417f4e1051f9d24ea96e084a65a9",
    "render/shaders/npc.vert.glsl": "8ded741247a9eb94a2dd77212fa0c3341f025cf9",
    "render/shaders/player.vert.glsl": "1e8b2160c4ba819f642caf13b251dea378caf4e3",
    "render/shaders/player.frag.glsl": "ac413e697a5a01761bf0bd413821fe79592fbe5c",
    "render/shaders/projectile.vert.glsl": "584390a0c3ba4ea125b78d5d7976b17eaf872b7d",
    "render/shaders/frame-fxaa.vert.glsl": "8b5c071c9a1794fd05592aaf7f82d045ef2b22f7",
    "render/shaders/frame-fxaa.frag.glsl": "f483a5ab01260aa1c4a3f15cee13bccbd2fe5138",
    "render/shaders/includes/fog.glsl": "c99ae3c49db136dbce6e7e7654f834d882887110",
    "render/shaders/includes/hsl-to-rgb.glsl": "c45ab2b36ef75cffa513452122fd25e8a8e193c2",
    "render/shaders/includes/vertex.glsl": "34fcc9a695db49a5558912599a2619940ad94f46",
    "render/shaders/includes/material.glsl": "97982d4627f81389e5c5bbe168bb5fe52daeee2e",
    "render/shaders/includes/height-map.glsl": "eedb0fed035a35e8ea7a443b2e6a73272e3f2159",
    "render/shaders/includes/priority-depth.glsl": "e1c3b3ab735eda9bc107f2cb57cde6a40d942c80",
    "render/shaders/includes/scene-uniforms.glsl": "0139ed8f95b3c86ce98d7a3467d23ea342ac1a10",
    "render/shaders/includes/unpack-float.glsl": "2023c7368001fe951d39dc060aeea5267454b0d2"
};

function gitBlobSha1(bytes) {
    const header = Buffer.from(`blob ${bytes.length}\0`);
    return createHash("sha1")
        .update(header)
        .update(bytes)
        .digest("hex");
}

const changed = [];
for (const [relative, expected] of Object.entries(expectedGitBlobSha1)) {
    const bytes = readFileSync(path.join(clientRoot, relative));
    const actual = gitBlobSha1(bytes);
    if (actual !== expected) {
        changed.push({ relative, expected, actual });
    }
}

if (changed.length > 0) {
    const details = changed
        .map(({ relative, expected, actual }) =>
            `  ${relative}\n    expected ${expected}\n    actual   ${actual}`
        )
        .join("\n");
    throw new Error(
        "Legacy PicoGL shader reference changed without a Rust parity review.\n"
        + details
        + "\nUpdate the Rust shader semantics first, validate parity, then refresh this manifest.",
    );
}

console.log(
    `Legacy shader parity reference is stable (${Object.keys(expectedGitBlobSha1).length} files)`,
);
