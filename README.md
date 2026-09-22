# RustOSRS

RustOSRS is a browser OSRS client with a Rust/WASM WebGL2 renderer.

This repository is **client-only**. It does not include a game server or RuneScape cache data. A fresh checkout can build and launch on macOS and Linux without the old monorepo layout, but you must point it at a compatible cache source and game server to enter the game.

## Requirements

- Node.js 22.x (22.5 or newer)
- Corepack (included with the supported Node 22 releases)
- Rust via rustup
- A modern browser with WebGL2
- Cache data, either local or hosted over HTTP
- A compatible WebSocket game server for login/gameplay

The repository pins Node 22 through `.nvmrc` / `.node-version` and requests the Rust WASM target through `rust-toolchain.toml`.

## Quick start

### macOS

```bash
git clone https://github.com/Neosback/RustOSRS.git
cd RustOSRS

# Node 22, using nvm if you have it
nvm install
nvm use

# Install Rust if rustup is not already available:
# https://rustup.rs/

cd client
node scripts/bootstrap.mjs
```

### Linux

```bash
git clone https://github.com/Neosback/RustOSRS.git
cd RustOSRS

# Node 22, using nvm if you have it
nvm install
nvm use

# Install Rust if rustup is not already available:
# https://rustup.rs/

cd client
node scripts/bootstrap.mjs
```

The bootstrap command:

1. verifies the supported Node version,
2. verifies `corepack`, `cargo`, and `rustup`,
3. installs the `wasm32-unknown-unknown` target,
4. installs the exact Yarn dependencies from `yarn.lock`,
5. builds the Rust/WASM renderer (including the matching `wasm-bindgen-cli` if needed).

You can rerun it safely.

## Cache setup

The browser needs an OSRS cache. The cache is deliberately not committed to this repository.

### Option A: local cache

Put the cache tree in:

```text
client/caches/
```

The directory must contain `caches.json` plus the cache directory/directories referenced by that manifest. Development automatically serves this directory at `/caches/`.

You can use another local directory without copying it into the repository:

```bash
RUSTOSRS_CACHE_DIR=/absolute/path/to/caches yarn start
```

### Option B: hosted cache

Create `client/.env.local`:

```bash
REACT_APP_CACHE_BASE_URL=https://your-host.example/caches/
```

The remote host must permit browser requests and Range requests for sparse JS5 loading.

## Game server setup

By default the client connects to:

```text
ws://localhost:43594
```

Override it in `client/.env.local`:

```bash
REACT_APP_DEFAULT_WS_URL=ws://localhost:43594
REACT_APP_DEFAULT_SERVER_ADDRESS=localhost:43594
REACT_APP_DEFAULT_SERVER_NAME=Local Development
REACT_APP_DEFAULT_SERVER_SECURE=false
```

The server implementation is not part of this repository.

## Run

From `client/`:

```bash
yarn start

# If your Node installation did not allow Corepack to create the yarn shim:
corepack yarn start
```

Then open the URL printed by the CRA development server.

Useful commands:

```bash
yarn doctor                 # environment + cache/server diagnostics
yarn start                  # Rust-primary renderer
yarn start:pico             # legacy Pico renderer (diagnostic/fallback)
yarn build                  # production build with Rust renderer
yarn build:pico             # production build without rebuilding Rust
yarn typecheck
yarn test
```

## Troubleshooting

### `cargo: command not found` or `rustup: command not found`

Install Rust with rustup, reopen the shell (or source `$HOME/.cargo/env`), and rerun:

```bash
node scripts/bootstrap.mjs
```

### `wasm32-unknown-unknown target may not be installed`

Run:

```bash
rustup target add wasm32-unknown-unknown
```

The bootstrap script does this automatically when rustup is available.

### Yarn/Corepack errors

Use Node 22, then:

```bash
corepack enable
corepack prepare yarn@4.12.0 --activate
```

The project uses the Yarn version declared in `client/package.json`.

### Client starts but cannot load assets

Run:

```bash
yarn doctor
```

If no cache source is configured, put the cache under `client/caches/`, set `RUSTOSRS_CACHE_DIR`, or set `REACT_APP_CACHE_BASE_URL`.

### Login cannot connect

A game server is not bundled. Start a compatible server or configure the WebSocket endpoint in `.env.local`.

## Why the setup changed

RustOSRS was extracted from a larger repository, but the client scripts still referenced sibling paths such as `../server/scripts/ensure-cache.ts` and `../scripts/run-with-env.mjs`. Those files are not present here, so the old default `yarn start` and production build were not valid in a clean standalone checkout.

The current scripts are self-contained inside this repository.
