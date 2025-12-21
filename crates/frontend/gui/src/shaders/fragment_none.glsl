#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    // Simple passthrough - no filter
    FragColor = texture(uTexture, vTexCoord);
}
