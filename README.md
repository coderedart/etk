

[![checkmate](https://github.com/coderedart/etk/actions/workflows/rust_ci.yml/badge.svg)](https://github.com/coderedart/etk/actions/workflows/rust_ci.yml)

# Egui Toolkit
Supporting crates for egui

Egui is a small ui library that you embed into your App. And it requires the App to provide input like mouse/keyboard. Once it processes the input, egui will provide you with a bunch of data which you need to draw on the display.

`egui_backend` is a crate that abstracts away the input requirements as a `WindowBackend` trait and output drawing stuff as `GfxBackend`. This allows you to implement these traits for specific libraries like winit, glfw, wgpu, vulkan etc.. And any user can easily reuse these backends.

Most App devs only need to take care of implementing the `UserApp` trait and choose the backends they want to use. If you don't use advanced stuff, then you can just swap the backends with barely a couple lines of code.

```rust
// This is the user struct where we can store any data we want along with the window and gfx backends.
pub struct App {
    pub frame_count: usize,
    pub egui_context: egui::Context,
    pub glow_backend: GlowBackend,
    pub glfw_backend: GlfwBackend,
}
// we need to implement this trait for our struct
impl UserApp for App {
    // we can make it generic, but didn't want to complicate the example.
    type UserGfxBackend = GlowBackend;
    type UserWindowBackend = GlfwBackend;
    // The function which is used by some default fn impl of this trait
    // allows us access to window/gfx backends mutably at the same time
    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    ) {
        (
            &mut self.glfw_backend,
            &mut self.glow_backend,
            &self.egui_context,
        )
    }
    // here, you put your gui code, which will be run every frame.
    fn gui_run(&mut self) {
        let egui_context = self.egui_context.clone();
        egui::Window::new("egui window").show(&egui_context, |ui| {
            ui.label(format!("frame number: {}", self.frame_count));
        });
    }
}

pub fn fake_main() {
    // create window backend with default config. here you can set initial flags like transparency or selecting opengl vs vulkan etc..
    let mut glfw_backend = GlfwBackend::new(Default::default(), BackendConfig::default());
    // creating gfx backend. It uses Window backend to load things like fn pointers or window handle for swapchain etc.. behind the scenes.
    let glow_backend = GlowBackend::new(&mut glfw_backend, Default::default());
    // initialize app state
    let app = App {
    frame_count: 0,
    egui_context: Default::default(),
    glow_backend,
    glfw_backend,
    };
    // enter event loop. Now, the `gui_run` method from trait impl will be called every frame. read docs for more info
    <App as UserApp>::UserWindowBackend::run_event_loop(app);
}
```




