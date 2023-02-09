use egui_backend::{
    egui::{self, Window},
    BackendConfig, EguiUserApp, GfxBackend, WindowBackend,
};
use egui_render_wgpu::{
    wgpu::RenderPipeline,
    wgpu::{self, Backends, BlendState, ColorTargetState, ColorWrites},
    WgpuBackend, WgpuConfig,
};
use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::borrow::Cow;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
struct App {
    pipeline: RenderPipeline,
    frame_count: usize,
    egui_wants_input: bool,
    is_window_receiving_events: bool,
    egui_context: egui::Context,
    wgpu_backend: WgpuBackend,
}

impl EguiUserApp<GlfwBackend> for App {
    fn gui_run(&mut self, egui_context: &egui::Context, window_backend: &mut GlfwBackend) {
        self.frame_count += 1;
        // draw a triangle
        self.draw_triangle();
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            ui.checkbox(
                &mut self.is_window_receiving_events,
                "Is Window receiving events?",
            );
            ui.checkbox(&mut self.egui_wants_input, "Does egui want input?");
        });

        self.is_window_receiving_events = !window_backend.window.is_mouse_passthrough();
        // don't forget to only ask egui if it wants input AFTER ending the frame
        self.egui_wants_input =
            egui_context.wants_pointer_input() || egui_context.wants_keyboard_input();
        // if window is receiving events when egui doesn't want input. or if window not receiving events when egui wants input.
        if self.is_window_receiving_events != self.egui_wants_input {
            window_backend
                .window
                .set_mouse_passthrough(!self.egui_wants_input); // passthrough means not receiving events. so, if egui wants input, we set passthrough to false. otherwise true.
        }
    }

    type UserGfxBackend = WgpuBackend;

    fn get_gfx_backend(&mut self) -> &mut Self::UserGfxBackend {
        &mut self.wgpu_backend
    }

    fn get_egui_context(&mut self) -> egui::Context {
        self.egui_context.clone()
    }
}
impl App {
    pub fn new(window_backend: &mut GlfwBackend) -> Self {
        let wgpu_backend = WgpuBackend::new(
            window_backend,
            WgpuConfig {
                backends: Backends::VULKAN,
                ..Default::default()
            },
        );
        let device = wgpu_backend.device.clone();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(TRIANGLE_SHADER_SRC)),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("triangle pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("triangle pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: wgpu_backend.surface_manager.surface_config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        Self {
            pipeline: render_pipeline,
            frame_count: 0,
            egui_wants_input: false,
            is_window_receiving_events: false,
            egui_context: Default::default(),
            wgpu_backend,
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
const TRIANGLE_SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec4<f32>,
};
@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput
{
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    var output: VertexOutput;
    output.pos = vec4<f32>(x, y, 0.0, 1.0);
    output.ndc = output.pos;
    return output;    
}

@fragment
fn fs_main(output: VertexOutput) -> @location(0) vec4<f32> {
    let ndc = output.ndc;
    return abs(vec4<f32>(ndc.x, ndc.y, ndc.x * ndc.y, 0.7));
}"#;

pub fn fake_main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut window_backend = GlfwBackend::new(
        GlfwConfig {
            glfw_callback: Box::new(|glfw_context| {
                // make the window that will be created transparent.
                glfw_context.window_hint(
                    egui_window_glfw_passthrough::glfw::WindowHint::TransparentFramebuffer(true),
                );
                glfw_context.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::Floating(
                    true,
                ));
                egui_window_glfw_passthrough::default_glfw_callback(glfw_context);
            }),
            ..Default::default()
        },
        BackendConfig {},
    );

    let app = App::new(&mut window_backend);
    window_backend.run_event_loop(app);
}

fn main() {
    fake_main();
}
