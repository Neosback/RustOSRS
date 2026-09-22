import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientDir = path.resolve(scriptDir, "..");
const mode = process.argv[2];
const command = process.argv[3];

if (!["development", "production", "test"].includes(mode) || !command) {
    console.error("Usage: node scripts/run-craco.mjs <development|production|test> <command>");
    process.exit(2);
}

// Use Yarn to resolve the CRACO binary so this works with both Yarn PnP and
// node_modules installs. A hard-coded node_modules path breaks on a clean
// Yarn 4 checkout because PnP is the default linker.
const child = spawn("corepack", ["yarn", "exec", "craco", command], {
    cwd: clientDir,
    stdio: "inherit",
    env: {
        ...process.env,
        NODE_ENV: mode,
    },
});

child.on("error", (error) => {
    console.error(
        "Failed to launch CRACO through Corepack. Run 'node scripts/bootstrap.mjs' first.",
        error,
    );
    process.exitCode = 1;
});

child.on("exit", (code, signal) => {
    if (signal) {
        process.kill(process.pid, signal);
        return;
    }
    process.exitCode = code ?? 1;
});
