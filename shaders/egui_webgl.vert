#version 100
precision mediump float;

uniform vec2 u_screen_size;
attribute vec2 vin_pos;
attribute vec2 vin_tc;
attribute vec4 vin_sc; // 0-255 sRGB

varying vec4 vout_srgba;
varying vec2 vout_tc;

void main() {
    gl_Position = vec4(
                      2.0 * vin_pos.x / u_screen_size.x - 1.0,
                      1.0 - 2.0 * vin_pos.y / u_screen_size.y,
                      0.0,
                      1.0);
    vout_srgba = vin_sc / 255.0;
    vout_tc = vin_tc;
}
