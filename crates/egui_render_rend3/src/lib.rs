use egui_backend::{egui, GfxBackend, WindowBackend};
use egui_render_wgpu::{
    wgpu::{self, CommandEncoder, TextureFormat, TextureUsages},
    EguiPainter, SurfaceManager, WgpuBackend, WgpuConfig,
};
use rend3::{
    graph::{
        NodeResourceUsage, RenderGraph, RenderPassTarget, RenderPassTargets, RenderTargetHandle,
    },
    types::Color,
};

use std::{mem, sync::Arc};

use egui::TexturesDelta;
use rend3::{types::SampleCount, Renderer};

pub struct Rend3Backend {
    surface_manager: SurfaceManager,
    renderer: Arc<Renderer>,
    command_encoders: Vec<CommandEncoder>,
    painter: EguiPainter,
    screen_size: [f32; 2],
    surface_size: [u32; 2],
}

pub struct Rend3Config {}
impl Default for Rend3Config {
    fn default() -> Self {
        Self {}
    }
}

impl GfxBackend for Rend3Backend {
    type Configuration = Rend3Config;

    fn new(window_backend: &mut impl WindowBackend, config: Self::Configuration) -> Self {
        let iad = pollster::block_on(rend3::create_iad(
            None,
            None,
            Default::default(),
            Default::default(),
        ))
        .expect("failed to create iad in rend3");
        let aspect_ratio = window_backend
            .get_live_physical_size_framebuffer()
            .map(|wh| wh[0] as f32 / wh[1] as f32);

        let mut surface_manager = SurfaceManager::new(
            window_backend,
            &iad.instance,
            &iad.adapter,
            &iad.device,
            None,
            vec![],
            wgpu::SurfaceConfiguration {
                view_formats: vec![],
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                format: TextureFormat::Bgra8UnormSrgb,
                width: Default::default(),
                height: Default::default(),
                present_mode: Default::default(),
                alpha_mode: Default::default(),
            },
        );
        surface_manager.reconfigure_surface(
            window_backend,
            &iad.instance,
            &iad.adapter,
            &iad.device,
        );
        let renderer = Renderer::new(iad.clone(), Default::default(), aspect_ratio)
            .expect("failed to create renderer");
        let painter = EguiPainter::new(&iad.device, surface_manager.surface_config.format);
        let screen_size = window_backend.get_window_size().unwrap_or_default();
        let surface_size = [
            surface_manager.surface_config.width,
            surface_manager.surface_config.height,
        ];

        Self {
            surface_manager,
            renderer,
            painter,
            screen_size,
            surface_size,
            command_encoders: vec![],
        }
    }

    fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend) {
        self.surface_manager
            .resize_framebuffer(&self.renderer.device, window_backend);
    }

    fn prepare_frame(&mut self, window_backend: &mut impl WindowBackend) {
        self.surface_manager
            .create_current_surface_texture_view(window_backend, &self.renderer.device);
    }

    fn render_egui(
        &mut self,
        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
        let draw_calls = self.painter.upload_egui_data(
            &self.renderer.device,
            &self.renderer.queue,
            meshes,
            textures_delta,
            logical_screen_size,
            [
                self.surface_manager.surface_config.width,
                self.surface_manager.surface_config.height,
            ],
        );
        /*

        impl Rend3Backend {
            pub fn add_to_graph<'node>(
                &'node mut self,
                graph: &mut RenderGraph<'node>,
                mut input: Input<'node>,
                output: RenderTargetHandle,
            ) {

                let mut builder = graph.add_node("egui");

                let output_handle = builder.add_render_target(output, NodeResourceUsage::InputOutput);

                let rpass_handle = builder.add_renderpass(RenderPassTargets {
                    targets: vec![RenderPassTarget {
                        color: output_handle,
                        clear: Color::BLACK,
                        resolve: None,
                    }],
                    depth_stencil: None,
                });

                // We can't free textures directly after the call to `execute_with_renderpass` as it freezes
                // the lifetime of `self` for the remainder of the closure. so we instead buffer the textures
                // to free for a frame so we can clean them up before the next call.
                let textures_to_free = mem::replace(
                    &mut self.textures_to_free,
                    mem::take(&mut input.textures_delta.free),
                );

                builder.build(move |mut ctx| {
                    let rpass = ctx.encoder_or_pass.take_rpass(rpass_handle);


                    let mut cmd_buffer = ctx
                        .renderer
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                    self.internal.update_buffers(
                        &ctx.renderer.device,
                        &ctx.renderer.queue,
                        &mut cmd_buffer,
                        input.clipped_meshes,
                        &self.screen_descriptor,
                    );
                    drop(cmd_buffer);

                    self.internal
                        .render(rpass, input.clipped_meshes, &self.screen_descriptor);
                }
            }
        }

                 */
        let mut command_encoder =
            self.renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui command encoder"),
                });
        {
            let mut egui_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self
                        .surface_manager
                        .surface_view
                        .as_ref()
                        .expect("failed ot get surface view for egui render pass creation"),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            self.painter
                .draw_egui_with_renderpass(&mut egui_pass, draw_calls);
        }
        self.command_encoders.push(command_encoder);
    }

    fn present(&mut self, window_backend: &mut impl WindowBackend) {
        assert!(self.surface_manager.surface_view.is_some());
        self.renderer.queue.submit(
            std::mem::take(&mut self.command_encoders)
                .into_iter()
                .map(|encoder| encoder.finish()),
        );
        {
            self.surface_manager
                .surface_view
                .take()
                .expect("failed to get surface view to present");
        }
        self.surface_manager
            .surface_current_image
            .take()
            .expect("failed to surface texture to preset")
            .present();
    }
}
