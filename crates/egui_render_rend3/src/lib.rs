use egui_backend::{egui, GfxBackend, WindowBackend};
use egui_render_wgpu::{
    wgpu::{self, CommandEncoder, TextureFormat, TextureUsages},
    EguiPainter, SurfaceManager,
};

use std::sync::Arc;

use rend3::Renderer;

pub struct Rend3Backend {
    surface_manager: SurfaceManager,
    renderer: Arc<Renderer>,
    painter: EguiPainter,
    command_encoders: Vec<CommandEncoder>,
}

pub struct Rend3Config {}
impl Default for Rend3Config {
    fn default() -> Self {
        Self {}
    }
}

impl GfxBackend for Rend3Backend {
    type Configuration = Rend3Config;

    fn new(window_backend: &mut impl WindowBackend, _config: Self::Configuration) -> Self {
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

        Self {
            surface_manager,
            renderer,
            painter,
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

    fn present(&mut self, _window_backend: &mut impl WindowBackend) {
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
