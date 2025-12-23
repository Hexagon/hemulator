#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // Commodore 1702 color monitor - popular for C64/Amiga
    
    // Calculate pixel position
    vec2 screenPos = vTexCoord * uResolution;
    
    // Shadow mask pattern (triangular RGB dots)
    float dotX = mod(screenPos.x, 3.0);
    float dotY = mod(screenPos.y, 3.0);
    
    vec3 mask = vec3(1.0);
    
    // Create triangular shadow mask pattern
    if (dotX < 1.0 && dotY < 1.0) {
        mask = vec3(1.0, 0.94, 0.94); // Red dot
    } else if (dotX >= 1.0 && dotX < 2.0 && dotY >= 1.0 && dotY < 2.0) {
        mask = vec3(0.94, 1.0, 0.94); // Green dot
    } else if (dotX >= 2.0 && dotY >= 2.0) {
        mask = vec3(0.94, 0.94, 1.0); // Blue dot
    }
    
    color.rgb *= mask;
    
    // Moderate phosphor glow
    vec2 texelSize = 1.0 / uResolution;
    vec4 left = texture(uTexture, vTexCoord - vec2(texelSize.x, 0.0));
    vec4 right = texture(uTexture, vTexCoord + vec2(texelSize.x, 0.0));
    
    // Medium horizontal bleeding (15% from neighbors)
    color.rgb = color.rgb * 0.85 + left.rgb * 0.075 + right.rgb * 0.075;
    
    // Moderate scanlines (75% brightness on dark lines)
    float scanline = mod(screenPos.y, 2.0);
    if (scanline >= 1.0) {
        color.rgb *= 0.75;
    }
    
    // Slight color bleeding for composite-like effect
    // (Many users connected C64 via composite)
    vec4 leftColor = texture(uTexture, vTexCoord - vec2(texelSize.x * 2.0, 0.0));
    color.rgb += leftColor.rgb * 0.05;
    
    FragColor = vec4(clamp(color.rgb, 0.0, 1.0), 1.0);
}
