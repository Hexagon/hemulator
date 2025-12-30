#version 330 core

// Fragment shader for textured triangles
// Samples from a 2D texture using interpolated texture coordinates

uniform sampler2D uTexture;  // Texture sampler

in vec2 vTexCoord;   // Interpolated texture coordinates from vertex shader

out vec4 FragColor;

void main() {
    FragColor = texture(uTexture, vTexCoord);
}
