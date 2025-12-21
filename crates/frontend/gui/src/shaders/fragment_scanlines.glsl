#version 330 core

in vec2 vTexCoord;
out vec4 FragColor;

uniform sampler2D uTexture;
uniform vec2 uResolution;

void main() {
    vec4 color = texture(uTexture, vTexCoord);
    
    // Calculate scanline position
    float scanline = mod(gl_FragCoord.y, 2.0);
    
    // Darken every other scanline (60% brightness)
    if (scanline >= 1.0) {
        color.rgb *= 0.6;
    }
    
    FragColor = color;
}
