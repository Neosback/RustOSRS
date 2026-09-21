#version 300 es
precision highp float;

uniform highp sampler2D u_frame;
uniform vec2 u_resolution;

in vec2 v_uv;
in mediump vec2 v_rgbNW;
in mediump vec2 v_rgbNE;
in mediump vec2 v_rgbSW;
in mediump vec2 v_rgbSE;
in mediump vec2 v_rgbM;

out vec4 fragColor;

vec4 fxaa(
    sampler2D tex,
    vec2 fragCoord,
    vec2 resolution,
    vec2 rgbNWCoord,
    vec2 rgbNECoord,
    vec2 rgbSWCoord,
    vec2 rgbSECoord,
    vec2 rgbMCoord
) {
    const float FXAA_REDUCE_MIN = 1.0 / 128.0;
    const float FXAA_REDUCE_MUL = 1.0 / 8.0;
    const float FXAA_SPAN_MAX = 8.0;

    mediump vec2 inverseVP = vec2(
        1.0 / resolution.x,
        1.0 / resolution.y
    );
    vec3 rgbNW = texture(tex, rgbNWCoord).xyz;
    vec3 rgbNE = texture(tex, rgbNECoord).xyz;
    vec3 rgbSW = texture(tex, rgbSWCoord).xyz;
    vec3 rgbSE = texture(tex, rgbSECoord).xyz;
    vec4 texColor = texture(tex, rgbMCoord);
    vec3 rgbM = texColor.xyz;
    vec3 luma = vec3(0.299, 0.587, 0.114);

    float lumaNW = dot(rgbNW, luma);
    float lumaNE = dot(rgbNE, luma);
    float lumaSW = dot(rgbSW, luma);
    float lumaSE = dot(rgbSE, luma);
    float lumaM = dot(rgbM, luma);
    float lumaMin = min(
        lumaM,
        min(min(lumaNW, lumaNE), min(lumaSW, lumaSE))
    );
    float lumaMax = max(
        lumaM,
        max(max(lumaNW, lumaNE), max(lumaSW, lumaSE))
    );

    mediump vec2 dir;
    dir.x = -((lumaNW + lumaNE) - (lumaSW + lumaSE));
    dir.y = (lumaNW + lumaSW) - (lumaNE + lumaSE);

    float dirReduce = max(
        (lumaNW + lumaNE + lumaSW + lumaSE)
            * (0.25 * FXAA_REDUCE_MUL),
        FXAA_REDUCE_MIN
    );
    float rcpDirMin =
        1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
    dir = min(
        vec2(FXAA_SPAN_MAX),
        max(vec2(-FXAA_SPAN_MAX), dir * rcpDirMin)
    ) * inverseVP;

    vec3 rgbA = 0.5 * (
        texture(
            tex,
            fragCoord * inverseVP
                + dir * (1.0 / 3.0 - 0.5)
        ).xyz
        + texture(
            tex,
            fragCoord * inverseVP
                + dir * (2.0 / 3.0 - 0.5)
        ).xyz
    );
    vec3 rgbB = rgbA * 0.5 + 0.25 * (
        texture(tex, fragCoord * inverseVP + dir * -0.5).xyz
        + texture(tex, fragCoord * inverseVP + dir * 0.5).xyz
    );

    float lumaB = dot(rgbB, luma);
    if (lumaB < lumaMin || lumaB > lumaMax) {
        return vec4(rgbA, texColor.a);
    }
    return vec4(rgbB, texColor.a);
}

void main() {
    fragColor = fxaa(
        u_frame,
        gl_FragCoord.xy,
        u_resolution,
        v_rgbNW,
        v_rgbNE,
        v_rgbSW,
        v_rgbSE,
        v_rgbM
    );
}
