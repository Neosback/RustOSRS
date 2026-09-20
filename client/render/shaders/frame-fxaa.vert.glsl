#version 300 es

layout(location = 0) in vec4 a_pos;

uniform vec2 u_resolution;

out mediump vec2 v_rgbNW;
out mediump vec2 v_rgbNE;
out mediump vec2 v_rgbSW;
out mediump vec2 v_rgbSE;
out mediump vec2 v_rgbM;

void main() {
    gl_Position = a_pos;
    // Inline FXAA texcoords (avoid GLSL `out` params). Safari's Metal ANGLE
    // backend fails to link shaders that pass varyings through out-parameter
    // helpers (ANGLE_Out<float2> thread-address-space mismatch).
    vec2 fragCoord = (0.5 * gl_Position.xy + vec2(0.5)) * u_resolution;
    vec2 inverseVP = 1.0 / u_resolution.xy;
    v_rgbNW = (fragCoord + vec2(-1.0, -1.0)) * inverseVP;
    v_rgbNE = (fragCoord + vec2(1.0, -1.0)) * inverseVP;
    v_rgbSW = (fragCoord + vec2(-1.0, 1.0)) * inverseVP;
    v_rgbSE = (fragCoord + vec2(1.0, 1.0)) * inverseVP;
    v_rgbM = fragCoord * inverseVP;
}
