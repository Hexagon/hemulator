#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 center = texture(uTexture, vTexCoord);
    
    // Calculate texel size for efficient neighbor sampling
    vec2 texelSize = 1.0 / uResolution;
    
    // Sample neighboring pixels horizontally for phosphor glow
    // Optimized: reduce blend amount for better performance and subtler effect
    vec4 left = texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0));
    vec4 right = texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0));
    
    // Lighter blend: 85% center + 10% neighbors (was 70%/30% - too blurry)
    vec4 blended = center * 0.85 + left * 0.075 + right * 0.075;
    
    FragColor = blended;
}
