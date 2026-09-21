import type { Model } from "../../rs/model/Model";
import type { TextureLoader } from "../../rs/texture/TextureLoader";
import { loadRustRendererModule } from "./RustRendererModule";

export type PreparedModelFace = {
    index: number;
    alpha: number;
    priority: number;
    renderLayer?: number;
    textureId: number;
};

type RawFaceBuilder = (
    faceColors3: Int32Array,
    faceAlphas: Int8Array,
    priorities: Int8Array,
    renderLayers: Uint8Array,
    textureIds: Int16Array,
    transparentTextureIds: Int32Array,
    filter: number,
) => Int32Array;

export type ModelFaceBuilder = (
    model: Model,
    textureLoader: TextureLoader | undefined,
    filter: -1 | 0 | 1,
) => PreparedModelFace[];

const EMPTY_I8 = new Int8Array(0);
const EMPTY_U8 = new Uint8Array(0);
const EMPTY_I16 = new Int16Array(0);
const EMPTY_I32 = new Int32Array(0);
const FACE_RECORD_STRIDE = 5;

function collectTransparentTextureIds(
    model: Model,
    textureLoader: TextureLoader | undefined,
): Int32Array {
    const textures = model.faceTextures;
    if (!textureLoader || !textures || textures.length === 0) {
        return EMPTY_I32;
    }

    const ids = new Set<number>();
    for (let index = 0; index < textures.length; index++) {
        const textureId = textures[index] | 0;
        if (textureId !== -1 && textureLoader.isTransparent(textureId)) {
            ids.add(textureId);
        }
    }
    return ids.size > 0 ? Int32Array.from(ids) : EMPTY_I32;
}

function decodeFaces(flat: Int32Array): PreparedModelFace[] {
    if (flat.length % FACE_RECORD_STRIDE !== 0) {
        throw new Error(
            `Rust face builder returned malformed record length ${flat.length}`,
        );
    }

    const faces = new Array<PreparedModelFace>(flat.length / FACE_RECORD_STRIDE);
    for (let offset = 0, faceIndex = 0; offset < flat.length; offset += FACE_RECORD_STRIDE) {
        const renderLayer = flat[offset + 3] | 0;
        faces[faceIndex++] = {
            index: flat[offset] | 0,
            alpha: flat[offset + 1] | 0,
            priority: flat[offset + 2] | 0,
            renderLayer: renderLayer >= 0 ? renderLayer : undefined,
            textureId: flat[offset + 4] | 0,
        };
    }
    return faces;
}

function buildRawFacePacket(
    rawBuilder: RawFaceBuilder,
    model: Model,
    textureLoader: TextureLoader | undefined,
    filter: -1 | 0 | 1,
): Int32Array {
    const transparentTextureIds =
        filter === -1
            ? EMPTY_I32
            : collectTransparentTextureIds(model, textureLoader);
    return rawBuilder(
        model.faceColors3,
        model.faceAlphas ?? EMPTY_I8,
        model.faceRenderPriorities ?? EMPTY_I8,
        model.faceRenderLayers ?? EMPTY_U8,
        model.faceTextures ?? EMPTY_I16,
        transparentTextureIds,
        filter,
    );
}

function createModelFaceBuilder(rawBuilder: RawFaceBuilder): ModelFaceBuilder {
    return (
        model: Model,
        textureLoader: TextureLoader | undefined,
        filter: -1 | 0 | 1,
    ): PreparedModelFace[] => decodeFaces(
        buildRawFacePacket(rawBuilder, model, textureLoader, filter),
    );
}

let faceBuilderPromise: Promise<ModelFaceBuilder | undefined> | undefined;
let rawFaceBuilder: RawFaceBuilder | undefined;
let faceBuilder: ModelFaceBuilder | undefined;
let warnedAboutFallback = false;

export function buildModelFacesIfReady(
    model: Model,
    textureLoader: TextureLoader | undefined,
    filter: -1 | 0 | 1,
): PreparedModelFace[] | undefined {
    return faceBuilder?.(model, textureLoader, filter);
}

export function buildModelFacePacketIfReady(
    model: Model,
    textureLoader: TextureLoader | undefined,
    filter: -1 | 0 | 1,
): Int32Array | undefined {
    return rawFaceBuilder
        ? buildRawFacePacket(rawFaceBuilder, model, textureLoader, filter)
        : undefined;
}

export async function getModelFaceBuilder(): Promise<ModelFaceBuilder | undefined> {
    if (!faceBuilderPromise) {
        faceBuilderPromise = loadRustRendererModule()
            .then((module) => {
                const rawBuilder = module.build_model_faces;
                if (typeof rawBuilder !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export build_model_faces",
                    );
                }
                rawFaceBuilder = rawBuilder;
                faceBuilder = createModelFaceBuilder(rawBuilder);
                return faceBuilder;
            })
            .catch((error) => {
                if (!warnedAboutFallback) {
                    warnedAboutFallback = true;
                    console.warn(
                        "[RustFacePreparation] Rust face builder unavailable; "
                        + "using TypeScript face filtering.",
                        error,
                    );
                }
                return undefined;
            });
    }
    return faceBuilderPromise;
}
