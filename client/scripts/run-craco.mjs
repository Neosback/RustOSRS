import { existsSync } from "node:fs";
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

const craco = path.join(clientDir, "node_modules", "@craco", "craco", "dist", "bin", "craco.js");
if (!existsSync(craco)) {
    console.error(
        "Client dependencies are not installed. Run 'node scripts/bootstrap.mjs' from client/ first.",
    );
    process.exit(1);
}

const child = spawn(process.execPath, [craco, command], {
    cwd: clientDir,
    stdio: "inherit",
    env: {
        ...process.env,
        NODE_ENV: mode,
    },
});

child.on("error", (error) => {
    console.error("Failed to launch CRACO:", error);
    process.exitCode = 1;
});

child.on("exit", (code, signal) => {
    if (signal) {
        process.kill(process.pid, signal);
        return;
    }
    process.exitCode = code ?? 1;
});
