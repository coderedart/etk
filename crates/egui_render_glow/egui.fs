#version 300 es

precision highp float;
// fragment shader uniforms. texture and sampler
uniform sampler2D u_sampler;

// fragment inputs
in vec2 vout_tc;
in vec4 vout_lc; // linear or srgba?

out vec4 fout_color;
void main() {
    fout_color = vout_lc * texture(u_sampler, vout_tc);
    
}