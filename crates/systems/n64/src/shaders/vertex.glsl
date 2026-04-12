#version 330 core

// Vertex shader for N64 RDP triangle rendering
// Supports both 2D and 3D rendering with color and depth

layout(location = 0) in vec2 aPosition;  // Vertex position (normalized device coords)
layout(location = 1) in vec4 aColor;     // Vertex color (RGBA)
layout(location = 2) in float aDepth;    // Vertex depth (for Z-buffer)

out vec4 vColor;   // Pass color to fragment shader
out float vDepth;  // Pass depth to fragment shader

void main() {
    // Map depth from [0, 1] to NDC z range [-1, 1] so that the GPU depth
    // buffer test reflects the true scene depth order.  When aDepth is not
    // bound (non-z-buffer draw paths) it defaults to 0 → ndcZ = -1 (near
    // plane), which is harmless because depth testing is disabled for those
    // calls.
    float ndcZ = aDepth * 2.0 - 1.0;
    gl_Position = vec4(aPosition, ndcZ, 1.0);
    vColor = aColor;
    vDepth = aDepth;
}
