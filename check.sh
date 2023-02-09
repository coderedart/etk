#!/bin/sh

set -ex
# wasm32-unknown-unknown build-able crates are egui_window_winit, egui_render_wgpu, egui_render_glow, egui_render_three_d, winit_wgpu
# wasm32-unknown-emscripten crates are egui_window_sdl2, egui_render_glow, egui_render_three_d
# all are desktop crates
echo "starting check.sh desktop command list"
# desktop commands
echo "fmt"
cargo fmt --all --check
echo "check"
cargo check --workspace
echo "clippy"
cargo clippy --workspace -- -D warnings
echo "build"
cargo build --workspace
echo "test"
cargo test --workspace

# wasm32-unknown-unknown commands
echo "starting wasm32 unknonw unknown list"
echo "check"
cargo check -p egui_window_winit -p egui_render_wgpu -p egui_render_glow -p egui_render_three_d -p winit_wgpu --target=wasm32-unknown-unknown
echo "clippy"
cargo clippy -p egui_window_winit -p egui_render_wgpu -p egui_render_glow -p egui_render_three_d -p winit_wgpu --target=wasm32-unknown-unknown -- -D warnings
echo "build"
cargo build -p egui_window_winit -p egui_render_wgpu -p egui_render_glow -p egui_render_three_d -p winit_wgpu --target=wasm32-unknown-unknown

# emscripten commands
echo "starting emscripten list"
echo "check"
cargo check -p egui_window_sdl2 -p egui_render_glow -p egui_render_three_d --target=wasm32-unknown-emscripten
echo "clippy"
cargo clippy -p egui_window_sdl2 -p egui_render_glow -p egui_render_three_d --target=wasm32-unknown-emscripten
echo "build sdl2_glow." # need to cd into dir because we need the linker flags
(cd examples/sdl2_glow && cargo build -p sdl2_glow --target=wasm32-unknown-emscripten)



