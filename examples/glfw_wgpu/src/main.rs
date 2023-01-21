use egui_backend::{
    egui::{self, RawInput, Window},
    BackendConfig, GfxApiType, GfxBackend, UserAppData, WindowBackend,
};
use egui_render_wgpu::{
    wgpu,
    wgpu::{Device, RenderPipeline, TextureFormat},
    WgpuBackend,
};
use egui_window_glfw_passthrough::GlfwBackend;
use std::borrow::Cow;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
struct App {
    pipeline: RenderPipeline,
    frame_count: usize,
}

impl UserAppData<GlfwBackend, WgpuBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        raw_input: RawInput,
        _window_backend: &mut GlfwBackend,
        gfx_backend: &mut WgpuBackend,
    ) -> egui::FullOutput {
        egui_context.begin_frame(raw_input);
        // draw a triangle
        self.draw_triangle(gfx_backend);
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
        });
        egui_context.end_frame()
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

    fn draw_triangle(&self, gfx_backend: &mut WgpuBackend) {
        let mut encoder = gfx_backend
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: gfx_backend.surface_view.as_ref().unwrap(),
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
        gfx_backend.command_encoders.push(encoder);
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

pub fn fake_main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut window_backend = GlfwBackend::new(
        Default::default(),
        BackendConfig {
            gfx_api_type: GfxApiType::NoApi,
        },
    );

    let wgpu_backend = WgpuBackend::new(&mut window_backend, Default::default());
    let app = App::new(&wgpu_backend.device, wgpu_backend.surface_config.format);
    window_backend.run_event_loop(wgpu_backend, app);
}

fn main() {
    fake_main();
}
