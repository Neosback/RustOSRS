import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(here, "..");

const rustPath = path.join(clientRoot, "rust-renderer", "src", "lib.rs");
const tsPath = path.join(clientRoot, "render", "rust", "RendererPacket.ts");

const rustSource = fs.readFileSync(rustPath, "utf8");
const tsSource = fs.readFileSync(tsPath, "utf8");

const rustMatch = rustSource.match(/RENDERER_ABI_VERSION:\s*u32\s*=\s*(\d+)/);
const tsMatch = tsSource.match(/RUST_RENDERER_ABI_VERSION\s*=\s*(\d+)\s+as\s+const/);

if (!rustMatch) {
    throw new Error(`Could not find Rust renderer ABI in ${rustPath}`);
}
if (!tsMatch) {
    throw new Error(`Could not find TypeScript renderer ABI in ${tsPath}`);
}

const rustAbi = Number(rustMatch[1]);
const tsAbi = Number(tsMatch[1]);

if (rustAbi !== tsAbi) {
    throw new Error(
        `Rust renderer ABI mismatch: Rust=${rustAbi}, TypeScript=${tsAbi}`,
    );
}

console.log(`Rust renderer ABI ${rustAbi} is aligned across Rust and TypeScript`);
