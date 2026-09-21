import type { RustRendererWasmConstructor } from "./RustRendererBridge";

export interface RustVertexBufferBuilderWasm {
    clear(): void;
    vertex_count(): number;
    push_batch(
        integerFields: Int32Array,
        uvFields: Float32Array,
        flags: Uint8Array,
    ): Uint32Array;
    packed_vertices(): Uint32Array;
}

export type RustVertexBufferBuilderWasmConstructor = new () => RustVertexBufferBuilderWasm;

export interface RustRendererWebModule {
    default(input?: unknown): Promise<unknown>;
    RustWebGlRenderer: RustRendererWasmConstructor;
    RustVertexBufferBuilder?: RustVertexBufferBuilderWasmConstructor;
    build_model_info_texture_data?: (
        commandInstanceCounts: Uint32Array,
        instanceFields: Int32Array,
    ) => Uint16Array;
}

let modulePromise: Promise<RustRendererWebModule> | undefined;

function getPublicBaseUrl(): string {
    const publicUrl = process.env.PUBLIC_URL ?? "";
    return publicUrl.endsWith("/") ? publicUrl.slice(0, -1) : publicUrl;
}

export function getRustRendererModuleUrl(): string {
    return `${getPublicBaseUrl()}/rust-renderer/rustosrs_renderer.js`;
}

export async function loadRustRendererModule(): Promise<RustRendererWebModule> {
    if (!modulePromise) {
        modulePromise = import(
            /* webpackIgnore: true */
            getRustRendererModuleUrl()
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
