struct Material {
    int animU;
    int animV;
    float alphaCutOff;
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

const int MATERIAL_FLAG_WATER = 1;

const int WATER_FLAG_HAS_FOAM = 1;
const int WATER_FLAG_NORMAL_MAP_2 = 2;

Material getMaterial(uint textureId) {
    ivec4 data = texelFetch(u_textureMaterials, ivec2(textureId, 0), 0);
    ivec4 data1 = texelFetch(u_textureMaterials, ivec2(textureId, 1), 0);
    ivec4 data2 = texelFetch(u_textureMaterials, ivec2(textureId, 2), 0);
    ivec4 data3 = texelFetch(u_textureMaterials, ivec2(textureId, 3), 0);
    ivec4 data4 = texelFetch(u_textureMaterials, ivec2(textureId, 4), 0);
    ivec4 data5 = texelFetch(u_textureMaterials, ivec2(textureId, 5), 0);

    Material material;
    material.animU = data.r;
    material.animV = data.g;
    material.alphaCutOff = float(data.b & 0xFF) / 255.0;
    material.frameCount = data.a & 0xFF;
    material.animSpeed = data1.r & 0xFF;
    material.flags = data1.g & 0xFF;
    int waterFlags = data1.b & 0xFF;
    material.waterHasFoam = float(waterFlags & WATER_FLAG_HAS_FOAM);
    material.waterUseNormalMap2 = (waterFlags & WATER_FLAG_NORMAL_MAP_2) != 0;
    material.waterSurfaceColor = vec3(data2.r & 0xFF, data2.g & 0xFF, data2.b & 0xFF) / 255.0;
    material.waterBaseOpacity = float(data2.a & 0xFF) / 255.0;
    material.waterDepthColor = vec3(data3.r & 0xFF, data3.g & 0xFF, data3.b & 0xFF) / 255.0;
    material.waterFresnelAmount = float(data3.a & 0xFF) / 255.0;
    material.waterNormalStrength = float(data4.r & 0xFF) / 255.0 * 0.5;
    material.waterSpecularStrength = float(data4.g & 0xFF) / 255.0;
    material.waterSpecularGloss = max(float(data4.b & 0xFF) / 255.0 * 500.0, 1.0);
    material.waterDuration = float(data4.a & 0xFF) / 255.0 * 4.0;
    material.waterFoamColor = vec3(data5.r & 0xFF, data5.g & 0xFF, data5.b & 0xFF) / 255.0;
    if (material.frameCount == 0) {
        material.frameCount = 1;
    }

    return material;
}
