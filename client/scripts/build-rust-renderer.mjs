import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");
const crateDir = path.join(clientDir, "rust-renderer");
const outputDir = path.join(clientDir, "public", "rust-renderer");
const wasmInput = path.join(
    crateDir,
    "target",
    "wasm32-unknown-unknown",
    "release",
    "rustosrs_renderer.wasm",
);

function run(command, args, options = {}) {
    execFileSync(command, args, {
        cwd: options.cwd ?? clientDir,
        stdio: "inherit",
        env: process.env,
    });
}

function read(command, args, options = {}) {
    return execFileSync(command, args, {
        cwd: options.cwd ?? clientDir,
        encoding: "utf8",
        env: process.env,
    }).trim();
}

function resolveWasmBindgenVersion() {
    const metadata = JSON.parse(
        read(
            "cargo",
            ["metadata", "--format-version", "1", "--locked"],
            { cwd: crateDir },
        ),
    );
    const pkg = metadata.packages.find((entry) => entry.name === "wasm-bindgen");
    if (!pkg) {
        throw new Error("cargo metadata did not resolve wasm-bindgen");
    }
    return pkg.version;
}

function hasMatchingWasmBindgen(version) {
    const probe = spawnSync("wasm-bindgen", ["--version"], {
        cwd: crateDir,
        encoding: "utf8",
        env: process.env,
    });
    if (probe.status !== 0) return false;

    const output = `${probe.stdout ?? ""} ${probe.stderr ?? ""}`;
    return output.includes(version);
}

run("cargo", ["build", "--release", "--target", "wasm32-unknown-unknown"], {
    cwd: crateDir,
});

if (!existsSync(wasmInput)) {
    throw new Error(`Rust renderer wasm output is missing: ${wasmInput}`);
}

const wasmBindgenVersion = resolveWasmBindgenVersion();
if (!hasMatchingWasmBindgen(wasmBindgenVersion)) {
    console.log(
        `Installing wasm-bindgen-cli ${wasmBindgenVersion} to match the resolved Rust crate...`,
    );
    run(
        "cargo",
        [
            "install",
            "wasm-bindgen-cli",
            "--version",
            wasmBindgenVersion,
            "--locked",
            "--force",
        ],
        { cwd: crateDir },
    );
}

rmSync(outputDir, { recursive: true, force: true });
mkdirSync(outputDir, { recursive: true });

run(
    "wasm-bindgen",
    [
        wasmInput,
        "--target",
        "web",
        "--out-dir",
        outputDir,
        "--out-name",
        "rustosrs_renderer",
        "--typescript",
    ],
    { cwd: crateDir },
);

console.log(`Rust renderer web package written to ${outputDir}`);
