#version 300 es
precision highp float;
precision highp int;

layout(location = 0) in uvec3 a_packed;

uniform mat4 u_viewProj;
uniform float u_brightness;

out vec4 v_color;

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

void main() {
    uint v0 = a_packed.x;
    uint v1 = a_packed.y;
    uint v2 = a_packed.z;

    float x = float(int((v0 >> 17u) & 0x7fffu) - 0x4000);
    float y = -float(int(v1 & 0x7fffu) - 0x4000);
    float z = float(int((v2 >> 17u) & 0x7fffu) - 0x4000);

    int hsl = int((v1 >> 15u) & 0xffffu);
    bool textured = ((v1 >> 31u) & 1u) != 0u;
    float alpha = float((v2 >> 9u) & 0xffu) / 255.0;

    // Stage 0 validates geometry and HSL. Texture/material ownership moves
    // into Rust in Stage 1, so textured faces use their baked light value.
    vec3 rgb = textured
        ? vec3(float(hsl & 127) / 127.0)
        : hslToRgb(hsl, u_brightness);

    v_color = vec4(rgb, alpha);
    gl_Position = u_viewProj * vec4(x, y, z, 1.0);
}
