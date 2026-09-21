import { newDrawRange, type DrawRange } from "../DrawRange";
import {
    createModelInfoTextureData,
    type DrawCommand,
} from "../buffer/SceneBuffer";
import type { Model } from "../../rs/model/Model";
import { getModelHash, type ModelHashBuffer } from "../buffer/ModelHashBuffer";
import type { VertexBatchBuilder } from "../buffer/VertexBuffer";
import { loadRustRendererModule } from "./RustRendererModule";

export type ModelInfoTextureBuilder = (commands: DrawCommand[]) => Uint16Array;
export type VertexBatchBuilderFactory = () => VertexBatchBuilder | undefined;
export type ModelHasher = (model: Model) => number;
export type PreparedDrawList = {
    ranges: DrawRange[];
    planes: Uint8Array;
};
export type DrawListBuilder = (commands: DrawCommand[]) => PreparedDrawList;

type RustModelInfoPacketBuilder = (
    commandInstanceCounts: Uint32Array,
    instanceFields: Int32Array,
) => Uint16Array;

export type FlatModelInfoPacket = {
    commandInstanceCounts: Uint32Array;
    instanceFields: Int32Array;
};

const MODEL_INFO_FIELD_STRIDE = 9;

export function flattenModelInfoCommands(commands: DrawCommand[]): FlatModelInfoPacket {
    const commandInstanceCounts = new Uint32Array(commands.length);

    let instanceCount = 0;
    for (let commandIndex = 0; commandIndex < commands.length; commandIndex++) {
        const count = commands[commandIndex].instances.length;
        commandInstanceCounts[commandIndex] = count;
        instanceCount += count;
    }

    const instanceFields = new Int32Array(instanceCount * MODEL_INFO_FIELD_STRIDE);
    let offset = 0;
    for (const command of commands) {
        for (const instance of command.instances) {
            instanceFields[offset++] = instance.sceneX | 0;
            instanceFields[offset++] = instance.sceneZ | 0;
            instanceFields[offset++] = instance.heightOffset | 0;
            instanceFields[offset++] = instance.level | 0;
            instanceFields[offset++] = (instance.planeCullLevel ?? instance.level) | 0;
            instanceFields[offset++] = instance.contourGround | 0;
            instanceFields[offset++] = instance.priority | 0;
            instanceFields[offset++] = instance.interactType | 0;
            instanceFields[offset++] = instance.interactId | 0;
        }
    }

    return {
        commandInstanceCounts,
        instanceFields,
    };
}

export function buildModelInfoTextureDataWithRust(
    commands: DrawCommand[],
    rustBuilder: RustModelInfoPacketBuilder,
): Uint16Array {
    const { commandInstanceCounts, instanceFields } = flattenModelInfoCommands(commands);
    return rustBuilder(commandInstanceCounts, instanceFields);
}

let modelInfoBuilderPromise: Promise<ModelInfoTextureBuilder> | undefined;
let warnedAboutFallback = false;

export async function getModelInfoTextureBuilder(): Promise<ModelInfoTextureBuilder> {
    if (!modelInfoBuilderPromise) {
        modelInfoBuilderPromise = loadRustRendererModule()
            .then((module) => {
                const rustBuilder = module.build_model_info_texture_data;
                if (typeof rustBuilder !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export build_model_info_texture_data",
                    );
                }

                return (commands: DrawCommand[]): Uint16Array =>
                    buildModelInfoTextureDataWithRust(commands, rustBuilder);
            })
            .catch((error) => {
                if (!warnedAboutFallback) {
                    warnedAboutFallback = true;
                    console.warn(
                        "[RustGeometryPreparation] Rust model-info builder unavailable; "
                        + "using the TypeScript compatibility builder.",
                        error,
                    );
                }
                return createModelInfoTextureData;
            });
    }

    return modelInfoBuilderPromise;
}


let vertexBatchBuilderFactoryPromise: Promise<VertexBatchBuilderFactory> | undefined;
let vertexBatchBuilderFactory: VertexBatchBuilderFactory | undefined;
let warnedAboutVertexFallback = false;

export function createVertexBatchBuilderIfReady(): VertexBatchBuilder | undefined {
    return vertexBatchBuilderFactory?.();
}

export async function getVertexBatchBuilderFactory(): Promise<VertexBatchBuilderFactory> {
    if (!vertexBatchBuilderFactoryPromise) {
        vertexBatchBuilderFactoryPromise = loadRustRendererModule()
            .then((module) => {
                const RustVertexBufferBuilder = module.RustVertexBufferBuilder;
                if (typeof RustVertexBufferBuilder !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export RustVertexBufferBuilder",
                    );
                }
                vertexBatchBuilderFactory = (): VertexBatchBuilder => new RustVertexBufferBuilder();
                return vertexBatchBuilderFactory;
            })
            .catch((error) => {
                if (!warnedAboutVertexFallback) {
                    warnedAboutVertexFallback = true;
                    console.warn(
                        "[RustGeometryPreparation] Rust vertex builder unavailable; "
                        + "using the TypeScript compatibility packer.",
                        error,
                    );
                }
                vertexBatchBuilderFactory = (): undefined => undefined;
                return vertexBatchBuilderFactory;
            });
    }

    return vertexBatchBuilderFactoryPromise;
}


type RustModelHasher = (
    faceColors1: Int32Array,
    faceColors2: Int32Array,
    faceColors3: Int32Array,
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    textureIds: Int32Array,
) => number;

let rustModelHasherPromise: Promise<RustModelHasher | undefined> | undefined;
let warnedAboutModelHashFallback = false;
const EMPTY_TEXTURE_IDS = new Int32Array(0);

async function getRustModelHasher(): Promise<RustModelHasher | undefined> {
    if (!rustModelHasherPromise) {
        rustModelHasherPromise = loadRustRendererModule()
            .then((module) => {
                const rustHasher = module.hash_model_geometry;
                if (typeof rustHasher !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export hash_model_geometry",
                    );
                }
                return rustHasher;
            })
            .catch((error) => {
                if (!warnedAboutModelHashFallback) {
                    warnedAboutModelHashFallback = true;
                    console.warn(
                        "[RustGeometryPreparation] Rust model hasher unavailable; "
                        + "using the TypeScript compatibility hasher.",
                        error,
                    );
                }
                return undefined;
            });
    }
    return rustModelHasherPromise;
}

export async function getModelHasher(
    fallbackBuffer: ModelHashBuffer,
): Promise<ModelHasher> {
    const rustHasher = await getRustModelHasher();
    if (!rustHasher) {
        return (model: Model): number => getModelHash(fallbackBuffer, model);
    }

    return (model: Model): number => {
        const textureIds = model.faceTextures
            ? Int32Array.from(model.faceTextures)
            : EMPTY_TEXTURE_IDS;
        return rustHasher(
            model.faceColors1,
            model.faceColors2,
            model.faceColors3,
            model.verticesX,
            model.verticesY,
            model.verticesZ,
            textureIds,
        ) >>> 0;
    };
}


const DRAW_COMMAND_FIELD_STRIDE = 4;

export function flattenDrawCommands(commands: DrawCommand[]): Uint32Array {
    const fields = new Uint32Array(commands.length * DRAW_COMMAND_FIELD_STRIDE);
    let offset = 0;
    for (const command of commands) {
        const instance = command.instances[0];
        const plane = instance
            ? (instance.planeCullLevel ?? instance.level) | 0
            : 0;
        fields[offset++] = command.offset >>> 0;
        fields[offset++] = command.elements >>> 0;
        fields[offset++] = command.instances.length >>> 0;
        fields[offset++] = plane >>> 0;
    }
    return fields;
}

export function buildDrawListWithTypeScript(commands: DrawCommand[]): PreparedDrawList {
    const planes = new Uint8Array(commands.length);
    const ranges = commands.map((command, index) => {
        const instance = command.instances[0];
        planes[index] = instance
            ? ((instance.planeCullLevel ?? instance.level) & 0xff)
            : 0;
        return newDrawRange(
            command.offset,
            command.elements,
            command.instances.length,
        );
    });
    return { ranges, planes };
}

let drawListBuilderPromise: Promise<DrawListBuilder> | undefined;
let drawListBuilder: DrawListBuilder | undefined;
let warnedAboutDrawListFallback = false;

export function buildDrawListIfReady(commands: DrawCommand[]): PreparedDrawList {
    return drawListBuilder
        ? drawListBuilder(commands)
        : buildDrawListWithTypeScript(commands);
}

export async function getDrawListBuilder(): Promise<DrawListBuilder> {
    if (!drawListBuilderPromise) {
        drawListBuilderPromise = loadRustRendererModule()
            .then((module) => {
                const rustBuilder = module.build_draw_list;
                if (typeof rustBuilder !== "function") {
                    throw new Error(
                        "Rust renderer web package does not export build_draw_list",
                    );
                }

                drawListBuilder = (commands: DrawCommand[]): PreparedDrawList => {
                    const prepared = rustBuilder(flattenDrawCommands(commands));
                    try {
                        const flatRanges = prepared.flat_ranges();
                        const planes = prepared.planes();
                        if (flatRanges.length !== commands.length * 3) {
                            throw new Error(
                                `Rust draw list returned ${flatRanges.length} range values for ${commands.length} commands`,
                            );
                        }
                        const ranges = new Array<DrawRange>(commands.length);
                        for (let index = 0; index < commands.length; index++) {
                            const base = index * 3;
                            ranges[index] = newDrawRange(
                                flatRanges[base],
                                flatRanges[base + 1],
                                flatRanges[base + 2],
                            );
                        }
                        return { ranges, planes };
                    } finally {
                        prepared.free?.();
                    }
                };
                return drawListBuilder;
            })
            .catch((error) => {
                if (!warnedAboutDrawListFallback) {
                    warnedAboutDrawListFallback = true;
                    console.warn(
                        "[RustGeometryPreparation] Rust draw-list builder unavailable; "
                        + "using the TypeScript compatibility builder.",
                        error,
                    );
                }
                drawListBuilder = buildDrawListWithTypeScript;
                return drawListBuilder;
            });
    }

    return drawListBuilderPromise;
}
