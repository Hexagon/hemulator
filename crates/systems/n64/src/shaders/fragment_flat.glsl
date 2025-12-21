#version 330 core

// Fragment shader for flat-shaded triangles (no interpolation)
// Uses a uniform color for the entire triangle

uniform vec4 uColor;  // Triangle color (RGBA)

out vec4 FragColor;

void main() {
    FragColor = uColor;
}
