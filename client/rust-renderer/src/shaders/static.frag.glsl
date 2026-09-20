#version 300 es
precision highp float;

uniform vec4 u_skyColor;

in vec4 v_color;
in float v_fogAmount;
in vec3 v_worldPos;
flat in float v_plane;

out vec4 fragColor;

void main() {
    if (v_color.a <= 0.0) {
        discard;
    }

    float fog = smoothstep(0.0, 1.0, clamp(v_fogAmount, 0.0, 1.0));
    vec3 rgb = mix(v_color.rgb, u_skyColor.rgb, fog);
    fragColor = vec4(clamp(rgb, 0.0, 1.0), v_color.a);
}
