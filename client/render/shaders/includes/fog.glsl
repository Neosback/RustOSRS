float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

// OSRS-style fog: a square (Chebyshev) boundary with rounded corners around the
// player, matching the scene-edge fog shape of the OSRS client / RuneLite GPU
// plugin. The ramp starts at u_fogDepth (tiles) and reaches full fog at
// u_renderDistance. Evaluated per-vertex on the tile grid, so the fog edge
// follows tile geometry like OSRS.
float fogFactorOSRS(vec2 playerOffset) {
    float fogStart = min(u_fogDepth, u_renderDistance);
    float fogEnd = max(u_renderDistance, fogStart + 0.0001);
    float rounding = min(FOG_CORNER_ROUNDING, fogEnd);
    float d = sdRoundedBox(playerOffset, vec2(fogEnd), rounding);
    return clamp(d / (fogEnd - fogStart) + 1.0, 0.0, 1.0);
}

float fogFactorLinear(float dist, float start, float end) {
    return 1.0 - clamp((dist - start) / (end - start), 0.0, 1.0);
}
