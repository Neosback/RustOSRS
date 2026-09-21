import {
    recordRustStage5Attempt,
    recordRustStage5Fallback,
    recordRustStage5Success,
} from "./RustStage5Ownership";
import type { ModelData } from "./ModelData";

export type RustTextureMapper = (
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

let textureMapper: RustTextureMapper | undefined;
let warnedAboutTextureMappingFailure = false;

const EMPTY_I8 = new Int8Array(0);
const EMPTY_I16 = new Int16Array(0);
const EMPTY_I32 = new Int32Array(0);

export function registerRustTextureMapper(
    mapper: RustTextureMapper | undefined,
): void {
    textureMapper = mapper;
}

export function computeTextureCoordsWithRustIfReady(
    model: ModelData,
    effectiveFaceTextures: Int16Array,
): Float32Array | undefined {
    recordRustStage5Attempt("uv");
    if (!textureMapper) {
        recordRustStage5Fallback("uv", "backend unavailable");
        return undefined;
    }

    let uvs: Float32Array;
    try {
        uvs = textureMapper(
            model.verticesX,
            model.verticesY,
            model.verticesZ,
            model.indices1,
            model.indices2,
            model.indices3,
            effectiveFaceTextures,
            model.textureCoords ?? EMPTY_I8,
            model.textureRenderTypes ?? EMPTY_I8,
            model.textureMappingP ?? EMPTY_I16,
            model.textureMappingM ?? EMPTY_I16,
            model.textureMappingN ?? EMPTY_I16,
            model.textureScaleX ?? EMPTY_I32,
            model.textureScaleY ?? EMPTY_I32,
            model.textureScaleZ ?? EMPTY_I32,
            model.textureRotation ?? EMPTY_I8,
            model.textureDirection ?? EMPTY_I8,
            model.textureSpeed ?? EMPTY_I32,
            model.textureTransU ?? EMPTY_I32,
            model.textureTransV ?? EMPTY_I32,
        );
    } catch (error) {
        if (!warnedAboutTextureMappingFailure) {
            warnedAboutTextureMappingFailure = true;
            console.warn(
                "[RustTextureMapper] Rust UV generation failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("uv", "backend threw", true);
        return undefined;
    }
    if (uvs.length !== model.faceCount * 6) {
        recordRustStage5Fallback("uv", "invalid result length", true);
        return undefined;
    }
    recordRustStage5Success("uv");
    return uvs;
}
