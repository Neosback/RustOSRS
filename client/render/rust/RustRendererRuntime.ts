import { RustRendererBridge } from "./RustRendererBridge";
import { loadRustRendererModule } from "./RustRendererModule";

export type RustRendererRuntimeMode = "off" | "shadow" | "primary";

export interface RustRendererShadowRuntime {
    mode: "shadow" | "primary";
    canvas: HTMLCanvasElement;
    bridge: RustRendererBridge;
    disposeDom?: () => void;
}


export function getRustRendererRuntimeMode(
    search: string = typeof window !== "undefined" ? window.location.search : "",
): RustRendererRuntimeMode {
    const raw = new URLSearchParams(search).get("rust-renderer");
    if (raw === "off") return "off";
    if (raw === "shadow") return "shadow";
    return "primary";
}

export function isRustPrimaryRuntime(
    search: string = typeof window !== "undefined" ? window.location.search : "",
): boolean {
    return getRustRendererRuntimeMode(search) === "primary";
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

    const module = await loadRustRendererModule();
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
