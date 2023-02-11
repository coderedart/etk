## Egui Backend


`egui` is an immediate mode gui library which can be embedded into existing framworks/game engines. The library basically takes `RawInput` as input and outputs `ClippedShapes` as output.
### RawInput
This is basically a struct containing window events like resize, mouse events like cursor position / clicks, keyboard events like key press/release/ IME input etc..
Most of these events come from a Window Backend like `winit`, `glfw`, `sdl2` etc.. which are cross platform windowing libraries.

### Clipped Shapes
These are basically gpu friendly data structs which need to be rendered to the screen via a graphics backend.
This can be a low level graphics like `opengl`, `vulkan` etc.. or higher level abstractions like `wgpu` or `vulkano`. Or even higher abstractions like 2d renderers eg: `skia`

`egui_backend` crate tries to reduce these backends into a trait implementation. This allows one to write an egui app, and swap out the backends as needed.

### Eframe
egui already has an official backend crate called `eframe`. It uses winit on desktop and a custom written js-based backend on web. 
If winit works good enough, it is recommended to stick to eframe. 

### Usecases
#### Workaround bugs using alternate backends
If you are using winit, and suddenly some users are complaining about a crash on some specific configuration like fedora + nvidia. we have no idea how long it will take to fix that crash upstream. So, you immediately switch the backend to some alternative window backend like `glfw` which might not have this specific bug. This can be released as a different version or you can include both backends and based on user configuration, decide at startup which backend to use. 
If you realized that it was a vulkan bug, you can immediately switch to an opengl backend too in that same way as above.

#### Expose internals
If you don't particularly care about multiple backends, you can use internals of a windowing backend getting accesss to its "window" struct or raw "events" list etc..
For example, `glfw-passthrough` crate has a feature to make windows passthrough which is very useful for overlays. If you are making an overlay, there's no point in abstracting over other backends. This allows a single crate to serve both usecases. 

#### Serve as Modular reference implementations
Every attempt at creating a custom backend will have bugs like not knowing whether a size is in physical or logical pixels/coordinates. But having a decent reference implementation will serve as a launch platform for new backend implementations.
At the same time, devs can always reuse some items from the crates. like `key` or `mousebutton` or other event converting functions. 

#### Separate Gfx and Windowing APIs
egui_backend is trying to separate out the api boundaries using traits. This will allow crates like wgpu/glow/three-d/rend3 and other gfx backends to only implement the GfxBackend trait. winit/sdl2/glfw will also only implement a WindowBackend trat. this will enable them to work with each other, without no (or minimal) glue code. So, if someone wants to create a new backend using a crate like vulkano or ash or erupt, they will only have to implement this trait and then allow it to work with windowing crates like winit/sdl2/glfw.

### Limitations
There are some features which are explicitly excluded to keep this simple. 
1. no multiple windows. gfx and window apis need to sync window and swapchain lifecycle synchronization. This is niche requirement which shouldn't affect the ergonomics of the vast majority of other users.

For now, we will just assume that there's atmost one window at any point in the app's lifecycle.
