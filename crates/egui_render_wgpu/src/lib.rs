use bytemuck::cast_slice;
use egui::{
    epaint::ImageDelta, util::IdTypeMap, ClippedPrimitive, Mesh, PaintCallback, PaintCallbackInfo,
    Rect, TextureId,
};
use egui_backend::egui;
use egui_backend::{GfxBackend, WindowBackend};
use raw_window_handle::HasRawWindowHandle;
use std::collections::BTreeMap;
use std::{convert::TryInto, num::NonZeroU64, sync::Arc};
use tracing::{debug, info};
use wgpu::*;

pub use wgpu;

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
pub struct SurfaceManager {
    /// we create a view for the swapchain image and set it to this field during the `prepare_frame` fn.
    /// users can assume that it will *always* be available during the `UserApp::run` fn. but don't keep any references as
    /// it will be taken and submitted during the `present_frame` method after rendering is done.
    /// surface is always cleared by wgpu, so no need to wipe it again.
    pub surface_view: Option<TextureView>,
    /// once we acquire a swapchain image (surface texture), we will put it here. surface_view will be created from this
    pub surface_current_image: Option<SurfaceTexture>,
    /// this is the window surface
    pub surface: Option<Surface>,
    /// this configuration needs to be updated with the latest resize
    pub surface_config: SurfaceConfiguration,
    /// Surface manager will iterate over this and find the first format that is supported by surface.
    /// if we find one, we will set surface configuration to that format.
    /// if we don't find one, we will just use the first surface format support.
    /// so, if you don't care about the surface format, just set this to an empty vector.
    surface_formats_priority: Vec<TextureFormat>,
}
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
impl Drop for SurfaceManager {
    fn drop(&mut self) {
        tracing::warn!("dropping wgpu surface");
    }
}
impl SurfaceManager {
    pub fn new(
        window_backend: &mut impl WindowBackend,
        instance: &Instance,
        adapter: &Adapter,
        device: &Device,
        surface: Option<Surface>,
        surface_formats_priority: Vec<TextureFormat>,
        surface_config: SurfaceConfiguration,
    ) -> Self {
        let mut surface_manager = Self {
            surface_view: None,
            surface_current_image: None,
            surface,
            surface_config,
            surface_formats_priority,
        };
        surface_manager.reconfigure_surface(window_backend, instance, adapter, device);
        surface_manager
    }
    pub fn create_current_surface_texture_view(
        &mut self,
        window_backend: &mut impl WindowBackend,
        device: &Device,
    ) {
        if let Some(surface) = self.surface.as_ref() {
            let current_surface_image = surface.get_current_texture().unwrap_or_else(|_| {
                let phy_fb_size = window_backend.get_live_physical_size_framebuffer().unwrap();
                self.surface_config.width = phy_fb_size[0];
                self.surface_config.height = phy_fb_size[1];
                surface.configure(device, &self.surface_config);
                surface.get_current_texture().unwrap_or_else(|e| {
                    panic!("failed to get surface even after reconfiguration. {e}")
                })
            });
            if current_surface_image.suboptimal {
                tracing::warn!("current surface image is suboptimal. ");
            }
            let surface_view = current_surface_image
                .texture
                .create_view(&TextureViewDescriptor {
                    label: Some("surface view"),
                    format: Some(self.surface_config.format),
                    dimension: Some(TextureViewDimension::D2),
                    aspect: TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                });

            self.surface_view = Some(surface_view);
            self.surface_current_image = Some(current_surface_image);
        } else {
            tracing::warn!(
                "skipping acquiring the currnet surface image because there's no surface"
            );
        }
    }
    /// This basically checks if the surface needs creating. and then if needed, creates surface if window exists.
    /// then, it does all the work of configuring the surface.
    /// this is used during resume events to create a surface.
    pub fn reconfigure_surface(
        &mut self,
        window_backend: &mut impl WindowBackend,
        instance: &Instance,
        adapter: &Adapter,
        device: &Device,
    ) {
        if let Some(window) = window_backend.get_window() {
            if self.surface.is_none() {
                self.surface = Some(unsafe {
                    tracing::debug!("creating a surface with {:?}", window.raw_window_handle());
                    instance
                        .create_surface(window)
                        .expect("failed to create surface")
                });
            }

            let capabilities = self.surface.as_ref().unwrap().get_capabilities(adapter);
            let supported_formats = capabilities.formats;
            debug!(
                "supported alpha modes: {:#?}",
                &capabilities.alpha_modes[..]
            );

            if window_backend.get_config().transparent.unwrap_or_default() {
                for alpha_mode in capabilities.alpha_modes.iter().copied() {
                    match alpha_mode {
                        CompositeAlphaMode::PreMultiplied | CompositeAlphaMode::PostMultiplied => {
                            self.surface_config.alpha_mode = alpha_mode;
                        }
                        _ => {}
                    }
                }
            }
            debug!("supported formats of the surface: {supported_formats:#?}");

            let mut compatible_format_found = false;
            for sfmt in self.surface_formats_priority.iter() {
                debug!("checking if {sfmt:?} is supported");
                if supported_formats.contains(sfmt) {
                    debug!("{sfmt:?} is supported. setting it as surface format");
                    self.surface_config.format = *sfmt;
                    compatible_format_found = true;
                    break;
                }
            }
            if !compatible_format_found {
                if !self.surface_formats_priority.is_empty() {
                    tracing::warn!(
                        "could not find compatible surface format from user provided formats. choosing first supported format instead"
                    );
                }
                self.surface_config.format = supported_formats
                    .iter()
                    .find(|f| f.is_srgb())
                    .copied()
                    .unwrap_or_else(|| {
                        supported_formats
                            .first()
                            .copied()
                            .expect("surface has zero supported texture formats")
                    })
            }
            let view_format = if self.surface_config.format.is_srgb() {
                self.surface_config.format
            } else {
                tracing::warn!(
                    "surface format is not srgb: {:?}",
                    self.surface_config.format
                );
                match self.surface_config.format {
                    TextureFormat::Rgba8Unorm => TextureFormat::Rgba8UnormSrgb,
                    TextureFormat::Bgra8Unorm => TextureFormat::Bgra8UnormSrgb,
                    _ => self.surface_config.format,
                }
            };
            self.surface_config.view_formats = vec![view_format];

            #[cfg(target_os = "emscripten")]
            {
                self.surface_config.view_formats = vec![];
            }

            debug!(
                "using format: {:#?} for surface configuration",
                self.surface_config.format
            );
            self.resize_framebuffer(device, window_backend);
        }
    }

    pub fn resize_framebuffer(&mut self, device: &Device, window_backend: &mut impl WindowBackend) {
        if let Some(size) = window_backend.get_live_physical_size_framebuffer() {
            self.surface_config.width = size[0];
            self.surface_config.height = size[1];
            info!(
                "reconfiguring surface with config: {:#?}",
                &self.surface_config
            );
            self.surface
                .as_ref()
                .unwrap()
                .configure(device, &self.surface_config);
        }
    }
    pub fn suspend(&mut self) {
        self.surface = None;
        self.surface_current_image = None;
        self.surface_view = None;
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
        }
    }

    fn render_egui(
        &mut self,
        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
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
        );
        let mut command_encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("egui command encoder"),
            });
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

pub const EGUI_SHADER_SRC: &str = include_str!("../egui.wgsl");

type PrepareCallback = dyn Fn(&Device, &Queue, &mut IdTypeMap) + Sync + Send;
type RenderCallback =
    dyn for<'a, 'b> Fn(PaintCallbackInfo, &'a mut RenderPass<'b>, &'b IdTypeMap) + Sync + Send;

pub struct CallbackFn {
    pub prepare: Arc<PrepareCallback>,
    pub paint: Arc<RenderCallback>,
}

impl Default for CallbackFn {
    fn default() -> Self {
        CallbackFn {
            prepare: Arc::new(|_, _, _| ()),
            paint: Arc::new(|_, _, _| ()),
        }
    }
}

pub struct EguiPainter {
    /// current capacity of vertex buffer
    vb_len: usize,
    /// current capacity of index buffer
    ib_len: usize,
    /// vertex buffer for all egui (clipped) meshes
    vb: Buffer,
    /// index buffer for all egui (clipped) meshes
    ib: Buffer,
    /// Uniform buffer to store screen size in logical points
    screen_size_buffer: Buffer,
    /// bind group for the Uniform buffer using layout entry [`SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY`]
    screen_size_bind_group: BindGroup,
    /// this layout is reused by all egui textures.
    pub texture_bindgroup_layout: BindGroupLayout,
    /// used by pipeline create function
    pub screen_size_bindgroup_layout: BindGroupLayout,
    /// The current pipeline has been created with this format as the output
    /// If we need to render to a different format, then we need to recreate the render pipeline with the relevant format as output
    surface_format: TextureFormat,
    /// egui render pipeline
    pipeline: RenderPipeline,
    /// This is the sampler used for most textures that user uploads
    pub linear_sampler: Sampler,
    /// nearest sampler suitable for font textures (or any pixellated textures)
    pub nearest_sampler: Sampler,
    /// Textures uploaded by egui itself. 
    managed_textures: BTreeMap<u64, EguiTexture>,
    #[allow(unused)]
    user_textures: BTreeMap<u64, EguiTexture>,
    /// textures to free
    delete_textures: Vec<TextureId>,
    custom_data: IdTypeMap,
}

/// textures uploaded by egui are represented by this struct
pub struct EguiTexture {
    pub texture: Texture,
    pub view: TextureView,
    pub bindgroup: BindGroup,
}
/// We take all the
pub enum EguiDrawCalls {
    Mesh {
        clip_rect: [u32; 4],
        texture_id: TextureId,
        base_vertex: i32,
        index_start: u32,
        index_end: u32,
    },
    Callback {
        paint_callback_info: PaintCallbackInfo,
        clip_rect: [u32; 4],
        paint_callback: PaintCallback,
    },
}
impl EguiPainter {
    pub fn draw_egui_with_renderpass<'rpass>(
        &'rpass mut self,
        rpass: &mut RenderPass<'rpass>,
        draw_calls: Vec<EguiDrawCalls>,
    ) {
        // rpass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.screen_size_bind_group, &[]);

        rpass.set_vertex_buffer(0, self.vb.slice(..));
        rpass.set_index_buffer(self.ib.slice(..), IndexFormat::Uint32);
        for draw_call in draw_calls {
            match draw_call {
                EguiDrawCalls::Mesh {
                    clip_rect,
                    texture_id,
                    base_vertex,
                    index_start,
                    index_end,
                } => {
                    let [x, y, width, height] = clip_rect;
                    rpass.set_scissor_rect(x, y, width, height);
                    // In webgl, base vertex is not supported in the draw_indexed function (draw elements in webgl2).
                    // so, we instead bind the buffer with different offsets every call so that indices will point to their respective vertices.
                    // this is possible because webgl2 has bindBufferRange (which allows specifying a offset as the start of the buffer binding)
                    rpass.set_vertex_buffer(0, self.vb.slice(base_vertex as u64 * 20..));
                    match texture_id {
                        TextureId::Managed(key) => {
                            rpass.set_bind_group(
                                1,
                                &self
                                    .managed_textures
                                    .get(&key)
                                    .expect("cannot find managed texture")
                                    .bindgroup,
                                &[],
                            );
                        }
                        TextureId::User(_) => unimplemented!(),
                    }
                    rpass.draw_indexed(index_start..index_end, 0, 0..1);
                }
                EguiDrawCalls::Callback {
                    clip_rect,
                    paint_callback,
                    paint_callback_info,
                } => {
                    let [x, y, width, height] = clip_rect;
                    rpass.set_scissor_rect(x, y, width, height);
                    (paint_callback
                        .callback
                        .downcast_ref::<CallbackFn>()
                        .expect("failed to downcast Callbackfn")
                        .paint)(
                        PaintCallbackInfo {
                            viewport: paint_callback_info.viewport,
                            clip_rect: paint_callback_info.clip_rect,
                            pixels_per_point: paint_callback_info.pixels_per_point,
                            screen_size_px: paint_callback_info.screen_size_px,
                        },
                        rpass,
                        &self.custom_data,
                    );
                }
            }
        }
    }
    pub fn create_render_pipeline(
        dev: &Device,
        pipeline_surface_format: TextureFormat,
        screen_size_bindgroup_layout: &BindGroupLayout,
        texture_bindgroup_layout: &BindGroupLayout,
    ) -> RenderPipeline {
        // let srgb = pipeline_surface_format.is_srgb();

        // pipeline layout. screensize uniform buffer for vertex shader + texture and sampler for fragment shader
        let egui_pipeline_layout = dev.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("egui pipeline layout"),
            bind_group_layouts: &[screen_size_bindgroup_layout, texture_bindgroup_layout],
            push_constant_ranges: &[],
        });
        // shader from the wgsl source.
        let shader_module = dev.create_shader_module(ShaderModuleDescriptor {
            label: Some("egui shader src"),
            source: ShaderSource::Wgsl(EGUI_SHADER_SRC.into()),
        });
        // create pipeline using shaders + pipeline layout
        dev.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("egui pipeline"),
            layout: Some(&egui_pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &VERTEX_BUFFER_LAYOUT,
            },
            primitive: EGUI_PIPELINE_PRIMITIVE_STATE,
            depth_stencil: None,
            // support multi sampling in future?
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: if pipeline_surface_format.is_srgb() {
                    "fs_main_linear_output"
                } else {
                    "fs_main_srgb_output"
                },
                targets: &[Some(ColorTargetState {
                    format: pipeline_surface_format,
                    blend: Some(EGUI_PIPELINE_BLEND_STATE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
        })
    }
    pub fn new(dev: &Device, surface_format: TextureFormat) -> Self {
        // create uniform buffer for screen size
        let screen_size_buffer = dev.create_buffer(&BufferDescriptor {
            label: Some("screen size uniform buffer"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // create temporary layout to create screensize uniform buffer bindgroup
        let screen_size_bindgroup_layout =
            dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("egui screen size bindgroup layout"),
                entries: &SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY,
            });
        // create texture bindgroup layout. all egui textures need to have a bindgroup with this layout to use
        // them in egui draw calls.
        let texture_bindgroup_layout = dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("egui texture bind group layout"),
            entries: &TEXTURE_BINDGROUP_ENTRIES,
        });
        // create screen size bind group with the above layout. store this permanently to bind before drawing egui.
        let screen_size_bind_group = dev.create_bind_group(&BindGroupDescriptor {
            label: Some("egui bindgroup"),
            layout: &screen_size_bindgroup_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &screen_size_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let pipeline = Self::create_render_pipeline(
            dev,
            surface_format,
            &screen_size_bindgroup_layout,
            &texture_bindgroup_layout,
        );

        // linear and nearest samplers for egui textures to use for creation of their bindgroups
        let linear_sampler = dev.create_sampler(&SamplerDescriptor {
            label: Some("linear sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let nearest_sampler = dev.create_sampler(&SamplerDescriptor {
            label: Some("nearest sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // empty vertex and index buffers.
        let vb = dev.create_buffer(&BufferDescriptor {
            label: Some("egui vertex buffer"),
            size: 0,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ib = dev.create_buffer(&BufferDescriptor {
            label: Some("egui index buffer"),
            size: 0,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            screen_size_buffer,
            pipeline,
            linear_sampler,
            nearest_sampler,
            managed_textures: Default::default(),
            vb,
            ib,
            screen_size_bind_group,
            texture_bindgroup_layout,
            vb_len: 0,
            ib_len: 0,
            delete_textures: Vec::new(),
            custom_data: IdTypeMap::default(),
            user_textures: Default::default(),
            screen_size_bindgroup_layout,
            surface_format,
        }
    }
    fn on_resume(&mut self, dev: &Device, surface_format: TextureFormat) {
        if self.surface_format != surface_format {
            self.pipeline = Self::create_render_pipeline(
                dev,
                surface_format,
                &self.screen_size_bindgroup_layout,
                &self.texture_bindgroup_layout,
            );
        }
    }
    fn set_textures(
        &mut self,
        dev: &Device,
        queue: &Queue,
        textures_delta_set: Vec<(TextureId, ImageDelta)>,
    ) {
        for (tex_id, delta) in textures_delta_set {
            let width = delta.image.width() as u32;
            let height = delta.image.height() as u32;

            let size = Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            let data_color32 = match delta.image {
                egui::ImageData::Color(color_image) => color_image.pixels,
                egui::ImageData::Font(font_image) => {
                    font_image.srgba_pixels(None).collect::<Vec<_>>()
                }
            };
            let data_bytes: &[u8] = bytemuck::cast_slice(data_color32.as_slice());
            match tex_id {
                egui::TextureId::Managed(tex_id) => {
                    if let Some(delta_pos) = delta.pos {
                        // we only update part of the texture, if the tex id refers to a live texture
                        if let Some(tex) = self.managed_textures.get(&tex_id) {
                            queue.write_texture(
                                ImageCopyTexture {
                                    texture: &tex.texture,
                                    mip_level: 0,
                                    origin: Origin3d {
                                        x: delta_pos[0].try_into().unwrap(),
                                        y: delta_pos[1].try_into().unwrap(),
                                        z: 0,
                                    },
                                    aspect: TextureAspect::All,
                                },
                                data_bytes,
                                ImageDataLayout {
                                    offset: 0,
                                    bytes_per_row: Some(size.width * 4),
                                    // only required in 3d textures or 2d array textures
                                    rows_per_image: None,
                                },
                                size,
                            );
                        }
                    } else {
                        let mip_level_count = 1;
                        let new_texture = dev.create_texture(&TextureDescriptor {
                            label: None,
                            size,
                            mip_level_count,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[TextureFormat::Rgba8UnormSrgb],
                        });

                        queue.write_texture(
                            ImageCopyTexture {
                                texture: &new_texture,
                                mip_level: 0,
                                origin: Origin3d::default(),
                                aspect: TextureAspect::All,
                            },
                            data_bytes,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(size.width * 4),
                                rows_per_image: None,
                            },
                            size,
                        );
                        let view = new_texture.create_view(&TextureViewDescriptor {
                            label: None,
                            format: Some(TextureFormat::Rgba8UnormSrgb),
                            dimension: Some(TextureViewDimension::D2),
                            aspect: TextureAspect::All,
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        });
                        assert!(delta.options.magnification == delta.options.minification);
                        let bindgroup = dev.create_bind_group(&BindGroupDescriptor {
                            label: None,
                            layout: &self.texture_bindgroup_layout,
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::Sampler(if tex_id == 0 {
                                        &self.nearest_sampler
                                    } else {
                                        match delta.options.magnification {
                                            egui::TextureFilter::Nearest => &self.nearest_sampler,
                                            egui::TextureFilter::Linear => &self.linear_sampler,
                                        }
                                    }),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::TextureView(&view),
                                },
                            ],
                        });
                        self.managed_textures.insert(
                            tex_id,
                            EguiTexture {
                                texture: new_texture,
                                view,
                                bindgroup,
                            },
                        );
                    }
                }
                egui::TextureId::User(_) => todo!(),
            }
        }
    }
    pub fn upload_egui_data(
        &mut self,
        dev: &Device,
        queue: &Queue,
        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        logical_screen_size: [f32; 2],
        physical_framebuffer_size: [u32; 2],
    ) -> Vec<EguiDrawCalls> {
        let scale = physical_framebuffer_size[0] as f32 / logical_screen_size[0];
        // first deal with textures
        {
            // we need to delete textures in textures_delta.free AFTER the draw calls
            // so we store them in self.delete_textures.
            // otoh, the textures that were scheduled to be deleted previous frame, we will delete now

            let delete_textures = std::mem::replace(&mut self.delete_textures, textures_delta.free);
            // remove textures to be deleted in previous frame
            for tid in delete_textures {
                match tid {
                    TextureId::Managed(key) => {
                        self.managed_textures.remove(&key);
                    }
                    TextureId::User(_) => todo!(),
                }
            }
            // upload textures
            self.set_textures(dev, queue, textures_delta.set);
        }
        // update screen size uniform buffer
        queue.write_buffer(
            &self.screen_size_buffer,
            0,
            cast_slice(&logical_screen_size),
        );

        {
            // total vertices and indices lengths
            let (vb_len, ib_len) = meshes.iter().fold((0, 0), |(vb_len, ib_len), mesh| {
                if let egui::epaint::Primitive::Mesh(ref m) = mesh.primitive {
                    (vb_len + m.vertices.len(), ib_len + m.indices.len())
                } else {
                    (vb_len, ib_len)
                }
            });
            if vb_len == 0 || ib_len == 0 {
                return meshes
                    .into_iter()
                    .filter_map(|p| match p.primitive {
                        egui::epaint::Primitive::Mesh(_) => None,
                        egui::epaint::Primitive::Callback(cb) => {
                            (cb.callback
                                .downcast_ref::<CallbackFn>()
                                .expect("failed to downcast egui callback fn")
                                .prepare)(
                                dev, queue, &mut self.custom_data
                            );
                            egui_backend::util::scissor_from_clip_rect(
                                &p.clip_rect,
                                scale,
                                physical_framebuffer_size,
                            )
                            .map(|clip_rect| EguiDrawCalls::Callback {
                                clip_rect,
                                paint_callback: cb,
                                paint_callback_info: PaintCallbackInfo {
                                    viewport: Rect::from_min_size(
                                        Default::default(),
                                        logical_screen_size.into(),
                                    ),
                                    clip_rect: p.clip_rect,
                                    pixels_per_point: scale,
                                    screen_size_px: physical_framebuffer_size,
                                },
                            })
                        }
                    })
                    .collect();
            }

            // resize if vertex or index buffer capcities are not enough
            if self.vb_len < vb_len {
                self.vb = dev.create_buffer(&BufferDescriptor {
                    label: Some("egui vertex buffer"),
                    size: vb_len as u64 * 20,
                    usage: BufferUsages::COPY_DST | BufferUsages::VERTEX,
                    mapped_at_creation: false,
                });
                self.vb_len = vb_len;
            }
            if self.ib_len < ib_len {
                self.ib = dev.create_buffer(&BufferDescriptor {
                    label: Some("egui index buffer"),
                    size: ib_len as u64 * 4,
                    usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
                    mapped_at_creation: false,
                });
                self.ib_len = ib_len;
            }
            // create mutable slices for vertex and index buffers
            let mut vertex_buffer_mut = queue
                .write_buffer_with(
                    &self.vb,
                    0,
                    NonZeroU64::new(
                        (self.vb_len * 20)
                            .try_into()
                            .expect("unreachable as usize is u64"),
                    )
                    .expect("vertex buffer length should not be zero"),
                )
                .expect("failed to create queuewritebufferview");
            let mut index_buffer_mut = queue
                .write_buffer_with(
                    &self.ib,
                    0,
                    NonZeroU64::new(
                        (self.ib_len * 4)
                            .try_into()
                            .expect("unreachable as usize is u64"),
                    )
                    .expect("index buffer length should not be zero"),
                )
                .expect("failed to create queuewritebufferview");
            // offsets from where to start writing vertex or index buffer data
            let mut vb_offset = 0;
            let mut ib_offset = 0;
            let mut draw_calls = vec![];
            for clipped_primitive in meshes {
                let ClippedPrimitive {
                    clip_rect,
                    primitive,
                } = clipped_primitive;
                let primitive_clip_rect = clip_rect;
                let clip_rect = if let Some(c) = egui_backend::util::scissor_from_clip_rect(
                    &primitive_clip_rect,
                    scale,
                    physical_framebuffer_size,
                ) {
                    c
                } else {
                    continue;
                };

                match primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        let Mesh {
                            indices,
                            vertices,
                            texture_id,
                        } = mesh;

                        // offset upto where we want to write the vertices or indices.
                        let new_vb_offset = vb_offset + vertices.len() * 20; // multiply by vertex size as slice is &[u8]
                        let new_ib_offset = ib_offset + indices.len() * 4; // multiply by index size as slice is &[u8]
                                                                           // write from start offset to end offset
                        vertex_buffer_mut[vb_offset..new_vb_offset]
                            .copy_from_slice(cast_slice(&vertices));
                        index_buffer_mut[ib_offset..new_ib_offset]
                            .copy_from_slice(cast_slice(&indices));
                        // record draw call
                        draw_calls.push(EguiDrawCalls::Mesh {
                            clip_rect,
                            texture_id,
                            // vertex buffer offset is in bytes. so, we divide by size to get the "nth" vertex to use as base
                            base_vertex: (vb_offset / 20)
                                .try_into()
                                .expect("failed to fit vertex buffer offset into i32"),
                            // ib offset is in bytes. divided by index size, we get the starting and ending index to use for this draw call
                            index_start: (ib_offset / 4) as u32,
                            index_end: (new_ib_offset / 4) as u32,
                        });
                        // set end offsets as start offsets for next iteration
                        vb_offset = new_vb_offset;
                        ib_offset = new_ib_offset;
                    }
                    egui::epaint::Primitive::Callback(cb) => {
                        (cb.callback
                            .downcast_ref::<CallbackFn>()
                            .expect("failed to downcast egui callback fn")
                            .prepare)(dev, queue, &mut self.custom_data);
                        draw_calls.push(EguiDrawCalls::Callback {
                            clip_rect,
                            paint_callback: cb,
                            paint_callback_info: PaintCallbackInfo {
                                viewport: Rect::from_min_size(
                                    Default::default(),
                                    logical_screen_size.into(),
                                ),
                                clip_rect: primitive_clip_rect,
                                pixels_per_point: scale,
                                screen_size_px: physical_framebuffer_size,
                            },
                        });
                    }
                }
            }
            draw_calls
        }
    }
}

pub const SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY: [BindGroupLayoutEntry; 1] =
    [BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::VERTEX,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(16),
        },
        count: None,
    }];

pub const TEXTURE_BINDGROUP_ENTRIES: [BindGroupLayoutEntry; 2] = [
    BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    },
    BindGroupLayoutEntry {
        binding: 1,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    },
];
pub const VERTEX_BUFFER_LAYOUT: [VertexBufferLayout; 1] = [VertexBufferLayout {
    // vertex size
    array_stride: 20,
    step_mode: VertexStepMode::Vertex,
    attributes: &[
        // position x, y
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        // texture coordinates x, y
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 8,
            shader_location: 1,
        },
        // color as rgba (unsigned bytes which will be turned into floats inside shader)
        VertexAttribute {
            format: VertexFormat::Unorm8x4,
            offset: 16,
            shader_location: 2,
        },
    ],
}];

pub const EGUI_PIPELINE_PRIMITIVE_STATE: PrimitiveState = PrimitiveState {
    topology: PrimitiveTopology::TriangleList,
    strip_index_format: None,
    front_face: FrontFace::Ccw,
    cull_mode: None,
    unclipped_depth: false,
    polygon_mode: PolygonMode::Fill,
    conservative: false,
};

pub const EGUI_PIPELINE_BLEND_STATE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::OneMinusDstAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
};

// `Default::default` is not const. so, we have to manually fill the default values
