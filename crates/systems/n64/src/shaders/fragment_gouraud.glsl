#version 330 core

// Fragment shader for Gouraud-shaded triangles
// Interpolates per-vertex colors across the triangle

in vec4 vColor;   // Interpolated color from vertex shader

out vec4 FragColor;

void main() {
    FragColor = vColor;
}
