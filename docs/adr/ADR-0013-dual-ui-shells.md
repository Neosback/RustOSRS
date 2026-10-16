# ADR-0013: Two UI shells (egui and Svelte) over one UI contract

Status: **Proposed** (2026-10-16). Not accepted until a spike (see Follow-up) proves the contract on
one real panel in both shells. It would supersede ADR-0002's "wasm deferred" for the editor and
extend ADR-0007 rather than replace it.

## Context

- The native shell is eframe/egui (ADR-0007). The editor already builds for `wasm32` (Trunk, File System
  Access for the cache, `raise_limits` for the 117HD texture array), so a browser build exists, but
  with the same egui panels.
- The owner wants a Svelte web UI (bits-ui components, Dockview docking) **in addition to** the
  native one, a launcher that asks which to use, 117HD working in the browser, and tools that are
  cheap to add without writing UI twice or hand-plumbing state between UI and core.
- Today every panel reads and mutates `EditorApp` fields directly. That is why panels are fast to write
  and why they cannot be reused by another UI.

## Decision (proposed)

1. **The Rust core stays authoritative.** Edited semantics, history, workers and GPU resources do not
   move. Neither shell owns product state (ADR-0003, ADR-0011 hold).
2. **One UI contract**, a Rust module with no egui types: typed *view models* (what a panel shows),
   typed *commands* (what a user asked for) and *events* (what changed). All `serde`; TypeScript types
   are generated from the Rust definitions (`ts-rs` or `specta`, chosen in the spike) so the web
   side never hand-copies a type.
3. **Declarative panels for the common case.** A panel is data: sections, rows with a field kind
   (toggle, number with range, enum, text, id picker, tile coordinate, button, list, read-only text,
   progress), help text and enabled state. Each shell has one generic renderer. A new tool, settings
   page or def editor written once in Rust appears in both shells with no UI code. The settings
   registry (`settings_window/rows.rs`) is already this shape.
4. **Custom widgets are written once per shell, reused everywhere**: `SoundPlayButton`, `NpcPicker`,
   `ColourSwatch`, a virtualized list. Only a truly bespoke panel is written twice, or in one shell
   with a generic fallback in the other.
5. **egui keeps the viewport.** Picking highlights, gizmos, spawn footprints, route lines, in-viewport
   HUD, the minimap and tile info stay in egui painting over the wgpu canvas, because they are tied to
   camera projection and need no DOM. In the web shell eframe runs "canvas only": no panels, menus
   or docking; the DOM provides those.
6. **Web shell = WASM core + Svelte 5 + bits-ui + `dockview-core`**, served as static files. Dockview's
   core is framework-agnostic; each panel is a Svelte component mounted into the element Dockview
   hands out. Commands go to the core through `wasm-bindgen` calls; view-model changes come back as
   events. There is no second process and no frame streaming.
7. **Launcher.** First run asks "Native UI" or "Web UI" and remembers the answer (`--ui native|web`
   overrides). "Web UI" serves the built Svelte bundle from a local static server and opens the
   browser. A native process does not render a viewport for a browser to display.
8. **117HD in the browser requires WebGPU.** The HD path uses storage buffers and a texture array of
   up to 1024 layers (`HD_TEXTURE_LAYERS`), neither available on WebGL2. WebGPU's default layer limit
   (256) is below the ~385 the material set needs, so the device must request the adapter's limit,
   which `raise_limits` already does. WebGL2 browsers get the Reference profile only. Acceptance:
   the same region renders in the browser and natively on the same adapter, compared with the
   existing HD tolerance (HD captures differ by a channel level between identical runs).
9. **A shared splash** ("OpenRune" filling like a liquid as the first regions stream in) is driven by
   a small progress model in the contract, so both shells show the same thing.

## Consequences

Positive: one place to define a tool; typed, versioned commands (also scriptable and testable
headlessly, which the existing `npc ...`/`sound ...` script commands already hint at); the browser
build gains real docking and accessible components; egui stays where it is strongest.

Costs and risks:

- Two renderers to maintain for the generic widget set, and a migration of existing panels (about
  thirty). Migrate on touch, not all at once; new panels start declarative.
- A declarative description cannot express every interaction. The escape hatch (custom widgets,
  single-shell panels) must stay cheap or the contract gets bypassed.
- Browser limits: File System Access is Chromium-only, WASM threads need cross-origin isolation for
  `SharedArrayBuffer`, the WASM bundle is large, and keyboard focus has to be arbitrated between the
  canvas and DOM panels.
- Two UI toolchains (Cargo and pnpm/Vite) in CI.

## Alternatives considered

- **Embed a WebView (Tauri/wry) in the native window.** One window, but it adds a native web engine
  dependency and a native-to-web bridge that duplicates the WASM one. Reconsider if the browser build
  is not wanted as a product.
- **Native process plus Svelte in a browser (panels over WebSocket).** Needs the viewport somewhere;
  streaming frames is bandwidth-heavy and laggy, and a native viewport window beside a browser is a
  poor experience.
- **egui for everything (current).** Cheapest, but no accessible DOM, weak text/table editing, no
  web-native docking.
- **Rust web frameworks (Leptos/Yew/Dioxus) instead of Svelte.** Shares types without generation, but
  the owner prefers Svelte and its component ecosystem.
- **Svelte for everything, drop egui.** Overlays then need a second projection implementation or
  renderer-owned overlay primitives; large cost for no clear gain.

## Constraints / invariants affected

- B4 (UI never leaks downward): the contract module must not depend on egui, and semantic crates must
  not depend on the contract. The contract may depend on `osrs-core` values.
- `EditorApp` remains a composition root; commands are executed by existing owners and gateways
  (`EditRuntime` for edits), never by a shell.
- Settings and key bindings keep one source of truth (`SettingsRuntime`).

## Follow-up work

1. Spike: the contract plus one panel (NPC info and spawn settings) rendered by egui and by a minimal
   Svelte + Dockview page against the WASM build; measure the time to add a field.
2. Choose the type-generation tool and the event encoding (full snapshot versus patches).
3. Browser verification of the HD profile on WebGPU; record limits and failures.
4. Shared splash and launcher once there is a Svelte bundle to serve.
