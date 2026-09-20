import {
    RustRendererBridge,
    type RustRendererWasmConstructor,
} from "./RustRendererBridge";

export type RustRendererRuntimeMode = "off" | "shadow" | "primary";

interface RustRendererWebModule {
    default(input?: unknown): Promise<unknown>;
    RustWebGlRenderer: RustRendererWasmConstructor;
}

export interface RustRendererShadowRuntime {
    mode: "shadow" | "primary";
    canvas: HTMLCanvasElement;
    bridge: RustRendererBridge;
    disposeDom?: () => void;
}

let modulePromise: Promise<RustRendererWebModule> | undefined;

export function getRustRendererRuntimeMode(
    search: string = typeof window !== "undefined" ? window.location.search : "",
): RustRendererRuntimeMode {
    const raw = new URLSearchParams(search).get("rust-renderer");
    if (raw === "shadow") return "shadow";
    if (raw === "primary") return "primary";
    return "off";
}

export function isRustPrimaryRuntime(
    search: string = typeof window !== "undefined" ? window.location.search : "",
): boolean {
    return getRustRendererRuntimeMode(search) === "primary";
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
                    + "Run 'yarn build:rust-renderer' before using the Rust renderer.",
                    { cause: error },
                );
            });
    }

    return modulePromise;
}

export function syncRustShadowCanvasSize(
    shadowCanvas: HTMLCanvasElement,
    width: number,
    height: number,
): void {
    const nextWidth = Math.max(1, width | 0);
    const nextHeight = Math.max(1, height | 0);
    if (shadowCanvas.width !== nextWidth) shadowCanvas.width = nextWidth;
    if (shadowCanvas.height !== nextHeight) shadowCanvas.height = nextHeight;
}

function attachPrimaryCanvas(
    sourceCanvas: HTMLCanvasElement,
    rustCanvas: HTMLCanvasElement,
): () => void {
    const parent = sourceCanvas.parentElement;
    if (!parent) {
        throw new Error(
            "Rust primary renderer requires the source canvas to have a parent element",
        );
    }

    const sourceContext = sourceCanvas.getContext("webgl2");
    const sourceHasAlpha =
        sourceContext?.getContextAttributes()?.alpha !== false;
    if (!sourceHasAlpha) {
        throw new Error(
            "Rust primary renderer requires an alpha-enabled Pico overlay canvas",
        );
    }

    const previousParentPosition = parent.style.position;
    const previousSourcePosition = sourceCanvas.style.position;
    const previousSourceZIndex = sourceCanvas.style.zIndex;
    const previousSourceBackground = sourceCanvas.style.backgroundColor;
    const previousRustParent = rustCanvas.parentElement;

    if (
        typeof window !== "undefined"
        && window.getComputedStyle(parent).position === "static"
    ) {
        parent.style.position = "relative";
    }

    rustCanvas.dataset.renderer = "rust-primary";
    rustCanvas.style.position = "absolute";
    rustCanvas.style.pointerEvents = "none";
    rustCanvas.style.zIndex = "0";

    sourceCanvas.style.position = "relative";
    sourceCanvas.style.zIndex = "1";
    sourceCanvas.style.backgroundColor = "transparent";

    parent.insertBefore(rustCanvas, sourceCanvas);

    const syncPrimaryCanvasBox = (): void => {
        rustCanvas.style.left = `${sourceCanvas.offsetLeft}px`;
        rustCanvas.style.top = `${sourceCanvas.offsetTop}px`;
        rustCanvas.style.width = `${Math.max(1, sourceCanvas.offsetWidth)}px`;
        rustCanvas.style.height = `${Math.max(1, sourceCanvas.offsetHeight)}px`;
    };
    syncPrimaryCanvasBox();

    const resizeObserver =
        typeof ResizeObserver !== "undefined"
            ? new ResizeObserver(syncPrimaryCanvasBox)
            : undefined;
    resizeObserver?.observe(sourceCanvas);
    resizeObserver?.observe(parent);
    window.addEventListener("resize", syncPrimaryCanvasBox);

    return () => {
        resizeObserver?.disconnect();
        window.removeEventListener("resize", syncPrimaryCanvasBox);

        if (rustCanvas.parentElement === parent) {
            parent.removeChild(rustCanvas);
        } else if (previousRustParent) {
            previousRustParent.appendChild(rustCanvas);
        }
        parent.style.position = previousParentPosition;
        sourceCanvas.style.position = previousSourcePosition;
        sourceCanvas.style.zIndex = previousSourceZIndex;
        sourceCanvas.style.backgroundColor = previousSourceBackground;
    };
}

export async function createRustRendererShadowRuntime(
    sourceCanvas: HTMLCanvasElement,
): Promise<RustRendererShadowRuntime | undefined> {
    const mode = getRustRendererRuntimeMode();
    if (mode === "off") {
        return undefined;
    }

    const module = await importRustRendererModule();
    const canvas = document.createElement("canvas");
    canvas.dataset.renderer =
        mode === "primary" ? "rust-primary" : "rust-shadow";
    syncRustShadowCanvasSize(
        canvas,
        sourceCanvas.width,
        sourceCanvas.height,
    );

    const disposeDom =
        mode === "primary"
            ? attachPrimaryCanvas(sourceCanvas, canvas)
            : undefined;

    try {
        return {
            mode,
            canvas,
            bridge: new RustRendererBridge(canvas, module.RustWebGlRenderer),
            disposeDom,
        };
    } catch (error) {
        disposeDom?.();
        throw error;
    }
}
