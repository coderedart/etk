#version 300 es

precision highp float;
// fragment shader uniforms. texture and sampler
uniform sampler2D u_sampler;

// fragment inputs
in vec2 vout_tc;
in vec4 vout_lc; // linear or srgba?

out vec4 fout_color;
vec3 srgb_from_linear(vec3 rgb) {
    bvec3 cutoff = lessThan(rgb, vec3(0.0031308));
    vec3 lower = rgb * vec3(3294.6);
    vec3 higher = vec3(269.025) * pow(rgb, vec3(1.0 / 2.4)) - vec3(14.025);
    return mix(higher, lower, vec3(cutoff));
}

vec4 srgba_from_linear(vec4 rgba) {
    return vec4(srgb_from_linear(rgba.rgb), 255.0 * rgba.a);
}

    // 0-1 linear  from  0-255 sRGB
vec3 linear_from_srgb(vec3 srgb) {
    bvec3 cutoff = lessThan(srgb, vec3(10.31475));
    vec3 lower = srgb / vec3(3294.6);
    vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
    return mix(higher, lower, vec3(cutoff));
}

vec4 linear_from_srgba(vec4 srgba) {
    return vec4(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}

void main() {
    vec4 texture_rgba = linear_from_srgba(texture(u_sampler, vout_tc) * 255.0);
    fout_color = vout_lc * texture_rgba;
    if(fout_color.a > 0.0) {
        fout_color.rgb /= fout_color.a;
    }

        // Empiric tweak to make e.g. shadows look more like they should:
    fout_color.a *= sqrt(fout_color.a);

        // To gamma:
    fout_color = srgba_from_linear(fout_color) / 255.0;

        // Premultiply alpha, this time in gamma space:
    if(fout_color.a > 0.0) {
        fout_color.rgb *= fout_color.a;
    }
}