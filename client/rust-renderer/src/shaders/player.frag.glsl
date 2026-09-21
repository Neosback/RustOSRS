#version 300 es
precision highp float;
precision highp int;
precision highp sampler2DArray;
precision highp isampler2D;

uniform highp sampler2DArray u_textures;
uniform highp isampler2D u_textureMaterials;
uniform float u_currentTime;
uniform float u_colorBanding;
uniform float u_brightness;
uniform vec4 u_skyColor;
uniform int u_discardAlpha;
uniform int u_materialCount;

in vec4 v_color;
in vec2 v_texCoord;
flat in uint v_texId;
flat in float v_alphaCutOff;
in float v_fogAmount;
flat in float v_plane;
flat in uint v_priority;

layout(location = 0) out vec4 fragColor;

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

void main() {
    vec4 textureColor =
        texture(u_textures, vec3(v_texCoord, v_texId), -2.0).bgra;
    float alpha = textureColor.a * v_color.a;

    if (u_discardAlpha != 0) {
        if (
            (v_texId == 0u && alpha < 0.01)
            || textureColor.a < v_alphaCutOff
        ) {
            discard;
        }
    }

    Material material = getMaterial(v_texId);
    int frameCount = max(material.frameCount, 1);
    if (frameCount > 1) {
        float frameSpeed = float(max(material.animSpeed, 1));
        float frameT = mod(
            u_currentTime * frameSpeed,
            float(frameCount)
        );
        float frame0 = floor(frameT);
        float frame1 = mod(frame0 + 1.0, float(frameCount));
        float mixAmount = fract(frameT);
        vec4 tex0 = texture(
            u_textures,
            vec3(v_texCoord, float(v_texId) + frame0),
            -2.0
        ).bgra;
        vec4 tex1 = texture(
            u_textures,
            vec3(v_texCoord, float(v_texId) + frame1),
            -2.0
        ).bgra;
        textureColor = mix(tex0, tex1, mixAmount);
        alpha = textureColor.a * v_color.a;
    }

    float banding = max(u_colorBanding, 1.0);
    vec3 paletteColor = round(v_color.rgb * banding) / banding;
    vec3 surface = textureColor.rgb * paletteColor * u_brightness;

    float fog = clamp(v_fogAmount, 0.0, 1.0);
    fog = smoothstep(0.0, 1.0, fog);
    vec3 finalRgb = mix(surface, u_skyColor.rgb, fog);

    fragColor = vec4(clamp(finalRgb, 0.0, 1.0), alpha);
}
