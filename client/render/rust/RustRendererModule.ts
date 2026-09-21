import {
    registerRustBasicVertexTransformer,
    registerRustContourBuilder,
    registerRustLegacyTransformer,
    registerRustSkeletalSkinner,
} from "../../rs/model/RustModelTransforms";
import { registerRustTextureMapper } from "../../rs/model/RustTextureMapper";
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
    transform_vertices_basic?: (
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        mode: number,
        a: number,
        b: number,
        c: number,
    ) => Int32Array;
    compute_model_uvs?: (
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        indices0: Int32Array,
        indices1: Int32Array,
        indices2: Int32Array,
        faceTextures: Int16Array,
        textureCoords: Int8Array,
        textureRenderTypes: Int8Array,
        textureMappingP: Int16Array,
        textureMappingM: Int16Array,
        textureMappingN: Int16Array,
        textureScaleX: Int32Array,
        textureScaleY: Int32Array,
        textureScaleZ: Int32Array,
        textureRotation: Int8Array,
        textureDirection: Int8Array,
        textureSpeed: Int32Array,
        textureTransU: Int32Array,
        textureTransV: Int32Array,
    ) => Float32Array;
    contour_vertices_y?: (
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        usedVertexCount: number,
        contourType: number,
        param: number,
        heightMap: Int32Array,
        heightWidth: number,
        heightDepth: number,
        heightMapAbove: Int32Array,
        aboveWidth: number,
        aboveDepth: number,
        sceneX: number,
        sceneHeight: number,
        sceneZ: number,
        type2Denominator: number,
        minY: number,
        maxY: number,
        preserveType1UnusedOob: boolean,
    ) => Int32Array;
    apply_legacy_transforms?: (
        verticesX: Int32Array,
        verticesY: Int32Array,
        verticesZ: Int32Array,
        faceAlphas: Int8Array,
        faceColors: Uint16Array,
        vertexLabelOffsets: Uint32Array,
        vertexLabelIndices: Int32Array,
        faceLabelOffsets: Uint32Array,
        faceLabelIndices: Int32Array,
        operationFields: Int32Array,
        operationLabelOffsets: Uint32Array,
        operationLabels: Int32Array,
        initialOriginX: number,
        initialOriginY: number,
        initialOriginZ: number,
    ) => Int32Array;
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
                registerRustLegacyTransformer(
                    typeof typed.apply_legacy_transforms === "function"
                        ? typed.apply_legacy_transforms
                        : undefined,
                );
                registerRustContourBuilder(
                    typeof typed.contour_vertices_y === "function"
                        ? typed.contour_vertices_y
                        : undefined,
                );
                registerRustBasicVertexTransformer(
                    typeof typed.transform_vertices_basic === "function"
                        ? typed.transform_vertices_basic
                        : undefined,
                );
                registerRustTextureMapper(
                    typeof typed.compute_model_uvs === "function"
                        ? typed.compute_model_uvs
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
