#version 300 es
precision highp float;
precision highp int;
precision highp usampler2D;
precision highp isampler2DArray;

layout(location = 0) in uvec3 a_packed;

uniform mat4 u_viewMatrix;
uniform mat4 u_projectionMatrix;
uniform vec4 u_sceneHslOverride;
uniform vec2 u_playerPos;
uniform float u_renderDistance;
uniform float u_fogDepth;
uniform float u_currentTime;
uniform float u_brightness;

uniform int u_drawId;
uniform vec2 u_mapPos;
uniform float u_timeLoaded;
uniform float u_roofPlaneLimit;
uniform int u_sceneBorderSize;

uniform highp usampler2D u_modelInfoTexture;
uniform mediump isampler2DArray u_heightMap;

out vec4 v_color;
out float v_fogAmount;
out vec3 v_worldPos;
flat out float v_plane;

const float CONTOUR_GROUND_CENTER_TILE = 0.0;
const float CONTOUR_GROUND_VERTEX = 1.0;
const float CONTOUR_GROUND_NONE = 2.0;
const int TILE_SIZE = 128;
const int TILE_SIZE_SHIFT = 7;

float whenEq(float x, float y) {
    return 1.0 - abs(sign(x - y));
}

int applyHslOverride(int hsl, vec4 overrideValue) {
    if (overrideValue.w <= 0.0) {
        return hsl;
    }

    int hue = (hsl >> 10) & 63;
    int sat = (hsl >> 7) & 7;
    int lum = hsl & 127;
    int amount = int(overrideValue.w);

    if (overrideValue.x >= 0.0) {
        hue += (amount * (int(overrideValue.x) - hue)) >> 7;
    }
    if (overrideValue.y >= 0.0) {
        sat += (amount * (int(overrideValue.y) - sat)) >> 7;
    }
    if (overrideValue.z >= 0.0) {
        lum += (amount * (int(overrideValue.z) - lum)) >> 7;
    }

    return (hue << 10) | (sat << 7) | lum;
}

vec3 hslToRgb(int hsl, float brightness) {
    const float oneThird = 1.0 / 3.0;
    const float twoThird = 2.0 / 3.0;
    const float six = 6.0;

    float hue = float(hsl >> 10) / 64.0 + 0.0078125;
    float sat = float((hsl >> 7) & 7) / 8.0 + 0.0625;
    float lum = float(hsl & 127) / 128.0;

    vec3 xt;
    if (hue < oneThird) {
        xt = vec3(six * (oneThird - hue), six * hue, 0.0);
    } else if (hue < twoThird) {
        xt = vec3(0.0, six * (twoThird - hue), six * (hue - oneThird));
    } else {
        xt = vec3(six * (hue - twoThird), 0.0, six * (1.0 - hue));
    }
    xt = min(xt, 1.0);

    float sat2 = 2.0 * sat;
    float satInv = 1.0 - sat;
    float lumInv = 1.0 - lum;
    float lum2m1 = 2.0 * lum - 1.0;
    vec3 ct = sat2 * xt + satInv;
    vec3 rgb = lum < 0.5 ? lum * ct : lumInv * ct + lum2m1;
    return pow(max(rgb, vec3(0.0)), vec3(brightness));
}

float unpackFloat11(uint value) {
    return 16.0 - float(value) / 64.0;
}

struct Vertex {
    vec3 pos;
    vec4 color;
    vec2 texCoord;
    uint textureId;
    uint priority;
};

Vertex decodeVertex(uvec3 packed) {
    uint v0 = packed.x;
    uint v1 = packed.y;
    uint v2 = packed.z;

    float x = float(int((v0 >> 17u) & 0x7fffu) - 0x4000);
    float u = unpackFloat11(((v0 >> 11u) & 0x3fu) | ((v2 & 0x1fu) << 6u));
    float v = unpackFloat11(v0 & 0x7ffu);

    float y = -float(int(v1 & 0x7fffu) - 0x4000);
    int hsl = int((v1 >> 15u) & 0xffffu);
    bool textured = ((v1 >> 31u) & 1u) != 0u;
    uint textureId = uint((hsl >> 7) | int(((v2 >> 5u) & 1u) << 9u)) * uint(textured);

    hsl = applyHslOverride(hsl, u_sceneHslOverride);

    float z = float(int((v2 >> 17u) & 0x7fffu) - 0x4000);
    float alpha = float((v2 >> 9u) & 0xffu) / 255.0;
    uint priority = (v2 >> 6u) & 0x7u;

    vec3 rgb = textured
        ? vec3(float(hsl & 0x7f) / 127.0)
        : hslToRgb(hsl, u_brightness);

    return Vertex(vec3(x, y, z), vec4(rgb, alpha), vec2(u, v), textureId, priority);
}

struct ModelInfo {
    vec2 tilePos;
    uint height;
    uint plane;
    uint planeCullLevel;
    uint priority;
    float contourGround;
};

ivec2 dataTexCoord(int index) {
    return ivec2(index % 16, index / 16);
}

ModelInfo decodeModelInfo(int offset) {
    uvec4 data = texelFetch(
        u_modelInfoTexture,
        dataTexCoord(offset + gl_InstanceID),
        0
    );

    ModelInfo info;
    info.tilePos = vec2(float(data.r & 0x3fffu), float(data.g & 0x3fffu));
    info.height = (data.b >> 8u) * 8u;
    info.plane = data.r >> 14u;
    info.planeCullLevel = (data.b >> 6u) & 0x3u;
    info.priority = data.b & 0x7u;
    info.contourGround = float((data.g >> 14u) & 0x3u);
    return info;
}

int getTileHeight(int x, int z, uint plane) {
    return texelFetch(
        u_heightMap,
        ivec3(u_sceneBorderSize + x, u_sceneBorderSize + z, int(plane)),
        0
    ).r * 8;
}

float getHeightInterp(vec2 pos, uint plane) {
    ivec2 ipos = ivec2(pos);
    int tileX = ipos.x >> TILE_SIZE_SHIFT;
    int tileZ = ipos.y >> TILE_SIZE_SHIFT;
    int offsetX = ipos.x & (TILE_SIZE - 1);
    int offsetZ = ipos.y & (TILE_SIZE - 1);

    int hSW = getTileHeight(tileX, tileZ, plane);
    int hSE = getTileHeight(tileX + 1, tileZ, plane);
    int hNW = getTileHeight(tileX, tileZ + 1, plane);
    int hNE = getTileHeight(tileX + 1, tileZ + 1, plane);

    int h0;
    if (offsetX + offsetZ <= TILE_SIZE) {
        h0 = (hSW * TILE_SIZE
            + (hSE - hSW) * offsetX
            + (hNW - hSW) * offsetZ) >> TILE_SIZE_SHIFT;
    } else {
        int rx = TILE_SIZE - offsetX;
        int rz = TILE_SIZE - offsetZ;
        h0 = (hNE * TILE_SIZE
            + (hNW - hNE) * rx
            + (hSE - hNE) * rz) >> TILE_SIZE_SHIFT;
    }

    int h1;
    if (offsetX <= offsetZ) {
        h1 = (hSW * TILE_SIZE
            + (hNW - hSW) * offsetZ
            + (hNE - hNW) * offsetX) >> TILE_SIZE_SHIFT;
    } else {
        h1 = (hSW * TILE_SIZE
            + (hSE - hSW) * offsetX
            + (hNE - hSE) * offsetZ) >> TILE_SIZE_SHIFT;
    }

    return float(max(h0, h1));
}

float fogFactorOsrs(vec2 playerOffset) {
    float fogStart = min(u_fogDepth, u_renderDistance);
    float fogEnd = max(u_renderDistance, fogStart + 0.0001);
    vec2 distanceToEdge = abs(playerOffset) - vec2(fogEnd);
    float distance = max(distanceToEdge.x, distanceToEdge.y);
    return clamp(distance / (fogEnd - fogStart) + 1.0, 0.0, 1.0);
}

void main() {
    int instanceOffset = int(texelFetch(
        u_modelInfoTexture,
        dataTexCoord(u_drawId),
        0
    ).r);

    Vertex vertex = decodeVertex(a_packed);
    ModelInfo modelInfo = decodeModelInfo(instanceOffset);

    v_plane = float(modelInfo.plane);

    if (float(modelInfo.planeCullLevel) > u_roofPlaneLimit + 0.5) {
        v_color = vec4(0.0);
        v_fogAmount = 1.0;
        v_worldPos = vec3(0.0);
        gl_Position = vec4(0.0);
        return;
    }

    vec3 localPos = vertex.pos + vec3(modelInfo.tilePos.x, 0.0, modelInfo.tilePos.y);

    vec2 interpPos =
        modelInfo.tilePos * whenEq(modelInfo.contourGround, CONTOUR_GROUND_CENTER_TILE)
        + localPos.xz * whenEq(modelInfo.contourGround, CONTOUR_GROUND_VERTEX);

    localPos.y -= float(modelInfo.height);
    if (modelInfo.contourGround < CONTOUR_GROUND_NONE) {
        localPos.y -= getHeightInterp(interpPos, modelInfo.plane);
    }

    localPos /= 128.0;
    localPos += vec3(u_mapPos.x, 0.0, u_mapPos.y) * 64.0;
    v_worldPos = localPos;

    float loadAlpha = smoothstep(
        0.0,
        1.0,
        min(u_currentTime - u_timeLoaded, 1.0)
    );
    float loadingFog = max(1.0 - loadAlpha, fogFactorOsrs(localPos.xz - u_playerPos));
    v_fogAmount = loadAlpha < 1.0
        ? loadingFog
        : fogFactorOsrs(localPos.xz - u_playerPos);

    vec4 viewPos = u_viewMatrix * vec4(localPos, 1.0);

    // Preserve the current renderer's static-scene depth layering.
    viewPos.z += float(modelInfo.plane) * 0.001;
    uint modelPriority = modelInfo.priority & 0x7u;
    if (modelPriority > 0u) {
        viewPos.z += float(modelPriority) * 0.001;
    }
    uint facePriority = vertex.priority & 0x7u;
    if (facePriority > 0u) {
        viewPos.z += float(facePriority) * 0.001;
    }

    v_color = vertex.color;
    gl_Position = u_projectionMatrix * viewPos;
}
