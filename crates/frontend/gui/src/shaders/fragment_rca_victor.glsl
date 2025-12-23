#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // RCA Victor - Vintage B/W CRT TV from 1950s-60s
    
    // Convert to grayscale
    float lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    
    // Calculate vignette (rounded corners darkening)
    vec2 center = vec2(0.5, 0.5);
    float dist = distance(vTexCoord, center);
    float vignette = 1.0 - (dist * 0.8); // 40% darkening at edges
    vignette = clamp(vignette, 0.0, 1.0);
    
    lum *= vignette;
    
    // Heavy scanlines (vintage CRTs had very prominent scanlines)
    vec2 screenPos = vTexCoord * uResolution;
    float scanline = mod(screenPos.y, 2.0);
    if (scanline >= 1.0) {
        lum *= 0.5; // 50% brightness on scanlines
    }
    
    // Reduce overall contrast for vintage look
    // Compress dynamic range (vintage TVs had limited contrast)
    lum = 0.125 + lum * 0.75;
    
    // Slight horizontal blur (poor horizontal resolution)
    vec2 texelSize = 1.0 / uResolution;
    float lumLeft = dot(texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0)).rgb, vec3(0.299, 0.587, 0.114));
    float lumRight = dot(texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0)).rgb, vec3(0.299, 0.587, 0.114));
    
    lum = lum * 0.6 + lumLeft * 0.2 + lumRight * 0.2;
    
    // Apply vignette again after blur
    lum *= vignette;
    
    // Slight warm tint (old B/W TVs had slightly warm phosphor)
    vec3 bwColor = vec3(lum * 1.02, lum * 1.0, lum * 0.98);
    
    FragColor = vec4(clamp(bwColor, 0.0, 1.0), 1.0);
}
