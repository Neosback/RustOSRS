import {
    RustRendererBridge,
    type RustRendererWasmConstructor,
} from "./RustRendererBridge";

export type RustRendererRuntimeMode = "off" | "shadow";

interface RustRendererWebModule {
    default(input?: unknown): Promise<unknown>;
    RustWebGlRenderer: RustRendererWasmConstructor;
}

export interface RustRendererShadowRuntime {
    mode: "shadow";
    canvas: HTMLCanvasElement;
    bridge: RustRendererBridge;
}

let modulePromise: Promise<RustRendererWebModule> | undefined;

export function getRustRendererRuntimeMode(
    search: string = typeof window !== "undefined" ? window.location.search : "",
): RustRendererRuntimeMode {
    const raw = new URLSearchParams(search).get("rust-renderer");
    return raw === "shadow" ? "shadow" : "off";
}

function getPublicBaseUrl(): string {
    const publicUrl = process.env.PUBLIC_URL ?? "";
    return publicUrl.endsWith("/") ? publicUrl.slice(0, -1) : publicUrl;
}

async function importRustRendererModule(): Promise<RustRendererWebModule> {
    if (!modulePromise) {
        const moduleUrl =
            `${getPublicBaseUrl()}/rust-renderer/rustosrs_renderer.js`;

        modulePromise = import(
            /* webpackIgnore: true */
            moduleUrl
        )
            .then(async (module) => {
                const typed = module as unknown as RustRendererWebModule;
                await typed.default();
                if (typeof typed.RustWebGlRenderer !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export RustWebGlRenderer",
                    );
                }
                return typed;
            })
            .catch((error) => {
                modulePromise = undefined;
                throw new Error(
                    "Failed to load the Rust renderer web package. "
                    + "Run 'yarn build:rust-renderer' before using ?rust-renderer=shadow.",
                    { cause: error },
                );
            });
    }

    return modulePromise;
}

export function syncRustShadowCanvasSize(
    shadowCanvas: HTMLCanvasElement,
    sourceCanvas: HTMLCanvasElement,
): void {
    const width = Math.max(1, sourceCanvas.width | 0);
    const height = Math.max(1, sourceCanvas.height | 0);
    if (shadowCanvas.width !== width) shadowCanvas.width = width;
    if (shadowCanvas.height !== height) shadowCanvas.height = height;
}

export async function createRustRendererShadowRuntime(
    sourceCanvas: HTMLCanvasElement,
): Promise<RustRendererShadowRuntime | undefined> {
    if (getRustRendererRuntimeMode() !== "shadow") {
        return undefined;
    }

    const module = await importRustRendererModule();
    const canvas = document.createElement("canvas");
    canvas.dataset.renderer = "rust-shadow";
    syncRustShadowCanvasSize(canvas, sourceCanvas);

    return {
        mode: "shadow",
        canvas,
        bridge: new RustRendererBridge(canvas, module.RustWebGlRenderer),
    };
}
