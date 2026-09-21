import { registerRustSkeletalSkinner } from "../../rs/model/RustModelTransforms";
import type { RustRendererWasmConstructor } from "./RustRendererBridge";

export interface RustVertexBufferBuilderWasm {
    clear(): void;
    vertex_count(): number;
    push_batch(
        integerFields: Int32Array,
        uvFields: Float32Array,
        flags: Uint8Array,
    ): Uint32Array;
    set_texture_id_map(
        textureIds: Int32Array,
        textureIndices: Int32Array,
    ): void;
    used_texture_ids(): Int32Array;
    push_terrain_tile(
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        facesA: Int32Array,
        facesB: Int32Array,
        facesC: Int32Array,
        colorsA: Int32Array,
        colorsB: Int32Array,
        colorsC: Int32Array,
        textureIds: Int32Array,
        tileX: number,
        tileZ: number,
        offsetX: number,
        offsetZ: number,
    ): Uint32Array;
    push_model_faces(
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        facesA: Int32Array,
        facesB: Int32Array,
        facesC: Int32Array,
        colorsA: Int32Array,
        colorsB: Int32Array,
        colorsC: Int32Array,
        uvs: Float32Array,
        faceFields: Int32Array,
        sceneX: number,
        sceneHeight: number,
        sceneZ: number,
        overrideHue: number,
        overrideSaturation: number,
        overrideLuminance: number,
        overrideAmount: number,
        reuseVertices: boolean,
    ): Uint32Array;
    packed_vertices(): Uint32Array;
}

export type RustVertexBufferBuilderWasmConstructor = new () => RustVertexBufferBuilderWasm;

export interface RustPreparedDrawListWasm {
    flat_ranges(): Uint32Array;
    planes(): Uint8Array;
    free?(): void;
}

export interface RustRendererWebModule {
    default(input?: unknown): Promise<unknown>;
    RustWebGlRenderer: RustRendererWasmConstructor;
    RustVertexBufferBuilder?: RustVertexBufferBuilderWasmConstructor;
    build_model_info_texture_data?: (
        commandInstanceCounts: Uint32Array,
        instanceFields: Int32Array,
    ) => Uint16Array;
    hash_model_geometry?: (
        faceColors1: Int32Array,
        faceColors2: Int32Array,
        faceColors3: Int32Array,
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        textureIds: Int32Array,
    ) => number;
    build_draw_list?: (commandFields: Uint32Array) => RustPreparedDrawListWasm;
    skin_skeletal_vertices?: (
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        vertexGroupOffsets: Uint32Array,
        boneIds: Int32Array,
        boneScales: Int32Array,
        boneMatrices: Float32Array,
    ) => Int32Array;
    build_model_faces?: (
        faceColors3: Int32Array,
        faceAlphas: Int8Array,
        priorities: Int8Array,
        renderLayers: Uint8Array,
        textureIds: Int16Array,
        transparentTextureIds: Int32Array,
        filter: number,
    ) => Int32Array;
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
                registerRustSkeletalSkinner(
                    typeof typed.skin_skeletal_vertices === "function"
                        ? typed.skin_skeletal_vertices
                        : undefined,
                );
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
