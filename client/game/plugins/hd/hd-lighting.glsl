// Non-water lighting adapted from elvarg-web-client/draw/hdShaders.ts (117HD).
// See public/images/water/LICENSE-117HD.txt.
in vec3 v_hdPosition;
flat in float v_hdTerrain;
uniform bool u_hdEnabled;
uniform bool u_hdShadowPass;
uniform vec3 u_hdLightDirection;
uniform vec3 u_hdAmbient;
uniform vec3 u_hdDirectional;
uniform vec3 u_hdFogColor;
uniform vec3 u_hdFog; // depth, scale, draw distance in tiles
uniform vec3 u_hdGroundFog;
uniform vec4 u_hdGrading; // saturation, contrast, brightness, rim
uniform float u_hdSpecular;
uniform mat4 u_hdShadowMatrix;
uniform sampler2D u_hdShadowMap;
uniform highp sampler2D u_hdMaterials;
uniform highp sampler2DArray u_hdTextures;
uniform float u_hdShadowStrength;
uniform int u_hdLightCount;
uniform vec4 u_hdLightPositions[8];
uniform vec4 u_hdLightColors[8];

vec3 hdSaturation(vec3 color, float amount) {
    return mix(vec3(dot(color, vec3(0.299, 0.587, 0.114))), color, amount);
}

float hdFogAmount(vec2 position) {
    float depth = clamp(u_hdFog.x / 5.0, 0.0, 1.0);
    if (depth <= 0.0002) return 0.0;
    float scale = clamp(u_hdFog.y, 0.0, 16.0);
    float distance = u_hdFog.z;
    vec2 delta = abs(position - u_playerPos);
    float squareDist = max(delta.x, delta.y);
    float reach = clamp((scale - 1.0) / 15.0, 0.0, 1.0);
    float rimWidth = max(8.0, distance * 0.18) * mix(1.0, 3.0, reach);
    float rim = pow(smoothstep(distance - rimWidth, distance, squareDist), mix(0.72, 0.42, depth));
    float wallBand = max(3.0, distance * 0.06) * mix(1.0, 2.0, reach);
    float wall = 1.0 - smoothstep(0.0, wallBand, distance - squareDist);
    float nearAttenuation = smoothstep(0.0, distance * mix(0.18, 0.34, reach), squareDist);
    return clamp((rim * (1.0 + 0.55 * depth) + wall * (0.9 + 0.5 * depth)) * scale * pow(depth, 0.55) * 1.25 * nearAttenuation, 0.0, 1.0);
}

float hdShadow(vec3 position, vec3 normal) {
    vec4 projected = u_hdShadowMatrix * vec4(position, 1.0);
    vec3 p = projected.xyz / projected.w * 0.5 + 0.5;
    if (u_hdShadowStrength == 0.0 || any(lessThan(p, vec3(0.0))) || any(greaterThan(p, vec3(1.0)))) return 1.0;
    float bias = max(0.0012 * (1.0 - max(dot(normal, u_hdLightDirection), 0.0)), 0.00035);
    vec2 texel = 1.0 / vec2(textureSize(u_hdShadowMap, 0));
    float shadow = 0.0;
    for (int x = 0; x < 2; x++) {
        for (int y = 0; y < 2; y++) {
            shadow += p.z - bias > texture(u_hdShadowMap, p.xy + (vec2(x, y) - 0.5) * texel).r ? 1.0 : 0.0;
        }
    }
    float edge = min(min(p.x, 1.0 - p.x), min(p.y, 1.0 - p.y));
    return 1.0 - shadow / 4.0 * u_hdShadowStrength * smoothstep(0.0, 0.05, edge);
}

vec3 hdShade(vec3 surface, vec3 position) {
    vec4 material = texelFetch(u_hdMaterials, ivec2(int(v_texId), 0), 0);
    bool unlit = texelFetch(u_hdMaterials, ivec2(int(v_texId), 1), 0).y > 0.5;
    if (unlit) return surface;
    // ponytail: packed meshes contain no normals; use geometric normals until
    // the mesh pipeline exposes smooth normals as an optional vertex stream.
    vec3 normal = cross(dFdx(position), dFdy(position));
    normal *= inversesqrt(max(dot(normal, normal), 1e-12));
    vec3 camera = -(u_viewMatrix[3].xyz * mat3(u_viewMatrix));
    vec3 viewDir = normalize(camera - position);
    if (dot(normal, viewDir) < 0.0) normal = -normal;
    float shadow = hdShadow(position, normal);
    // ponytail: terrain already has smooth baked lighting; use an up normal for
    // HD until smooth terrain normals are available in the vertex stream.
    if (v_hdTerrain > 0.5) normal = vec3(0.0, -1.0, 0.0);
    vec3 base = pow(max(surface, vec3(0.0)), vec3(2.2));
    float diffuse = max(dot(normal, u_hdLightDirection), 0.0);
    vec3 pointLight = vec3(0.0);
    for (int i = 0; i < 8; i++) {
        if (i >= u_hdLightCount) break;
        vec3 delta = u_hdLightPositions[i].xyz - position;
        float distance = max(length(delta), 0.001);
        float attenuation = max(1.0 - distance / u_hdLightPositions[i].w, 0.0);
        pointLight += u_hdLightColors[i].rgb * u_hdLightColors[i].w * attenuation * attenuation * max(dot(normal, delta / distance), 0.0);
    }
    pointLight *= 1.1;
    float baseLuma = dot(base, vec3(0.2126, 0.7152, 0.0722));
    float additiveFloor = mix(0.035, 0.085, smoothstep(0.10, 0.45, baseLuma));
    vec3 color = base * (u_hdAmbient + u_hdDirectional * diffuse * shadow + pointLight) + pointLight * additiveFloor;
    vec3 halfDir = normalize(viewDir + u_hdLightDirection);
    color += vec3(pow(max(dot(normal, halfDir), 0.0), max(material.y, 1.0)) * material.x * u_hdSpecular * shadow);
    color += vec3(pow(1.0 - max(dot(normal, viewDir), 0.0), 2.0) * u_hdGrading.w);
    color = hdSaturation(color, u_hdGrading.x);
    color = (color - 0.5) * u_hdGrading.y + 0.5;
    return pow(max(color * u_hdGrading.z, vec3(0.0)), vec3(1.0 / 2.2));
}
