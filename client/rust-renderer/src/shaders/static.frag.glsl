#version 300 es
precision highp float;
precision highp int;
precision highp sampler2DArray;
precision highp isampler2D;

uniform mat4 u_viewMatrix;
uniform vec4 u_skyColor;
uniform sampler2DArray u_textures;
uniform highp isampler2D u_textureMaterials;
uniform sampler2DArray u_waterTextures;
uniform sampler2DArray u_waterMask;
uniform vec2 u_mapPos;
uniform int u_sceneBorderSize;
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

const int MATERIAL_FLAG_WATER = 1;
const int WATER_FLAG_HAS_FOAM = 1;
const int WATER_FLAG_NORMAL_MAP_2 = 2;

const float WATER_NORMAL_1 = 0.0;
const float WATER_NORMAL_2 = 1.0;
const float WATER_FLOW = 2.0;
const float WATER_FOAM = 3.0;
const float WATER_CAUSTICS = 4.0;
const float WATER_MAX_DEPTH = 759.0;

const vec3 WATER_LIGHT_DIR = vec3(-0.5044, -0.7880, -0.3531);
const vec3 WATER_AMBIENT_COLOR = vec3(0.5922, 0.7294, 1.0);
const float WATER_AMBIENT_STRENGTH = 1.0;
const vec3 WATER_DIR_LIGHT_COLOR = vec3(1.0);
const float WATER_DIR_LIGHT_STRENGTH = 1.0;
const float WATER_SKY_LIGHT_STRENGTH = 0.5;
const vec3 WATER_COLOR_LIGHT = vec3(0.6562, 0.7598, 0.9063);
const vec3 WATER_COLOR_MID = vec3(0.5046, 0.5861, 0.7014);
const vec3 WATER_COLOR_DARK = vec3(0.1690, 0.2017, 0.2478);

struct Material {
    int frameCount;
    int animSpeed;
    int flags;
    vec3 waterSurfaceColor;
    vec3 waterFoamColor;
    vec3 waterDepthColor;
    float waterBaseOpacity;
    float waterFresnelAmount;
    float waterNormalStrength;
    float waterSpecularStrength;
    float waterSpecularGloss;
    float waterDuration;
    float waterHasFoam;
    bool waterUseNormalMap2;
};

struct WaterMaskSample {
    float water;
    float shore;
    float depth;
    vec3 bedColor;
};

Material getMaterial(uint textureId) {
    int maxMaterial = max(u_materialCount - 1, 0);
    int materialId = clamp(int(textureId), 0, maxMaterial);
    ivec4 row0 = texelFetch(u_textureMaterials, ivec2(materialId, 0), 0);
    ivec4 row1 = texelFetch(u_textureMaterials, ivec2(materialId, 1), 0);
    ivec4 row2 = texelFetch(u_textureMaterials, ivec2(materialId, 2), 0);
    ivec4 row3 = texelFetch(u_textureMaterials, ivec2(materialId, 3), 0);
    ivec4 row4 = texelFetch(u_textureMaterials, ivec2(materialId, 4), 0);
    ivec4 row5 = texelFetch(u_textureMaterials, ivec2(materialId, 5), 0);

    Material material;
    material.frameCount = max(row0.a & 0xff, 1);
    material.animSpeed = row1.r & 0xff;
    material.flags = row1.g & 0xff;

    int waterFlags = row1.b & 0xff;
    material.waterHasFoam = float(waterFlags & WATER_FLAG_HAS_FOAM);
    material.waterUseNormalMap2 = (waterFlags & WATER_FLAG_NORMAL_MAP_2) != 0;
    material.waterSurfaceColor =
        vec3(row2.r & 0xff, row2.g & 0xff, row2.b & 0xff) / 255.0;
    material.waterBaseOpacity = float(row2.a & 0xff) / 255.0;
    material.waterDepthColor =
        vec3(row3.r & 0xff, row3.g & 0xff, row3.b & 0xff) / 255.0;
    material.waterFresnelAmount = float(row3.a & 0xff) / 255.0;
    material.waterNormalStrength = float(row4.r & 0xff) / 255.0 * 0.5;
    material.waterSpecularStrength = float(row4.g & 0xff) / 255.0;
    material.waterSpecularGloss =
        max(float(row4.b & 0xff) / 255.0 * 500.0, 1.0);
    material.waterDuration = float(row4.a & 0xff) / 255.0 * 4.0;
    material.waterFoamColor =
        vec3(row5.r & 0xff, row5.g & 0xff, row5.b & 0xff) / 255.0;
    return material;
}

vec4 readWaterMaskTexel(ivec2 texel, int layer, ivec3 maskSize) {
    ivec2 clampedTexel = clamp(texel, ivec2(0), maskSize.xy - ivec2(1));
    return texelFetch(u_waterMask, ivec3(clampedTexel, layer), 0);
}

float waterMaskWaterBit(vec4 texel) {
    return step(0.5, texel.a);
}

float waterMaskDepth(vec4 texel) {
    return max(texel.a * 255.0 - 128.0, 0.0) / 127.0;
}

WaterMaskSample sampleWaterMask(vec2 worldUv, float plane) {
    ivec3 maskSize = textureSize(u_waterMask, 0);
    int layer = clamp(int(floor(plane + 0.5)), 0, maskSize.z - 1);

    vec2 maskPos =
        worldUv - u_mapPos * 64.0 + vec2(float(u_sceneBorderSize));
    ivec2 texel = ivec2(floor(maskPos));
    vec2 tileFract = fract(maskPos);

    float centerWater = waterMaskWaterBit(
        readWaterMaskTexel(texel, layer, maskSize)
    );

    float leftWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(-1, 0), layer, maskSize)
    );
    float rightWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(1, 0), layer, maskSize)
    );
    float downWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(0, -1), layer, maskSize)
    );
    float upWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(0, 1), layer, maskSize)
    );
    float downLeftWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(-1, -1), layer, maskSize)
    );
    float downRightWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(1, -1), layer, maskSize)
    );
    float upLeftWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(-1, 1), layer, maskSize)
    );
    float upRightWater = waterMaskWaterBit(
        readWaterMaskTexel(texel + ivec2(1, 1), layer, maskSize)
    );

    float shoreFromLand = 0.0;
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - leftWater) * (1.0 - tileFract.x)
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - rightWater) * tileFract.x
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - downWater) * (1.0 - tileFract.y)
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - upWater) * tileFract.y
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - downLeftWater) * (1.0 - tileFract.x) * (1.0 - tileFract.y)
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - downRightWater) * tileFract.x * (1.0 - tileFract.y)
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - upLeftWater) * (1.0 - tileFract.x) * tileFract.y
    );
    shoreFromLand = max(
        shoreFromLand,
        (1.0 - upRightWater) * tileFract.x * tileFract.y
    );
    shoreFromLand *= step(0.5, centerWater);

    vec2 depthPos = maskPos - 0.5;
    ivec2 depthBase = ivec2(floor(depthPos));
    vec2 depthFract = fract(depthPos);
    vec4 mask00 = readWaterMaskTexel(depthBase, layer, maskSize);
    vec4 mask10 = readWaterMaskTexel(
        depthBase + ivec2(1, 0),
        layer,
        maskSize
    );
    vec4 mask01 = readWaterMaskTexel(
        depthBase + ivec2(0, 1),
        layer,
        maskSize
    );
    vec4 mask11 = readWaterMaskTexel(
        depthBase + ivec2(1, 1),
        layer,
        maskSize
    );

    float depth = mix(
        mix(waterMaskDepth(mask00), waterMaskDepth(mask10), depthFract.x),
        mix(waterMaskDepth(mask01), waterMaskDepth(mask11), depthFract.x),
        depthFract.y
    );
    vec3 bedColor = mix(
        mix(mask00.rgb, mask10.rgb, depthFract.x),
        mix(mask01.rgb, mask11.rgb, depthFract.x),
        depthFract.y
    );

    return WaterMaskSample(centerWater, shoreFromLand, depth, bedColor);
}

vec2 waterWorldUvs(vec2 worldUv, float scale) {
    return -worldUv / scale;
}

float waterAnimationFrame(float animationDuration, float time) {
    if (animationDuration == 0.0) {
        return 0.0;
    }
    return mod(time, animationDuration) / animationDuration;
}

float waterSpecular(
    vec3 viewDir,
    vec3 reflectDir,
    float gloss,
    float strength
) {
    float vDotR = clamp(dot(viewDir, reflectDir), 1e-10, 1.0);
    return pow(vDotR, gloss) * strength;
}

float sampleCausticsChannel(
    vec2 flow1,
    vec2 flow2,
    vec2 aberration
) {
    return min(
        texture(
            u_waterTextures,
            vec3(flow1 + aberration, WATER_CAUSTICS)
        ).r,
        texture(
            u_waterTextures,
            vec3(flow2 + aberration, WATER_CAUSTICS)
        ).r
    );
}

vec3 sampleCaustics(vec2 flow1, vec2 flow2, float aberration) {
    float r = sampleCausticsChannel(
        flow1,
        flow2,
        aberration * vec2(1.0, 1.0)
    );
    float g = sampleCausticsChannel(
        flow1,
        flow2,
        aberration * vec2(1.0, -1.0)
    );
    float b = sampleCausticsChannel(
        flow1,
        flow2,
        aberration * vec2(-1.0, -1.0)
    );
    return vec3(r, g, b);
}

vec3 shadeWater(
    vec2 worldUv,
    vec2 vanillaUv,
    vec3 worldPos,
    Material material,
    WaterMaskSample waterMask,
    float time
) {
    float duration = material.waterDuration;

    vec2 uv1 = waterWorldUvs(worldUv, 3.0).yx
        - waterAnimationFrame(28.0 * duration, time);
    vec2 uv2 = waterWorldUvs(worldUv, 3.0)
        + waterAnimationFrame(24.0 * duration, time);
    vec2 uv3 = vanillaUv;

    vec2 flowMapUv = waterWorldUvs(worldUv, 15.0)
        + waterAnimationFrame(50.0 * duration, time);
    vec2 uvFlow = texture(
        u_waterTextures,
        vec3(flowMapUv, WATER_FLOW)
    ).xy;
    uv1 += uvFlow * 0.025;
    uv2 += uvFlow * 0.025;
    uv3 += uvFlow * 0.025;

    float normalLayer = material.waterUseNormalMap2
        ? WATER_NORMAL_2
        : WATER_NORMAL_1;
    vec3 t1 = texture(u_waterTextures, vec3(uv1, normalLayer)).xyz;
    vec3 t2 = texture(u_waterTextures, vec3(uv2, normalLayer)).xyz;
    float foamMask = texture(
        u_waterTextures,
        vec3(uv3, WATER_FOAM)
    ).r;

    vec3 n1 = -vec3(
        (t1.x * 2.0 - 1.0) * material.waterNormalStrength,
        t1.z,
        (t1.y * 2.0 - 1.0) * material.waterNormalStrength
    );
    vec3 n2 = -vec3(
        (t2.x * 2.0 - 1.0) * material.waterNormalStrength,
        t2.z,
        (t2.y * 2.0 - 1.0) * material.waterNormalStrength
    );
    vec3 normals = normalize(n1 + n2);

    vec3 cameraWorld = -(u_viewMatrix[3].xyz * mat3(u_viewMatrix));
    vec3 viewDir = normalize(cameraWorld - worldPos);

    float lightDotNormals = dot(normals, WATER_LIGHT_DIR);
    float downDotNormals = -normals.y;
    float viewDotNormals = dot(viewDir, normals);

    vec3 ambientLightOut =
        WATER_AMBIENT_COLOR * WATER_AMBIENT_STRENGTH;
    vec3 dirLightColor =
        WATER_DIR_LIGHT_COLOR * WATER_DIR_LIGHT_STRENGTH;
    vec3 lightOut = max(lightDotNormals, 0.0) * dirLightColor;

    vec3 lightReflectDir = reflect(-WATER_LIGHT_DIR, normals);
    vec3 lightSpecularOut = dirLightColor
        * waterSpecular(
            viewDir,
            lightReflectDir,
            material.waterSpecularGloss,
            material.waterSpecularStrength
        );
    vec3 skyLightOut = max(downDotNormals, 0.0)
        * u_skyColor.rgb
        * WATER_SKY_LIGHT_STRENGTH;

    float baseOpacity = 0.4;
    float fresnel = 1.0 - clamp(viewDotNormals, 0.0, 1.0);
    float finalFresnel = clamp(
        mix(baseOpacity, 1.0, fresnel * 1.2),
        0.0,
        1.0
    );

    vec3 surfaceColor;
    if (finalFresnel < 0.5) {
        surfaceColor = mix(
            WATER_COLOR_DARK,
            WATER_COLOR_MID,
            finalFresnel * 2.0
        );
    } else {
        surfaceColor = mix(
            WATER_COLOR_MID,
            WATER_COLOR_LIGHT,
            (finalFresnel - 0.5) * 2.0
        );
    }

    vec3 surfaceColorOut = surfaceColor
        * max(material.waterSpecularStrength, 0.2);
    vec3 compositeLight =
        ambientLightOut
        + lightOut
        + lightSpecularOut
        + skyLightOut
        + surfaceColorOut;

    vec3 baseColor = material.waterSurfaceColor * compositeLight;
    baseColor = mix(
        baseColor,
        surfaceColor,
        material.waterFresnelAmount
    );
    if (abs(material.waterFresnelAmount - 0.85) < 0.01) {
        baseColor *= 0.75;
    }

    float foamAmount = min(waterMask.shore, 0.8);
    vec3 foamColor =
        material.waterFoamColor * foamMask * compositeLight;
    foamAmount = clamp(
        pow(
            max(1.0 - ((1.0 - foamAmount) / 0.7), 0.0),
            3.0
        ),
        0.0,
        1.0
    ) * material.waterHasFoam;
    foamAmount *= foamColor.r;
    baseColor = mix(baseColor, foamColor, foamAmount);

    vec3 specularComposite =
        mix(lightSpecularOut, vec3(0.0), foamAmount);
    float flatFresnel =
        1.0 - dot(viewDir, vec3(0.0, -1.0, 0.0));
    finalFresnel = max(finalFresnel, flatFresnel);
    baseColor += lightSpecularOut / 3.0;

    float alpha = max(
        material.waterBaseOpacity,
        max(
            foamAmount,
            max(
                finalFresnel,
                length(specularComposite / 3.0)
            )
        )
    );

    float depth = waterMask.depth * WATER_MAX_DEPTH;
    vec3 underwater = waterMask.bedColor;
    if (depth < 150.0) {
        underwater *= mix(
            vec3(1.0),
            material.waterDepthColor,
            depth / 150.0
        );
    } else if (depth < 500.0) {
        underwater *= mix(
            material.waterDepthColor,
            vec3(0.0),
            (depth - 150.0) / 350.0
        );
    } else {
        underwater = vec3(0.0);
    }

    vec2 causticsUv = waterWorldUvs(worldUv, 1.75) * 0.75;
    vec2 causticsDir = vec2(1.0, -2.0);
    vec2 causticsFlow1 =
        causticsUv
        + waterAnimationFrame(17.0, time) * causticsDir;
    vec2 causticsFlow2 =
        causticsUv * 1.5
        - waterAnimationFrame(23.0, time) * causticsDir;
    vec3 caustics = sampleCaustics(
        causticsFlow1,
        causticsFlow2,
        0.005
    );
    float causticsDepthMultiplier =
        (depth - 512.0) / -512.0;
    causticsDepthMultiplier *= causticsDepthMultiplier;
    underwater *= 1.0
        + caustics
        * WATER_DIR_LIGHT_STRENGTH
        * causticsDepthMultiplier
        * max(-WATER_LIGHT_DIR.y, 0.0)
        * WATER_DIR_LIGHT_STRENGTH;

    baseColor = mix(underwater, baseColor, alpha);
    return clamp(baseColor, 0.0, 1.0);
}

vec4 sampleLayer(uint layer, vec2 uv) {
    int maxLayer = max(u_textureLayerCount - 1, 0);
    float clampedLayer = float(clamp(int(layer), 0, maxLayer));
    return texture(
        u_textures,
        vec3(uv, clampedLayer),
        -2.0
    ).bgra;
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

    if (
        u_discardAlpha != 0
        && (
            (v_texId == 0u && alpha < 0.01)
            || textureColor.a < v_alphaCutOff
        )
    ) {
        discard;
    }

    Material material = getMaterial(v_texId);
    int frameCount = max(material.frameCount, 1);
    if (v_texId != 0u && frameCount > 1) {
        float frameSpeed = float(max(material.animSpeed, 1));
        float frameT = mod(
            u_currentTime * frameSpeed,
            float(frameCount)
        );
        float frame0 = floor(frameT);
        float frame1 = mod(frame0 + 1.0, float(frameCount));
        float mixAmount = fract(frameT);
        vec4 tex0 = sampleLayer(
            v_texId + uint(frame0),
            v_texCoord
        );
        vec4 tex1 = sampleLayer(
            v_texId + uint(frame1),
            v_texCoord
        );
        textureColor = mix(tex0, tex1, mixAmount);
        alpha = textureColor.a * v_color.a;
    }

    float banding = max(u_colorBanding, 1.0);
    vec3 paletteColor =
        round(v_color.rgb * banding) / banding;

    vec3 surface;
    bool isFloorWater = false;
    WaterMaskSample waterMask =
        WaterMaskSample(0.0, 0.0, 0.0, vec3(0.0));

    if ((material.flags & MATERIAL_FLAG_WATER) != 0) {
        waterMask = sampleWaterMask(v_worldPos.xz, v_plane);
        isFloorWater = waterMask.water > 0.5;
    }

    if (isFloorWater) {
        surface = shadeWater(
            v_worldPos.xz,
            v_texCoord,
            v_worldPos,
            material,
            waterMask,
            u_currentTime
        ) * u_brightness;
    } else {
        surface =
            textureColor.rgb * paletteColor * u_brightness;
    }

    float fog = smoothstep(
        0.0,
        1.0,
        clamp(v_fogAmount, 0.0, 1.0)
    );
    vec3 rgb = mix(surface, u_skyColor.rgb, fog);
    fragColor = vec4(clamp(rgb, 0.0, 1.0), alpha);
}
