#version 330 core

// Vertex shader for textured triangles
// Supports texture coordinates with optional Z-buffer depth

layout(location = 0) in vec2 aPosition;  // Vertex position (normalized device coords)
layout(location = 1) in vec2 aTexCoord;  // Texture coordinates (s, t)
layout(location = 2) in float aDepth;    // Vertex depth (for Z-buffer)

out vec2 vTexCoord;  // Pass texture coordinates to fragment shader
out float vDepth;    // Pass depth to fragment shader

void main() {
    gl_Position = vec4(aPosition, 0.0, 1.0);
    vTexCoord = aTexCoord;
    vDepth = aDepth;
}
