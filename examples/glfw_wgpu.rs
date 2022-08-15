use std::borrow::Cow;

use egui::Window;
use egui_backend::{GfxApiConfig, GfxBackend, UserApp, WindowBackend};
use egui_render_wgpu::wgpu;
use egui_render_wgpu::{
    wgpu::{Device, Queue, RenderPipeline, TextureFormat},
    WgpuBackend,
};
use egui_window_glfw::{GlfwConfig, GlfwWindow};
struct App {
    pipeline: RenderPipeline,
    frame_count: usize,
}

impl UserApp<GlfwWindow, WgpuBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut GlfwWindow,
        gfx_backend: &mut WgpuBackend,
    ) {
        self.draw_triangle(
            &gfx_backend.device,
            &gfx_backend.queue,
            gfx_backend.surface_view.as_ref().expect("no surface view"),
        );
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
        });
    }
}
impl App {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(TRIANGLE_SHADER_SRC)),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(surface_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        Self {
            pipeline: render_pipeline,
            frame_count: 0,
        }
    }

    fn draw_triangle(&self, device: &Device, queue: &Queue, view: &wgpu::TextureView) {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
    }
}
const TRIANGLE_SHADER_SRC: &str = r#"@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}"#;
fn main() {
    tracing_subscriber::fmt().init();
    let glfw_config = GlfwConfig {
        glfw_callback: Some(Box::new(|glfw_context| {
            glfw_context.window_hint(egui_window_glfw::glfw::WindowHint::TransparentFramebuffer(
                true,
            ));
        })),
    };
    let (glfw_backend, window_info_for_gfx) = GlfwWindow::new(glfw_config, GfxApiConfig::Vulkan {});
    let wgpu_backend = WgpuBackend::new(window_info_for_gfx, Default::default());
    let app = App::new(&wgpu_backend.device, wgpu_backend.surface_config.format);
    glfw_backend.run_event_loop(wgpu_backend, app);
}
