#version 300 es
precision highp float;
precision highp int;
precision highp sampler2DArray;
precision highp isampler2D;

uniform vec4 u_skyColor;
uniform sampler2DArray u_textures;
uniform highp isampler2D u_textureMaterials;
uniform int u_textureLayerCount;
uniform int u_materialCount;
uniform int u_discardAlpha;
uniform float u_currentTime;
uniform float u_brightness;
uniform float u_colorBanding;

in vec4 v_color;
in vec2 v_texCoord;
in float v_fogAmount;
in vec3 v_worldPos;
flat in uint v_texId;
flat in float v_alphaCutOff;
flat in float v_plane;

out vec4 fragColor;

struct Material {
    int frameCount;
    int animSpeed;
};

Material getMaterial(uint textureId) {
    int maxMaterial = max(u_materialCount - 1, 0);
    int materialId = clamp(int(textureId), 0, maxMaterial);
    ivec4 row0 = texelFetch(u_textureMaterials, ivec2(materialId, 0), 0);
    ivec4 row1 = texelFetch(u_textureMaterials, ivec2(materialId, 1), 0);

    Material material;
    material.frameCount = max(row0.a & 0xff, 1);
    material.animSpeed = row1.r & 0xff;
    return material;
}

vec4 sampleLayer(uint layer, vec2 uv) {
    int maxLayer = max(u_textureLayerCount - 1, 0);
    float clampedLayer = float(clamp(int(layer), 0, maxLayer));
    return texture(u_textures, vec3(uv, clampedLayer), -2.0).bgra;
}

vec4 sampleModelTexture(uint textureId, vec2 texCoord) {
    if (textureId == 0u) {
        return vec4(1.0);
    }
    return sampleLayer(textureId, texCoord);
}

void main() {
    vec4 textureColor = sampleModelTexture(v_texId, v_texCoord);
    float alpha = textureColor.a * v_color.a;

    if (u_discardAlpha != 0
        && ((v_texId == 0u && alpha < 0.01) || textureColor.a < v_alphaCutOff)) {
        discard;
    }

    Material material = getMaterial(v_texId);
    int frameCount = max(material.frameCount, 1);
    if (v_texId != 0u && frameCount > 1) {
        float frameSpeed = float(max(material.animSpeed, 1));
        float frameT = mod(u_currentTime * frameSpeed, float(frameCount));
        float frame0 = floor(frameT);
        float frame1 = mod(frame0 + 1.0, float(frameCount));
        float mixAmount = fract(frameT);
        vec4 tex0 = sampleLayer(v_texId + uint(frame0), v_texCoord);
        vec4 tex1 = sampleLayer(v_texId + uint(frame1), v_texCoord);
        textureColor = mix(tex0, tex1, mixAmount);
        alpha = textureColor.a * v_color.a;
    }

    float banding = max(u_colorBanding, 1.0);
    vec3 paletteColor = round(v_color.rgb * banding) / banding;
    vec3 surface = textureColor.rgb * paletteColor * u_brightness;

    float fog = smoothstep(0.0, 1.0, clamp(v_fogAmount, 0.0, 1.0));
    vec3 rgb = mix(surface, u_skyColor.rgb, fog);
    fragColor = vec4(clamp(rgb, 0.0, 1.0), alpha);
}
