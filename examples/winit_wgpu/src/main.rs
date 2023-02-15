use egui_backend::{
    egui::{self, Window},
    GfxBackend, UserApp, WindowBackend,
};
use egui_render_wgpu::{wgpu, wgpu::RenderPipeline, WgpuBackend};
use egui_window_winit::WinitBackend;
use std::borrow::Cow;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
struct App {
    pipeline: RenderPipeline,
    frame_count: usize,
    egui_context: egui::Context,
    wgpu_backend: WgpuBackend,
    window_backend: WinitBackend,
}

impl UserApp for App {
    fn gui_run(&mut self) {
        let egui_context = self.egui_context.clone();
        let egui_context = &&egui_context;
        // draw a triangle
        self.draw_triangle();
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
        });
    }

    type UserGfxBackend = WgpuBackend;

    type UserWindowBackend = WinitBackend;

    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    ) {
        (
            &mut self.window_backend,
            &mut self.wgpu_backend,
            &self.egui_context,
        )
    }
}
impl App {
    pub fn new(wgpu_backend: WgpuBackend, window_backend: WinitBackend) -> Self {
        let device = wgpu_backend.device.clone();
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
                targets: &[Some(
                    wgpu_backend.surface_manager.surface_config.format.into(),
                )],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        Self {
            pipeline: render_pipeline,
            frame_count: 0,
            egui_context: Default::default(),
            wgpu_backend,
            window_backend,
        }
    }

    fn draw_triangle(&mut self) {
        let mut encoder = self
            .wgpu_backend
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self
                        .wgpu_backend
                        .surface_manager
                        .surface_view
                        .as_ref()
                        .unwrap(),
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
        self.wgpu_backend.command_encoders.push(encoder);
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
    let mut window_backend = WinitBackend::new(Default::default(), Default::default());

    let wgpu_backend = WgpuBackend::new(&mut window_backend, Default::default());
    let app = App::new(wgpu_backend, window_backend);
    <App as UserApp>::UserWindowBackend::run_event_loop(app);
}

fn main() {
    fake_main();
}
