import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");

function nodeSupported() {
    const [major, minor] = process.versions.node.split(".").map(Number);
    return major === 22 && minor >= 5;
}

function hasCommand(command, args = ["--version"]) {
    const result = spawnSync(command, args, {
        cwd: clientDir,
        stdio: "ignore",
        env: process.env,
    });
    return !result.error && result.status === 0;
}

function run(command, args, label) {
    console.log(`\n==> ${label}`);
    const result = spawnSync(command, args, {
        cwd: clientDir,
        stdio: "inherit",
        env: process.env,
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
        throw new Error(`${label} failed with exit code ${result.status}`);
    }
}

if (!nodeSupported()) {
    console.error(
        `RustOSRS requires Node 22.5+ and below Node 23. Current version: ${process.versions.node}\n` +
            "From the repository root, use 'nvm install && nvm use' if you use nvm.",
    );
    process.exit(1);
}

if (!hasCommand("corepack")) {
    console.error(
        "Corepack is required. Use the supported Node 22 release and run 'corepack enable'.",
    );
    process.exit(1);
}

if (!hasCommand("rustup") || !hasCommand("cargo")) {
    console.error(
        "Rust installed through rustup is required. Install it from https://rustup.rs/, reopen your shell, and rerun this command.",
    );
    process.exit(1);
}

run("rustup", ["target", "add", "wasm32-unknown-unknown"], "Preparing Rust WASM target");
run("corepack", ["yarn", "install", "--immutable"], "Installing JavaScript dependencies");
run(process.execPath, ["scripts/build-rust-renderer.mjs"], "Building Rust/WASM renderer");

console.log("\n==> Running environment doctor");
const doctorResult = spawnSync(process.execPath, ["scripts/doctor.mjs"], {
    cwd: clientDir,
    stdio: "inherit",
    env: process.env,
});
if (doctorResult.error) throw doctorResult.error;
if (doctorResult.status !== 0) {
    throw new Error("Environment doctor reported missing requirements");
}

console.log("\nRustOSRS bootstrap complete. Start the client with: yarn start");
