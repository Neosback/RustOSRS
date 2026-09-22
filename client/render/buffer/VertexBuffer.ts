import { FloatUtil } from "../../common/utils/FloatUtil";
import { clamp } from "../../common/utils/MathUtil";
import { DataBuffer } from "../../game/buffer/DataBuffer";

export const VERTEX_BATCH_FLAG_REUSE = 1;
export const VERTEX_BATCH_FLAG_PRIORITY_IS_PACKED = 2;

export interface VertexBatchBuilder {
    clear(): void;
    vertex_count(): number;
    push_batch(
        integerFields: Int32Array,
        uvFields: Float32Array,
        flags: Uint8Array,
    ): Uint32Array;
    set_texture_id_map?(
        textureIds: Int32Array,
        textureIndices: Int32Array,
    ): void;
    used_texture_ids?(): Int32Array;
    push_terrain_tile?(
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
    push_terrain_batch?(
        tileVertexOffsets: Uint32Array,
        tileFaceOffsets: Uint32Array,
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
        tileX: Int32Array,
        tileZ: Int32Array,
        offsetX: number,
        offsetZ: number,
    ): Uint32Array;
    push_model_faces?(
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

export class VertexBuffer extends DataBuffer {
    static readonly STRIDE = 12;
    static readonly INTEGER_FIELD_STRIDE = 7;
    static readonly UV_FIELD_STRIDE = 2;

    // The complete packed vertex is the cache identity. A numeric hash based
    // on multiplying its fields loses precision in JavaScript and can merge
    // unrelated vertices, creating stray stretched triangles in the scene.
    vertexIndices: Map<string, number>;

    constructor(
        count: number,
        private readonly rustBuilder?: VertexBatchBuilder,
    ) {
        super(VertexBuffer.STRIDE, count);
        this.vertexIndices = new Map();
    }

    // Compress OSRS face priorities (typically 0..11) into 3 bits (0..7)
    // while preserving useful ordering bands. Collapses 4..7 into two bands
    // and 8..11 into the top two bands so overlays remain clearly above base.
    private static compressPriority(p: number): number {
        if (p < 0) return 0;
        if (p <= 3) return p; // 0..3 -> 0..3
        if (p <= 7) return 4 + ((p - 4) >> 1); // 4..5 -> 4, 6..7 -> 5
        return 6 + ((p - 8) >> 1); // 8..9 -> 6, 10..11 -> 7
    }

    setTextureIdMap(textureIdIndexMap: Map<number, number>): void {
        if (!this.rustBuilder?.set_texture_id_map) {
            return;
        }

        const textureIds = new Int32Array(textureIdIndexMap.size);
        const textureIndices = new Int32Array(textureIdIndexMap.size);
        let offset = 0;
        for (const [textureId, textureIndex] of textureIdIndexMap) {
            textureIds[offset] = textureId;
            textureIndices[offset] = textureIndex;
            offset++;
        }
        this.rustBuilder.set_texture_id_map(textureIds, textureIndices);
    }

    rustUsedTextureIds(): Int32Array {
        return this.rustBuilder?.used_texture_ids?.() ?? new Int32Array(0);
    }

    hasRustTerrainBuilder(): boolean {
        return typeof this.rustBuilder?.push_terrain_tile === "function";
    }

    hasRustTerrainBatchBuilder(): boolean {
        return typeof this.rustBuilder?.push_terrain_batch === "function";
    }

    addTerrainBatch(
        tileVertexOffsets: Uint32Array,
        tileFaceOffsets: Uint32Array,
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
        tileX: Int32Array,
        tileZ: Int32Array,
        offsetX: number,
        offsetZ: number,
    ): Uint32Array | undefined {
        if (!this.rustBuilder?.push_terrain_batch) {
            return undefined;
        }

        const indices = this.rustBuilder.push_terrain_batch(
            tileVertexOffsets,
            tileFaceOffsets,
            verticesX,
            verticesY,
            verticesZ,
            facesA,
            facesB,
            facesC,
            colorsA,
            colorsB,
            colorsC,
            textureIds,
            tileX,
            tileZ,
            offsetX,
            offsetZ,
        );
        this.offset = this.rustBuilder.vertex_count();
        return indices;
    }

    addTerrainTile(
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
    ): Uint32Array | undefined {
        if (!this.rustBuilder?.push_terrain_tile) {
            return undefined;
        }

        const indices = this.rustBuilder.push_terrain_tile(
            verticesX,
            verticesY,
            verticesZ,
            facesA,
            facesB,
            facesC,
            colorsA,
            colorsB,
            colorsC,
            textureIds,
            tileX,
            tileZ,
            offsetX,
            offsetZ,
        );
        this.offset = this.rustBuilder.vertex_count();
        return indices;
    }

    hasRustModelFaceBuilder(): boolean {
        return typeof this.rustBuilder?.push_model_faces === "function";
    }

    addModelFaces(
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
    ): Uint32Array | undefined {
        if (!this.rustBuilder?.push_model_faces) {
            return undefined;
        }

        const indices = this.rustBuilder.push_model_faces(
            verticesX,
            verticesY,
            verticesZ,
            facesA,
            facesB,
            facesC,
            colorsA,
            colorsB,
            colorsC,
            uvs,
            faceFields,
            sceneX,
            sceneHeight,
            sceneZ,
            overrideHue,
            overrideSaturation,
            overrideLuminance,
            overrideAmount,
            reuseVertices,
        );
        this.offset = this.rustBuilder.vertex_count();
        return indices;
    }

    addBatch(
        integerFields: Int32Array,
        uvFields: Float32Array,
        flags: Uint8Array,
    ): Uint32Array {
        if (integerFields.length % VertexBuffer.INTEGER_FIELD_STRIDE !== 0) {
            throw new Error(
                `Vertex integer field packet must contain stride-${VertexBuffer.INTEGER_FIELD_STRIDE} records`,
            );
        }
        const vertexCount = integerFields.length / VertexBuffer.INTEGER_FIELD_STRIDE;
        if (uvFields.length !== vertexCount * VertexBuffer.UV_FIELD_STRIDE) {
            throw new Error(
                `Vertex UV field packet has ${uvFields.length} values; expected ${vertexCount * VertexBuffer.UV_FIELD_STRIDE}`,
            );
        }
        if (flags.length !== vertexCount) {
            throw new Error(
                `Vertex flag packet has ${flags.length} values; expected ${vertexCount}`,
            );
        }

        if (this.rustBuilder) {
            const indices = this.rustBuilder.push_batch(integerFields, uvFields, flags);
            this.offset = this.rustBuilder.vertex_count();
            return indices;
        }

        const indices = new Uint32Array(vertexCount);
        for (let i = 0; i < vertexCount; i++) {
            const integerOffset = i * VertexBuffer.INTEGER_FIELD_STRIDE;
            const uvOffset = i * VertexBuffer.UV_FIELD_STRIDE;
            const recordFlags = flags[i];
            if (
                (recordFlags
                    & ~(VERTEX_BATCH_FLAG_REUSE | VERTEX_BATCH_FLAG_PRIORITY_IS_PACKED))
                !== 0
            ) {
                throw new Error(
                    `Vertex flags at record ${i} contain unsupported bits 0x${recordFlags.toString(16)}`,
                );
            }
            indices[i] = this.addVertex(
                integerFields[integerOffset],
                integerFields[integerOffset + 1],
                integerFields[integerOffset + 2],
                integerFields[integerOffset + 3],
                integerFields[integerOffset + 4],
                uvFields[uvOffset],
                uvFields[uvOffset + 1],
                integerFields[integerOffset + 5],
                (recordFlags & VERTEX_BATCH_FLAG_REUSE) !== 0,
                integerFields[integerOffset + 6],
                (recordFlags & VERTEX_BATCH_FLAG_PRIORITY_IS_PACKED) !== 0,
            );
        }
        return indices;
    }

    addVertex(
        x: number,
        y: number,
        z: number,
        hsl: number,
        alpha: number,
        u: number,
        v: number,
        textureId: number,
        reuseVertex: boolean = true,
        priority: number = 0,
        priorityIsPacked: boolean = false,
    ) {
        if (this.rustBuilder) {
            const flags =
                (reuseVertex ? VERTEX_BATCH_FLAG_REUSE : 0)
                | (priorityIsPacked ? VERTEX_BATCH_FLAG_PRIORITY_IS_PACKED : 0);
            const indices = this.rustBuilder.push_batch(
                new Int32Array([x, y, z, hsl, alpha, textureId, priority]),
                new Float32Array([u, v]),
                new Uint8Array([flags]),
            );
            this.offset = this.rustBuilder.vertex_count();
            return indices[0];
        }

        if (textureId >= 1024) {
            textureId = -1;
        }
        const isTextured = textureId !== -1;
        if (isTextured) {
            // textureId = 119;
            // only light
            hsl &= 127;
            hsl |= (textureId & 0x1ff) << 7;
        }

        const xPos = clamp(x + 0x4000, 0, 0x8000);
        const yPos = clamp(-y + 0x4000, 0, 0x8000);
        const zPos = clamp(z + 0x4000, 0, 0x8000);

        const uPacked = clamp(FloatUtil.packFloat11(u), 0, 0x7ff);
        const vPacked = clamp(FloatUtil.packFloat11(v), 0, 0x7ff);

        const v0 = (xPos << 17) | ((uPacked & 0x3f) << 11) | vPacked;

        const v1 = yPos | (hsl << 15) | (Number(isTextured) << 31);

        // Pack per-face priority (0-7, 3 bits) into bits [8:6] (previously unused)
        // Keep uPacked high bits at [4:0] and textureId bit at [5]
        const packedPriority = priorityIsPacked
            ? clamp(priority, 0, 7)
            : VertexBuffer.compressPriority(priority);
        const v2 =
            (zPos << 17) |
            (alpha << 9) |
            ((packedPriority & 0x7) << 6) |
            (((textureId >> 9) & 0x1) << 5) |
            (uPacked >> 6);

        if (reuseVertex) {
            const key = `${v0 >>> 0}:${v1 >>> 0}:${v2 >>> 0}`;
            const cachedIndex = this.vertexIndices.get(key);
            if (cachedIndex !== undefined) {
                return cachedIndex;
            } else {
                this.vertexIndices.set(key, this.offset);
            }
        }
        this.ensureSize(1);
        const byteOffset = this.byteOffset();

        this.view.setUint32(byteOffset, v0, true);
        this.view.setUint32(byteOffset + 4, v1, true);
        this.view.setUint32(byteOffset + 8, v2, true);

        return this.offset++;
    }

    reset(): void {
        this.offset = 0;
        this.vertexIndices.clear();
        this.rustBuilder?.clear();
    }

    byteArray(): Uint8Array {
        if (!this.rustBuilder) {
            return super.byteArray();
        }
        const packed = this.rustBuilder.packed_vertices();
        return new Uint8Array(
            packed.buffer,
            packed.byteOffset,
            packed.byteLength,
        );
    }
}
