#version 300 es
precision highp float;

uniform vec2 u_resolution;

out vec2 v_uv;
out mediump vec2 v_rgbNW;
out mediump vec2 v_rgbNE;
out mediump vec2 v_rgbSW;
out mediump vec2 v_rgbSE;
out mediump vec2 v_rgbM;

void main() {
    vec2 pos;
    if (gl_VertexID == 0) {
        pos = vec2(-1.0, -1.0);
    } else if (gl_VertexID == 1) {
        pos = vec2(3.0, -1.0);
    } else {
        pos = vec2(-1.0, 3.0);
    }

    gl_Position = vec4(pos, 0.0, 1.0);
    v_uv = 0.5 * pos + vec2(0.5);

    vec2 fragCoord = v_uv * u_resolution;
    vec2 inverseVP = 1.0 / u_resolution;
    v_rgbNW = (fragCoord + vec2(-1.0, -1.0)) * inverseVP;
    v_rgbNE = (fragCoord + vec2(1.0, -1.0)) * inverseVP;
    v_rgbSW = (fragCoord + vec2(-1.0, 1.0)) * inverseVP;
    v_rgbSE = (fragCoord + vec2(1.0, 1.0)) * inverseVP;
    v_rgbM = fragCoord * inverseVP;
}
