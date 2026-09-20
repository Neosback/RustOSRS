import type {
    App as PicoApp,
    DrawCall,
    Program,
    Texture,
    VertexArray,
    VertexBuffer,
} from "picogl";

import type { Overlay, OverlayInitArgs, OverlayUpdateArgs } from "../../../ui/devoverlay/Overlay";
import { RenderPhase } from "../../../ui/devoverlay/Overlay";

export type MapIconGroundEntry = {
    spriteId: number;
    sprite: HTMLCanvasElement;
    tileX: number;
    tileY: number;
    plane: number;
};

const VERT_SRC = `#version 300 es
layout(std140, column_major) uniform;
precision highp float;
uniform SceneUniforms {
    mat4 u_viewProjMatrix;
    mat4 u_viewMatrix;
    mat4 u_projectionMatrix;
    vec4 u_skyColor;
    vec4 u_sceneHslOverride;
    vec2 u_cameraPos;
    vec2 u_playerPos;
    float u_renderDistance;
    float u_fogDepth;
    float u_currentTime;
    float u_brightness;
    float u_colorBanding;
    float u_isNewTextureAnim;
};
layout(location=0) in vec3 a_position;
layout(location=1) in vec2 a_texCoord;
out vec2 v_uv;
out float v_fogAmount;
void main() {
    gl_Position = u_projectionMatrix * (u_viewMatrix * vec4(a_position, 1.0));
    v_uv = a_texCoord;
    float distanceFromPlayer = max(
        abs(a_position.x - u_playerPos.x),
        abs(a_position.z - u_playerPos.y)
    );
    float fogStart = min(u_fogDepth, u_renderDistance);
    float fogWidth = max(u_renderDistance - fogStart, 0.0001);
    v_fogAmount = clamp((distanceFromPlayer - fogStart) / fogWidth, 0.0, 1.0);
}`;

const FRAG_SRC = `#version 300 es
precision mediump float;
in vec2 v_uv;
in float v_fogAmount;
uniform sampler2D u_sprite;
out vec4 fragColor;
void main() {
    fragColor = texture(u_sprite, v_uv);
    fragColor.a *= 1.0 - smoothstep(0.0, 1.0, v_fogAmount);
    if (fragColor.a < 0.01) discard;
}`;

const UVS = new Float32Array([0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]);
export function buildMapIconGroundVertices(
    entry: Pick<MapIconGroundEntry, "tileX" | "tileY" | "plane">,
    sampleHeight: (x: number, y: number, plane: number) => number,
    out = new Float32Array(18),
): Float32Array {
    const { tileX: x, tileY: y, plane } = entry;
    const a = sampleHeight(x, y, plane) - 0.015;
    const b = sampleHeight(x + 1, y, plane) - 0.015;
    const c = sampleHeight(x + 1, y + 1, plane) - 0.015;
    const d = sampleHeight(x, y + 1, plane) - 0.015;
    out.set([
        x, a, y,
        x + 1, b, y,
        x + 1, c, y + 1,
        x, a, y,
        x + 1, c, y + 1,
        x, d, y + 1,
    ]);
    return out;
}

export class MapIconGroundOverlay implements Overlay {
    private app?: PicoApp;
    private gl?: WebGL2RenderingContext;
    private positions?: VertexBuffer;
    private uvs?: VertexBuffer;
    private array?: VertexArray;
    private drawCall?: DrawCall;
    private program?: Program;
    private args?: OverlayUpdateArgs;
    private entries: readonly MapIconGroundEntry[] = [];
    private textures = new Map<number, Texture>();
    private vertices = new Float32Array(18);

    setEntries(entries: readonly MapIconGroundEntry[]): void {
        this.entries = entries;
    }

    init({ app, sceneUniforms }: OverlayInitArgs): void {
        this.app = app;
        this.gl = app.gl as WebGL2RenderingContext;
        this.positions = app.createVertexBuffer(this.gl.FLOAT, 3, new Float32Array(18));
        this.uvs = app.createVertexBuffer(this.gl.FLOAT, 2, UVS);
        this.array = app
            .createVertexArray()
            .vertexAttributeBuffer(0, this.positions)
            .vertexAttributeBuffer(1, this.uvs);
        this.program = app.createProgram(VERT_SRC, FRAG_SRC);
        this.drawCall = app
            .createDrawCall(this.program, this.array)
            .uniformBlock("SceneUniforms", sceneUniforms)
            .primitive(this.gl.TRIANGLES);
    }

    update(args: OverlayUpdateArgs): void {
        this.args = args;
    }

    draw(phase: RenderPhase): void {
        if (
            phase !== RenderPhase.ToSceneFramebuffer ||
            !this.app ||
            !this.gl ||
            !this.args ||
            !this.positions ||
            !this.drawCall
        ) {
            return;
        }
        this.app.enable(this.gl.DEPTH_TEST);
        this.app.depthMask(false);
        this.app.disable(this.gl.CULL_FACE as any);
        this.app.enable(this.gl.BLEND);
        for (const entry of this.entries) {
            let texture = this.textures.get(entry.spriteId);
            if (!texture) {
                texture = this.app.createTexture2D(entry.sprite as any, {
                    flipY: false,
                    minFilter: this.gl.NEAREST,
                    magFilter: this.gl.NEAREST,
                    wrapS: this.gl.CLAMP_TO_EDGE,
                    wrapT: this.gl.CLAMP_TO_EDGE,
                });
                this.textures.set(entry.spriteId, texture);
            }
            this.positions.data(
                buildMapIconGroundVertices(
                    entry,
                    this.args.helpers.sampleHeightAtExactPlane,
                    this.vertices,
                ),
            );
            this.drawCall.texture("u_sprite", texture).draw();
        }
        this.app.depthMask(true);
        this.app.disable(this.gl.BLEND);
    }

    dispose(): void {
        this.positions?.delete?.();
        this.uvs?.delete?.();
        this.array?.delete?.();
        this.drawCall?.delete?.();
        this.program?.delete?.();
        for (const texture of this.textures.values()) texture.delete?.();
        this.textures.clear();
    }
}
