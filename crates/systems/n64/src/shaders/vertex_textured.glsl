#version 330 core

// Vertex shader for textured triangles
// Supports texture coordinates with optional Z-buffer depth

layout(location = 0) in vec2 aPosition;  // Vertex position (normalized device coords)
layout(location = 1) in vec2 aTexCoord;  // Texture coordinates (s, t)
layout(location = 2) in float aDepth;    // Vertex depth (for Z-buffer)

out vec2 vTexCoord;  // Pass texture coordinates to fragment shader
out float vDepth;    // Pass depth to fragment shader

void main() {
    // Map depth from [0, 1] to NDC z range [-1, 1] for correct GPU depth
    // testing.  When aDepth is unbound (non-z-buffer paths) it is 0 by
    // default, which places the fragment at the near plane; depth testing is
    // disabled for those paths so this has no visible effect.
    float ndcZ = aDepth * 2.0 - 1.0;
    gl_Position = vec4(aPosition, ndcZ, 1.0);
    vTexCoord = aTexCoord;
    vDepth = aDepth;
}
