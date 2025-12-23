#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // Sharp LCD - Early passive matrix LCD from 1980s
    
    // Convert to monochrome
    float lum = dot(color.rgb, vec3(0.299, 0.587, 0.114));
    
    // Reduce contrast (compress dynamic range)
    lum = 0.25 + lum * 0.5;
    
    // LCD has slight blue/green tint
    vec3 lcdColor = vec3(0.9, 1.0, 0.95) * lum;
    
    // Motion blur (LCD persistence/ghosting)
    vec2 texelSize = 1.0 / uResolution;
    vec4 blurred = vec4(0.0);
    
    // 3x3 box blur
    for (float dy = -1.0; dy <= 1.0; dy += 1.0) {
        for (float dx = -1.0; dx <= 1.0; dx += 1.0) {
            vec2 offset = vec2(dx, dy) * texelSize;
            blurred += texture(uTexture, vTexCoord + offset);
        }
    }
    blurred /= 9.0;
    
    float blurredLum = dot(blurred.rgb, vec3(0.299, 0.587, 0.114));
    blurredLum = 0.25 + blurredLum * 0.5;
    lcdColor = vec3(0.9, 1.0, 0.95) * blurredLum;
    
    // Pixel grid (LCD has visible gaps)
    vec2 screenPos = vTexCoord * uResolution;
    float gridX = mod(screenPos.x, 3.0);
    float gridY = mod(screenPos.y, 3.0);
    
    if (gridX >= 2.0 || gridY >= 2.0) {
        // Darken pixel borders
        lcdColor *= 0.7;
    }
    
    // Additional low contrast characteristic
    vec3 midGray = vec3(0.5);
    lcdColor = mix(midGray, lcdColor, 0.8);
    
    FragColor = vec4(clamp(lcdColor, 0.0, 1.0), 1.0);
}
