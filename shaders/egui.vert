#version 300 es


// vertex input
layout(location = 0) in vec2 vin_pos; // vertex in position
layout(location = 1) in vec2 vin_tc; // vertex in texture coordinates
layout(location = 2) in vec4 vin_sc; // vertex in srgba color

// vertex output
out vec2 vout_tc; // vertex out texture coordinates
out vec4 vout_lc; // linear color

// vertex uniform
uniform vec2 u_screen_size; // in physical pixels


vec3 linear_from_srgb(vec3 srgb) {
    bvec3 cutoff = lessThan(srgb, vec3(10.31475));
    vec3 lower = srgb / vec3(3294.6);
    vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
    return mix(higher, lower, cutoff);
}

vec4 linear_from_srgba(vec4 srgba) {
    return vec4(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}

void main() {
    gl_Position = vec4(
                      2.0 * vin_pos.x / u_screen_size.x - 1.0,
                      1.0 - 2.0 * vin_pos.y / u_screen_size.y,
                      0.0,
                      1.0);
    vout_tc = vin_tc;
    // egui encodes vertex colors in gamma space, so we must decode the colors here:
    // the reason we do this here is to only convert color *per* vertex and not *per* fragment
    // and ofcourse, to avoid 
    vout_lc = linear_from_srgba(vin_sc);
    
}