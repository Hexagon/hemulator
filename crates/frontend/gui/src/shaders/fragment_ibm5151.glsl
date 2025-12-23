#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // IBM 5151 green phosphor monochrome monitor
    
    // Convert to luminance
    float lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    
    // Apply green phosphor characteristic
    // IBM 5151 had P39 phosphor (green)
    vec3 green = vec3(0.12, 1.0, 0.16);
    color.rgb = green * lum;
    
    // Phosphor glow (horizontal bleeding)
    vec2 texelSize = 1.0 / uResolution;
    vec4 left = texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0));
    vec4 right = texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0));
    
    float lumLeft = dot(left.rgb, vec3(0.299, 0.587, 0.114));
    float lumRight = dot(right.rgb, vec3(0.299, 0.587, 0.114));
    
    // Strong glow effect (20% from neighbors)
    float glowLum = lum * 0.6 + lumLeft * 0.2 + lumRight * 0.2;
    color.rgb = green * glowLum;
    
    // Moderate scanlines
    vec2 screenPos = vTexCoord * uResolution;
    float scanline = mod(screenPos.y, 2.0);
    if (scanline >= 1.0) {
        color.rgb *= 0.8;
    }
    
    // Phosphor persistence (add slight glow from previous frames)
    // This simulates the characteristic green afterglow
    color.rgb *= 1.1; // Boost brightness slightly
    
    FragColor = vec4(clamp(color.rgb, 0.0, 1.0), 1.0);
}
