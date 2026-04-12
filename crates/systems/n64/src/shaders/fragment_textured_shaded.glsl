#version 330 core

// Fragment shader for textured+shaded triangles (colour-combiner MODULATE mode)
// Computes: out_colour = texel_colour * shade_colour
// This implements the most common N64 colour-combiner mode G_CC_MODULATERGB /
// G_CC_MODULATERGBA which multiplies a texture sample by the Gouraud-
// interpolated vertex shade colour (including the effect of lighting).

uniform sampler2D uTexture;

in vec2 vTexCoord;
in vec4 vColor;

out vec4 FragColor;

void main() {
    vec4 texColor = texture(uTexture, vTexCoord);
    FragColor = texColor * vColor;
}
