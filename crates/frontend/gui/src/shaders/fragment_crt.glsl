#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 center = texture(uTexture, vTexCoord);
    
    // Calculate texel size for neighbor sampling
    vec2 texelSize = 1.0 / uResolution;
    
    // Sample neighboring pixels for subtle phosphor glow
    vec4 left = texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0));
    vec4 right = texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0));
    
    // Lighter blend for subtler effect (85% center + 10% neighbors)
    vec4 blended = center * 0.85 + left * 0.075 + right * 0.075;
    
    // Calculate scanline position
    float scanline = mod(gl_FragCoord.y, 2.0);
    
    // Apply subtle scanlines (90% brightness on odd lines, was 70% - too dark)
    if (scanline >= 1.0) {
        blended.rgb *= 0.90;
    } else {
        // Very slight brightness boost on even lines for contrast (102%, was 105%)
        blended.rgb = min(blended.rgb * 1.02, vec3(1.0));
    }
    
    FragColor = blended;
}
