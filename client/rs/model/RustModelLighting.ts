import {
    recordRustStage5Attempt,
    recordRustStage5Fallback,
    recordRustStage5Success,
} from "./RustStage5Ownership";

export type RustModelNormalCalculator = (
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    usedVertexCount: number,
    indices1: Int32Array,
    indices2: Int32Array,
    indices3: Int32Array,
    faceRenderTypes: Int8Array,
) => Int32Array;

export type RustModelFaceLighter = (
    indices1: Int32Array,
    indices2: Int32Array,
    indices3: Int32Array,
    faceColors: Uint16Array,
    faceRenderTypes: Int8Array,
    faceAlphas: Int8Array,
    faceTextures: Int16Array,
    vertexNormals: Int32Array,
    mergedNormals: Int32Array,
    faceNormals: Int32Array,
    ambient: number,
    contrast: number,
    lightX: number,
    lightY: number,
    lightZ: number,
) => Int32Array;

export type RustModelNormalsPacket = {
    usedVertexCount: number;
    faceCount: number;
    vertexNormals: Int32Array;
    faceNormals: Int32Array;
};

let normalCalculator: RustModelNormalCalculator | undefined;
let faceLighter: RustModelFaceLighter | undefined;
let warnedAboutNormalFailure = false;
let warnedAboutLightingFailure = false;

const EMPTY_I8 = new Int8Array(0);
const EMPTY_I16 = new Int16Array(0);
const EMPTY_I32 = new Int32Array(0);

export function registerRustModelNormalCalculator(
    calculator: RustModelNormalCalculator | undefined,
): void {
    normalCalculator = calculator;
}

export function registerRustModelFaceLighter(
    lighter: RustModelFaceLighter | undefined,
): void {
    faceLighter = lighter;
}

export function calculateModelNormalsWithRustIfReady(
    verticesX: Int32Array,
    verticesY: Int32Array,
    verticesZ: Int32Array,
    usedVertexCount: number,
    indices1: Int32Array,
    indices2: Int32Array,
    indices3: Int32Array,
    faceRenderTypes: Int8Array | undefined,
): RustModelNormalsPacket | undefined {
    recordRustStage5Attempt("normals");
    if (!normalCalculator) {
        recordRustStage5Fallback("normals", "backend unavailable");
        return undefined;
    }

    let packet: Int32Array;
    try {
        packet = normalCalculator(
            verticesX,
            verticesY,
            verticesZ,
            usedVertexCount | 0,
            indices1,
            indices2,
            indices3,
            faceRenderTypes ?? EMPTY_I8,
        );
    } catch (error) {
        if (!warnedAboutNormalFailure) {
            warnedAboutNormalFailure = true;
            console.warn(
                "[RustModelLighting] Rust normal generation failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("normals", "backend threw", true);
        return undefined;
    }

    if (packet.length < 2) {
        recordRustStage5Fallback("normals", "result header missing", true);
        return undefined;
    }
    const resultUsedVertexCount = packet[0] | 0;
    const faceCount = packet[1] | 0;
    const expectedUsedVertexCount = Math.min(
        usedVertexCount | 0,
        verticesX.length,
        verticesY.length,
        verticesZ.length,
    );
    const expectedFaceCount = indices1.length;
    const vertexNormalLength = resultUsedVertexCount * 4;
    const faceNormalLength = faceCount * 4;
    if (
        resultUsedVertexCount !== expectedUsedVertexCount
        || faceCount !== expectedFaceCount
        || packet.length !== 2 + vertexNormalLength + faceNormalLength
    ) {
        recordRustStage5Fallback("normals", "invalid result packet", true);
        return undefined;
    }

    const vertexStart = 2;
    const faceStart = vertexStart + vertexNormalLength;
    recordRustStage5Success("normals");
    return {
        usedVertexCount: resultUsedVertexCount,
        faceCount,
        vertexNormals: packet.slice(vertexStart, faceStart),
        faceNormals: packet.slice(faceStart),
    };
}

export function lightModelFacesWithRustIfReady(
    indices1: Int32Array,
    indices2: Int32Array,
    indices3: Int32Array,
    faceColors: Uint16Array,
    faceRenderTypes: Int8Array | undefined,
    faceAlphas: Int8Array | undefined,
    faceTextures: Int16Array | undefined,
    vertexNormals: Int32Array,
    mergedNormals: Int32Array | undefined,
    faceNormals: Int32Array,
    ambient: number,
    contrast: number,
    lightX: number,
    lightY: number,
    lightZ: number,
): Int32Array | undefined {
    recordRustStage5Attempt("lighting");
    if (!faceLighter) {
        recordRustStage5Fallback("lighting", "backend unavailable");
        return undefined;
    }

    let result: Int32Array;
    try {
        result = faceLighter(
            indices1,
            indices2,
            indices3,
            faceColors,
            faceRenderTypes ?? EMPTY_I8,
            faceAlphas ?? EMPTY_I8,
            faceTextures ?? EMPTY_I16,
            vertexNormals,
            mergedNormals ?? EMPTY_I32,
            faceNormals,
            ambient | 0,
            contrast | 0,
            lightX | 0,
            lightY | 0,
            lightZ | 0,
        );
    } catch (error) {
        if (!warnedAboutLightingFailure) {
            warnedAboutLightingFailure = true;
            console.warn(
                "[RustModelLighting] Rust model lighting failed; using TypeScript fallback.",
                error,
            );
        }
        recordRustStage5Fallback("lighting", "backend threw", true);
        return undefined;
    }

    if (result.length !== indices1.length * 3) {
        recordRustStage5Fallback("lighting", "invalid result length", true);
        return undefined;
    }
    recordRustStage5Success("lighting");
    return result;
}
