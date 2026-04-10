#version 330 core

// Vertex shader for textured+shaded triangles (colour-combiner MODULATE mode)
// Passes both texture coordinates and per-vertex shade colour to the fragment
// stage so the fragment shader can compute: out = texel * shade.

layout(location = 0) in vec2 aPosition;  // Vertex position (NDC)
layout(location = 1) in vec2 aTexCoord;  // Texture coordinates (s, t)
layout(location = 2) in float aDepth;    // Vertex depth [0,1] for Z-buffer
layout(location = 3) in vec4 aColor;     // Per-vertex shade colour (RGBA)

out vec2 vTexCoord;
out vec4 vColor;

void main() {
    float ndcZ = aDepth * 2.0 - 1.0;
    gl_Position = vec4(aPosition, ndcZ, 1.0);
    vTexCoord = aTexCoord;
    vColor = aColor;
}
