#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // Sony Trinitron aperture grille simulation
    // Trinitron's distinctive feature: vertical RGB stripes instead of shadow mask dots
    
    // Calculate pixel position in screen space
    vec2 screenPos = vTexCoord * uResolution;
    
    // RGB stripe pattern (every 3 pixels)
    float stripe = mod(screenPos.x, 3.0);
    vec3 rgbMask = vec3(1.0);
    
    if (stripe < 1.0) {
        // Red stripe
        rgbMask = vec3(1.0, 0.9, 0.9);
    } else if (stripe < 2.0) {
        // Green stripe  
        rgbMask = vec3(0.9, 1.0, 0.9);
    } else {
        // Blue stripe
        rgbMask = vec3(0.9, 0.9, 1.0);
    }
    
    // Apply RGB mask
    color.rgb *= rgbMask;
    
    // Very subtle scanlines (Trinitron had fine scanlines)
    float scanline = mod(screenPos.y, 2.0);
    if (scanline >= 1.0) {
        color.rgb *= 0.92; // Very subtle darkening
    }
    
    // Bloom effect on bright pixels
    float brightness = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    if (brightness > 0.7) {
        // Calculate texel size
        vec2 texelSize = 1.0 / uResolution;
        
        // Sample neighbors for bloom
        vec4 bloom = vec4(0.0);
        bloom += texture(uTexture, vTexCoord + vec2(-texelSize.x, 0.0)) * 0.15;
        bloom += texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0)) * 0.15;
        bloom += texture(uTexture, vTexCoord + vec2(0.0, -texelSize.y)) * 0.15;
        bloom += texture(uTexture, vTexCoord + vec2(0.0, texelSize.y)) * 0.15;
        
        float bloomAmount = (brightness - 0.7) * 0.5;
        color.rgb += bloom.rgb * bloomAmount;
    }
    
    // Slight saturation boost (Trinitron was known for vivid colors)
    vec3 gray = vec3(dot(color.rgb, vec3(0.299, 0.587, 0.114)));
    color.rgb = mix(gray, color.rgb, 1.15);
    
    FragColor = vec4(clamp(color.rgb, 0.0, 1.0), 1.0);
}
