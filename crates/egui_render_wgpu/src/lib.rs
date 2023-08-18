mod painter;
mod surface;

use egui_backend::egui;
use egui_backend::{GfxBackend, WindowBackend};
use raw_window_handle::HasRawWindowHandle;
use std::sync::Arc;
use tracing::{debug, info};
use wgpu::*;

pub use painter::*;
pub use surface::SurfaceManager;
pub use wgpu;

pub struct WgpuConfig {
    pub backends: Backends,
    pub power_preference: PowerPreference,
    pub device_descriptor: DeviceDescriptor<'static>,
    /// If not empty, We will try to iterate over this vector and use the first format that is supported by the surface.
    /// If this is empty or none of the formats in this vector are supported, we will just use the first supported format of the surface.
    pub surface_formats_priority: Vec<TextureFormat>,
    /// we will try to use this config if supported. otherwise, the surface recommended options will be used.   
    pub surface_config: SurfaceConfiguration,
}
impl Default for WgpuConfig {
    fn default() -> Self {
        Self {
            backends: Backends::all(),
            power_preference: PowerPreference::default(),
            device_descriptor: DeviceDescriptor {
                label: Some("my wgpu device"),
                features: Default::default(),
                limits: Limits::downlevel_webgl2_defaults(),
            },
            surface_config: SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: TextureFormat::Bgra8UnormSrgb,
                width: 0,
                height: 0,
                present_mode: PresentMode::Fifo,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
            surface_formats_priority: vec![],
        }
    }
}
/// This provides a Gfx backend for egui using wgpu as the backend
/// If you are making your own wgpu integration, then you can reuse the `EguiPainter` instead which contains only egui render specific data.
pub struct WgpuBackend {
    /// wgpu instance
    pub instance: Arc<Instance>,
    /// wgpu adapter
    pub adapter: Arc<Adapter>,
    /// wgpu device.
    pub device: Arc<Device>,
    /// wgpu queue. if you have commands that you would like to submit, instead push them into `Self::command_encoders`
    pub queue: Arc<Queue>,
    /// contains egui specific wgpu data like textures or buffers or pipelines etc..
    pub painter: EguiPainter,
    pub surface_manager: SurfaceManager,
    /// this is where we store our command encoders. we will create one during the `prepare_frame` fn.
    /// users can just use this. or create new encoders, and push them into this vec.
    /// `wgpu::Queue::submit` is very expensive, so we will submit ALL command encoders at the same time during the `present_frame` method
    /// just before presenting the swapchain image (surface texture).
    pub command_encoders: Vec<CommandEncoder>,
}
impl Drop for WgpuBackend {
    fn drop(&mut self) {
        tracing::warn!("dropping wgpu backend");
    }
}
impl WgpuBackend {
    pub async fn new_async(
        window_backend: &mut impl WindowBackend,
        config: <Self as GfxBackend>::Configuration,
    ) -> Self {
        let WgpuConfig {
            power_preference,
            device_descriptor,
            surface_formats_priority,
            surface_config,
            backends,
        } = config;
        debug!("using wgpu backends: {:?}", backends);
        let instance = Arc::new(Instance::new(InstanceDescriptor {
            backends,
            dx12_shader_compiler: Default::default(),
        }));
        debug!("iterating over all adapters");
        #[cfg(not(target_arch = "wasm32"))]
        for adapter in instance.enumerate_adapters(Backends::all()) {
            debug!("adapter: {:#?}", adapter.get_info());
        }

        let surface = window_backend.get_window().map(|w| unsafe {
            tracing::debug!("creating a surface with {:?}", w.raw_window_handle());
            instance
                .create_surface(w)
                .expect("failed to create surface")
        });

        info!("is surfaced created at startup?: {}", surface.is_some());

        debug!("using power preference: {:?}", config.power_preference);
        let adapter = Arc::new(
            instance
                .request_adapter(&RequestAdapterOptions {
                    power_preference,
                    force_fallback_adapter: false,
                    compatible_surface: surface.as_ref(),
                })
                .await
                .expect("failed to get adapter"),
        );

        info!("chosen adapter details: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&device_descriptor, Default::default())
            .await
            .expect("failed to create wgpu device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_manager = SurfaceManager::new(
            window_backend,
            &instance,
            &adapter,
            &device,
            surface,
            surface_formats_priority,
            surface_config,
        );

        debug!("device features: {:#?}", device.features());
        debug!("device limits: {:#?}", device.limits());

        let painter = EguiPainter::new(&device, surface_manager.surface_config.format);

        Self {
            instance,
            adapter,
            device,
            queue,
            painter,
            command_encoders: Vec::new(),
            surface_manager,
        }
    }
}
impl GfxBackend for WgpuBackend {
    type Configuration = WgpuConfig;

    fn new(window_backend: &mut impl WindowBackend, config: Self::Configuration) -> Self {
        pollster::block_on(Self::new_async(window_backend, config))
    }

    fn resume(&mut self, window_backend: &mut impl WindowBackend) {
        self.surface_manager.reconfigure_surface(
            window_backend,
            &self.instance,
            &self.adapter,
            &self.device,
        );
        self.painter.on_resume(
            &self.device,
            self.surface_manager
                .surface_config
                .view_formats
                .first()
                .copied()
                .unwrap(),
        );
    }

    fn prepare_frame(&mut self, window_backend: &mut impl WindowBackend) {
        self.surface_manager
            .create_current_surface_texture_view(window_backend, &self.device);
        if let Some(view) = self.surface_manager.surface_view.as_ref() {
            let mut ce = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: "surface clear ce".into(),
                });
            ce.begin_render_pass(&RenderPassDescriptor {
                label: "surface clear rpass".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            self.command_encoders.push(ce);
        }
    }

    fn render_egui(
        &mut self,
        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
        let mut command_encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("egui command encoder"),
            });
        let draw_calls = self.painter.upload_egui_data(
            &self.device,
            &self.queue,
            meshes,
            textures_delta,
            logical_screen_size,
            [
                self.surface_manager.surface_config.width,
                self.surface_manager.surface_config.height,
            ],
            &mut command_encoder,
        );
        {
            let mut egui_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: self
                        .surface_manager
                        .surface_view
                        .as_ref()
                        .expect("failed ot get surface view for egui render pass creation"),
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
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
        self.queue.submit(
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

    fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend) {
        self.surface_manager
            .resize_framebuffer(&self.device, window_backend);
    }

    fn suspend(&mut self, _window_backend: &mut impl WindowBackend) {
        self.surface_manager.suspend();
    }
}
