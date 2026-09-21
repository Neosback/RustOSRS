import {
    createModelInfoTextureData,
    type DrawCommand,
} from "../buffer/SceneBuffer";
import type { VertexBatchBuilder } from "../buffer/VertexBuffer";
import { loadRustRendererModule } from "./RustRendererModule";

export type ModelInfoTextureBuilder = (commands: DrawCommand[]) => Uint16Array;
export type VertexBatchBuilderFactory = () => VertexBatchBuilder | undefined;

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
let warnedAboutVertexFallback = false;

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
                return (): VertexBatchBuilder => new RustVertexBufferBuilder();
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
                return (): undefined => undefined;
            });
    }

    return vertexBatchBuilderFactoryPromise;
}
