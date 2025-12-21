#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 center = texture(uTexture, vTexCoord);
    
    // Calculate texel size
    vec2 texelSize = 1.0 / uResolution;
    
    // Sample neighboring pixels horizontally for phosphor glow
    vec4 left = texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0));
    vec4 right = texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0));
    
    // Blend with neighbors (15% contribution from each neighbor)
    vec4 blended = center * 0.7 + left * 0.15 + right * 0.15;
    
    FragColor = blended;
}
