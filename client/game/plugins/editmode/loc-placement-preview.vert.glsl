#version 300 es

#define TEXTURE_ANIM_UNIT (1.0f / 128.0f)

precision highp float;
layout(std140, column_major) uniform;

uniform highp isampler2D u_textureMaterials;

#include "../../../render/shaders/includes/scene-uniforms.glsl";

uniform vec3 u_previewPosition;
uniform float u_previewPlane;

layout(location = 0) in uvec3 a_vertex;

out vec4 v_color;
out vec2 v_texCoord;
out vec2 v_worldUv;
out vec3 v_worldPos;
flat out uint v_texId;
flat out float v_alphaCutOff;
out float v_fogAmount;
flat out float v_plane;

#include "../../../render/shaders/includes/branchless-logic.glsl";
#include "../../../render/shaders/includes/hsl-to-rgb.glsl";
#include "../../../render/shaders/includes/unpack-float.glsl";
#include "../../../render/shaders/includes/material.glsl";
#include "../../../render/shaders/includes/vertex.glsl";

void main() {
    Vertex vertex = decodeVertex(a_vertex.x, a_vertex.y, a_vertex.z, u_brightness, vec4(-1, -1, -1, 0));
    v_color = vertex.color;

    Material material = getMaterial(vertex.textureId);
    vec2 textureAnimation = vec2(material.animU, material.animV);
    if (u_isNewTextureAnim > 0.5) {
        v_texCoord = vertex.texCoord + mod(mod(u_currentTime, 128.0) * textureAnimation / 64.0, 1.0);
    } else {
        v_texCoord = vertex.texCoord + (u_currentTime / 0.02) * textureAnimation * TEXTURE_ANIM_UNIT;
    }
    v_texId = vertex.textureId;
    v_alphaCutOff = material.alphaCutOff;

    vec3 worldPos = vertex.pos / 128.0 + u_previewPosition;
    v_worldUv = worldPos.xz;
    v_worldPos = worldPos;
    v_fogAmount = 0.0;
    v_plane = u_previewPlane;

    vec4 viewPos = u_viewMatrix * vec4(worldPos, 1.0);
    viewPos.z += u_previewPlane * 0.001 + float(vertex.priority) * 0.001;
    gl_Position = u_projectionMatrix * viewPos;
}
