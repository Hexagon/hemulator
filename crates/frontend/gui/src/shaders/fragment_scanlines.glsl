#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // Calculate scanline position - tighter scanlines with subtle darkening
    float scanline = mod(gl_FragCoord.y, 2.0);
    
    // Darken every other scanline subtly (85% brightness, was 60% - too dark)
    if (scanline >= 1.0) {
        color.rgb *= 0.85;
    }
    
    FragColor = color;
}
