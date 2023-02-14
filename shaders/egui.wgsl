// Vertex shader bindings

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u_screen_size: vec4<f32>;


@vertex
fn vs_main(
    @location(0) a_pos: vec2<f32>,
    @location(1) a_tex_coord: vec2<f32>,
    @location(2) a_color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;
    out.color = vec4<f32>(linear_from_srgb(a_color.rgb), a_color.a);

    out.position = vec4<f32>(
        2.0 * a_pos.x / u_screen_size.x - 1.0,
        1.0 - 2.0 * a_pos.y / u_screen_size.y,
        0.5,
        1.0,
    );

    return out;
}



// Fragment shader bindings
@group(1) @binding(0) var r_tex_sampler: sampler;
@group(1) @binding(1) var r_tex_color: texture_2d<f32>;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let in_color = gamma_from_linear_rgba(in.color);

    let tex_linear = textureSample(r_tex_color, r_tex_sampler, in.tex_coord);

    let tex_gamma = gamma_from_linear_rgba(tex_linear);

    let out_color_gamma = in_color * tex_gamma;
    return vec4<f32>(linear_from_srgb(out_color_gamma.rgb), out_color_gamma.a);
}


// 0-1 sRGB gamma  from  0-1 linear
fn gamma_from_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
    let cutoff = rgb < vec3<f32>(0.0031308);
    let lower = rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

// 0-1 sRGBA gamma  from  0-1 linear
fn gamma_from_linear_rgba(linear_rgba: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(gamma_from_linear_rgb(linear_rgba.rgb), linear_rgba.a);
}

fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}