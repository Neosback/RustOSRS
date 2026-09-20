import type { ProgramSource } from "../../../render/shaders/ShaderUtil";

/** Adapt the shared world programs, never the UI, water implementation or mesh builders. */
export function createHdProgram([vertex, fragment]: ProgramSource, lighting: string): ProgramSource {
    const vertexEnd = vertex.lastIndexOf("}");
    if (vertexEnd < 0 || !vertex.includes("vec4 viewPos =") || !fragment.includes("void main()") ||
        !fragment.includes("    float banding =") || !fragment.includes("    vec3 finalRgb = mix(surface, u_skyColor.rgb, fog);")) {
        throw new Error("117 HD: unsupported scene shader");
    }
    const terrain = vertex.includes("CONTOUR_GROUND_NONE");
    vertex = vertex.slice(0, vertexEnd) + `
    v_hdTerrain = ${terrain ? "modelInfo.contourGround == 3.0 ? 1.0 : 0.0" : "0.0"};
    v_hdPosition = u_hdEnabled ? (u_hdInverseView * viewPos).xyz : vec3(0.0);
    if (u_hdShadowPass) gl_Position = u_hdShadowMatrix * vec4(v_hdPosition, 1.0);
` + vertex.slice(vertexEnd);
    vertex = vertex.replace("void main()", `
out vec3 v_hdPosition;
flat out float v_hdTerrain;
uniform bool u_hdEnabled;
uniform mat4 u_hdInverseView;
uniform bool u_hdShadowPass;
uniform mat4 u_hdShadowMatrix;
void main()`);

    const water = fragment.includes("bool isFloorWater = false;");
    fragment = fragment.replace("void main()", lighting + "\nvoid main()");
    const mainStart = fragment.indexOf("void main()");
    const declarations = fragment.slice(0, mainStart);
    fragment = fragment.slice(mainStart);
    fragment = fragment.replace("    float alpha =", `
    float hdLayer = u_hdEnabled ? texelFetch(u_hdMaterials, ivec2(int(v_texId), 1), 0).x : 0.0;
    if (hdLayer > 0.0) {
        vec2 scale = max(texelFetch(u_hdMaterials, ivec2(int(v_texId), 0), 0).zw, vec2(0.001));
        textureColor = texture(u_hdTextures, vec3((v_texCoord - 0.5) / scale + 0.5, hdLayer));
    }
    float alpha =`);
    fragment = fragment.replace("frameCount > 1)", "frameCount > 1 && hdLayer == 0.0)");
    fragment = fragment.replace("    float banding =", `
    if (u_hdShadowPass) {
        if (alpha < 0.5) discard;
        fragColor = vec4(1.0);
        return;
    }
    float banding =`);
    // Surface water bypasses HD lighting AND environment fog, preserving its existing appearance.
    fragment = fragment.replace("    vec3 finalRgb = mix(surface, u_skyColor.rgb, fog);", `
    vec3 fogColor = u_skyColor.rgb;
    if (u_hdEnabled && ${water ? "!isFloorWater" : "true"}) {
        surface = hdShade(surface, v_hdPosition);
        float hdFog = hdFogAmount(v_hdPosition.xz);
        float groundFog = smoothstep(0.0, 1.0, (v_hdPosition.y - u_hdGroundFog.x) / min(-0.001, u_hdGroundFog.y - u_hdGroundFog.x)) * u_hdGroundFog.z;
        fog = max(fog, max(hdFog, groundFog));
        fogColor = u_hdFogColor;
    }
    vec3 finalRgb = mix(surface, fogColor, fog);`);
    return [vertex, declarations + fragment];
}
