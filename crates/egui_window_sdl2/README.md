### Sdl2 window backend for egui

### Emscripten 
on emscripten target, sdl2 crate will set raw window handle id to `1` by hardcoding it.
reference: https://github.com/Rust-SDL2/rust-sdl2/blob/master/src/sdl2/raw_window_handle.rs#L18
so, make sure that the relevant `data-raw-handle` property value on canvas to `1`.
