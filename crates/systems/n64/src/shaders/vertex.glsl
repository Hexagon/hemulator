#version 330 core

// Vertex shader for N64 RDP triangle rendering
// Supports both 2D and 3D rendering with color and depth

layout(location = 0) in vec2 aPosition;  // Vertex position (normalized device coords)
layout(location = 1) in vec4 aColor;     // Vertex color (RGBA)
layout(location = 2) in float aDepth;    // Vertex depth (for Z-buffer)

out vec4 vColor;   // Pass color to fragment shader
out float vDepth;  // Pass depth to fragment shader

void main() {
    gl_Position = vec4(aPosition, 0.0, 1.0);
    vColor = aColor;
    vDepth = aDepth;
}
