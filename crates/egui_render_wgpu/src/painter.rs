use std::{
    borrow::Cow,
    convert::TryInto,
    num::{NonZeroU32, NonZeroU64},
};

use bytemuck::cast_slice;
use egui_backend::{
    egui::{epaint::ImageDelta, ClippedPrimitive, Mesh, PaintCallback, TextureId},
    EguiGfxOutput,
};
use intmap::IntMap;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferDescriptor, BufferUsages, Device, Extent3d, FilterMode,
    FragmentState, FrontFace, ImageCopyTexture, ImageDataLayout, IndexFormat, MultisampleState,
    Origin3d, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderStages, Texture, TextureAspect,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
    VertexStepMode,
};

const EGUI_SHADER_SRC: &str = include_str!("egui.wgsl");
pub struct EguiPainter {
    /// current capacity of vertex buffer
    pub vb_len: usize,
    /// current capacity of index buffer
    pub ib_len: usize,
    /// vertex buffer
    pub vb: Buffer,
    /// index buffer
    pub ib: Buffer,
    /// Uniform buffer to store screen size in logical pixels
    pub screen_size_buffer: Buffer,
    /// bind group for the Uniform buffer using layout entry `SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY`
    pub screen_size_bind_group: BindGroup,
    /// egui render pipeline
    pub pipeline: RenderPipeline,
    /// linear sampler for egui textures that need to create bindgroups
    pub linear_sampler: Sampler,
    /// nearest sampler for egui textures (especially font texture) that need to create bindgroups for binding to egui pipelien
    pub nearest_sampler: Sampler,
    /// this layout is reused by all egui textures.
    pub texture_bindgroup_layout: BindGroupLayout,
    /// these are textures uploaded by egui. intmap is much faster than btree or hashmaps.
    /// maybe we can use a proper struct instead of tuple?
    pub managed_textures: IntMap<EguiTexture>,
    /// textures to free
    pub delete_textures: Vec<TextureId>,
    pub draw_calls: Vec<EguiDrawCalls>,
}
/// textures uploaded by egui are represented by this struct
pub struct EguiTexture {
    pub texture: Texture,
    pub view: TextureView,
    pub bindgroup: BindGroup,
}
/// DrawCalls list so that we can just get all the work done in the pre_render stage (upload egui data)
pub enum EguiDrawCalls {
    Mesh {
        clip_rect: [u32; 4],
        texture_id: TextureId,
        base_vertex: i32,
        index_start: u32,
        index_end: u32,
    },
    Callback {
        clip_rect: [u32; 4],
        paint_callback: PaintCallback,
    },
}
impl EguiPainter {
    pub fn new(dev: &Device, surface_format: TextureFormat) -> Self {
        // create uniform buffer for screen size
        let screen_size_buffer = dev.create_buffer(&BufferDescriptor {
            label: Some("screen size uniform buffer"),
            size: 8,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // create temporary layout to create screensize uniform buffer bindgroup
        let screen_size_bind_group_layout =
            dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("egui screen size bindgroup layout"),
                entries: &SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY,
            });
        // create screen size bind group with the above layout. store this permanently to bind before drawing egui.
        let screen_size_bind_group = dev.create_bind_group(&BindGroupDescriptor {
            label: Some("egui bindgroup"),
            layout: &screen_size_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &screen_size_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        // create texture bindgroup layout. all egui textures need to have a bindgroup with this layout to use
        // them in egui draw calls.
        let texture_bindgroup_layout = dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("egui texture bind group layout"),
            entries: &TEXTURE_BINDGROUP_ENTRIES,
        });
        // pipeline layout. screensize uniform buffer for vertex shader + texture and sampler for fragment shader
        let egui_pipeline_layout = dev.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("egui pipeline layout"),
            bind_group_layouts: &[&screen_size_bind_group_layout, &texture_bindgroup_layout],
            push_constant_ranges: &[],
        });
        // shader from the wgsl source.
        let shader_module = dev.create_shader_module(ShaderModuleDescriptor {
            label: Some("egui shader module"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(EGUI_SHADER_SRC)),
        });
        // create pipeline using shaders + pipeline layout
        let egui_pipeline = dev.create_render_pipeline(&RenderPipelineDescriptor {
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
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(EGUI_PIPELINE_BLEND_STATE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // linear and nearest samplers for egui textures to use for creation of their bindgroups
        let linear_sampler = dev.create_sampler(&EGUI_LINEAR_SAMPLER_DESCRIPTOR);
        let nearest_sampler = dev.create_sampler(&EGUI_NEAREST_SAMPLER_DESCRIPTOR);

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
            pipeline: egui_pipeline,
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
            draw_calls: Vec::new(),
        }
    }
    pub fn upload_egui_data(
        &mut self,
        dev: &Device,
        queue: &Queue,
        EguiGfxOutput {
            meshes,
            textures_delta,
            screen_size_logical,
            framebuffer_size_physical: screen_size_physical,
            scale,
        }: EguiGfxOutput,
    ) {
        self.draw_calls.clear();
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
                        self.managed_textures.remove(key);
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
            cast_slice(&screen_size_logical),
        );

        {
            // total vertices and indices lengths
            let (vb_len, ib_len) = meshes.iter().fold((0, 0), |(vb_len, ib_len), mesh| {
                if let egui_backend::egui::epaint::Primitive::Mesh(ref m) = mesh.primitive {
                    (vb_len + m.vertices.len(), ib_len + m.indices.len())
                } else {
                    (vb_len, ib_len)
                }
            });
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
            let mut vertex_buffer_mut = queue.write_buffer_with(
                &self.vb,
                0,
                NonZeroU64::new(
                    (self.vb_len * 20)
                        .try_into()
                        .expect("unreachable as usize is u64"),
                )
                .expect("vertex buffer length should not be zero"),
            );
            let mut index_buffer_mut = queue.write_buffer_with(
                &self.vb,
                0,
                NonZeroU64::new(
                    (self.ib_len * 4)
                        .try_into()
                        .expect("unreachable as usize is u64"),
                )
                .expect("index buffer length should not be zero"),
            );
            // offsets from where to start writing vertex or index buffer data
            let mut vb_offset = 0;
            let mut ib_offset = 0;
            for clipped_primitive in meshes {
                let ClippedPrimitive {
                    clip_rect,
                    primitive,
                } = clipped_primitive;
                // create proper clip rectangle
                let clip_min_x = scale * clip_rect.min.x;
                let clip_min_y = scale * clip_rect.min.y;
                let clip_max_x = scale * clip_rect.max.x;
                let clip_max_y = scale * clip_rect.max.y;

                // Make sure clip rect can fit within an `u32`.
                let clip_min_x = clip_min_x.clamp(0.0, screen_size_physical[0] as f32);
                let clip_min_y = clip_min_y.clamp(0.0, screen_size_physical[1] as f32);
                let clip_max_x = clip_max_x.clamp(clip_min_x, screen_size_physical[0] as f32);
                let clip_max_y = clip_max_y.clamp(clip_min_y, screen_size_physical[1] as f32);

                let clip_min_x = clip_min_x.round() as u32;
                let clip_min_y = clip_min_y.round() as u32;
                let clip_max_x = clip_max_x.round() as u32;
                let clip_max_y = clip_max_y.round() as u32;

                let width = (clip_max_x - clip_min_x).max(1);
                let height = (clip_max_y - clip_min_y).max(1);

                // Clip scissor rectangle to target size.
                let clip_x = clip_min_x.min(screen_size_physical[0]);
                let clip_y = clip_min_y.min(screen_size_physical[1]);
                let clip_width = width.min(screen_size_physical[0] - clip_x);
                let clip_height = height.min(screen_size_physical[1] - clip_y);

                // Skip rendering with zero-sized clip areas.
                if clip_width == 0 || clip_height == 0 {
                    continue;
                }
                let clip_rect = [clip_x, clip_y, clip_width, clip_height];
                match primitive {
                    egui_backend::egui::epaint::Primitive::Mesh(mesh) => {
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
                        self.draw_calls.push(EguiDrawCalls::Mesh {
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
                    egui_backend::egui::epaint::Primitive::Callback(cb) => {
                        self.draw_calls.push(EguiDrawCalls::Callback {
                            clip_rect,
                            paint_callback: cb,
                        });
                    }
                }
            }
        }
    }
    pub fn draw_egui<'rpass>(&'rpass mut self, rpass: &mut RenderPass<'rpass>) {
        // rpass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.screen_size_bind_group, &[]);

        rpass.set_vertex_buffer(0, self.vb.slice(..));
        rpass.set_index_buffer(self.ib.slice(..), IndexFormat::Uint32);
        for draw_call in self.draw_calls.iter() {
            match draw_call {
                &EguiDrawCalls::Mesh {
                    clip_rect,
                    texture_id,
                    base_vertex,
                    index_start,
                    index_end,
                } => {
                    let [x, y, width, height] = clip_rect;
                    rpass.set_scissor_rect(x, y, width, height);

                    match texture_id {
                        TextureId::Managed(key) => {
                            rpass.set_bind_group(
                                1,
                                &self
                                    .managed_textures
                                    .get(key)
                                    .expect("cannot find managed texture")
                                    .bindgroup,
                                &[],
                            );
                        }
                        TextureId::User(_) => unimplemented!(),
                    }
                    rpass.draw_indexed(index_start..index_end, base_vertex, 0..1);
                }
                EguiDrawCalls::Callback {
                    clip_rect,
                    paint_callback: _,
                } => {
                    let [x, y, width, height] = *clip_rect;
                    rpass.set_scissor_rect(x, y, width, height);
                    unimplemented!()
                }
            }
        }
    }
    fn set_textures(
        &mut self,
        dev: &Device,
        queue: &Queue,
        textures_delta_set: Vec<(TextureId, ImageDelta)>,
    ) {
        for (tex_id, delta) in textures_delta_set {
            let (pixels, size) = match delta.image {
                egui_backend::egui::ImageData::Color(_) => todo!(),
                egui_backend::egui::ImageData::Font(font_image) => {
                    let pixels: Vec<u8> = font_image
                        .srgba_pixels(1.0)
                        .flat_map(|c| c.to_array())
                        .collect();
                    (pixels, font_image.size)
                }
            };
            match tex_id {
                egui_backend::egui::TextureId::Managed(tex_id) => {
                    if let Some(_) = delta.pos {
                    } else {
                        let mip_level_count = if tex_id == 0 {
                            1
                        } else {
                            panic!("get mip map count formula")
                        };
                        let new_texture = dev.create_texture(&TextureDescriptor {
                            label: None,
                            size: Extent3d {
                                width: size[0] as u32,
                                height: size[1] as u32,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                        });

                        queue.write_texture(
                            ImageCopyTexture {
                                texture: &new_texture,
                                mip_level: 0,
                                origin: Origin3d::default(),
                                aspect: TextureAspect::All,
                            },
                            &pixels,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(
                                    NonZeroU32::new(size[0] as u32 * 4)
                                        .expect("texture bytes per row is zero"),
                                ),
                                rows_per_image: Some(
                                    NonZeroU32::new(size[1] as u32)
                                        .expect("texture rows count is zero"),
                                ),
                            },
                            Extent3d {
                                width: size[0] as u32,
                                height: size[1] as u32,
                                depth_or_array_layers: 1,
                            },
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
                        let bindgroup = dev.create_bind_group(&BindGroupDescriptor {
                            label: None,
                            layout: &self.texture_bindgroup_layout,
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::Sampler(if tex_id == 0 {
                                        &self.nearest_sampler
                                    } else {
                                        match delta.filter {
                                            egui_backend::egui::TextureFilter::Nearest => {
                                                &self.nearest_sampler
                                            }
                                            egui_backend::egui::TextureFilter::Linear => {
                                                &self.linear_sampler
                                            }
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
                egui_backend::egui::TextureId::User(_) => todo!(),
            }
        }
    }
}

pub const SCREEN_SIZE_UNIFORM_BUFFER_BINDGROUP_ENTRY: [BindGroupLayoutEntry; 1] =
    [BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(8),
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
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
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
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

pub const EGUI_LINEAR_SAMPLER_DESCRIPTOR: SamplerDescriptor = SamplerDescriptor {
    label: Some("linear sampler"),
    mag_filter: FilterMode::Linear,
    min_filter: FilterMode::Linear,
    mipmap_filter: FilterMode::Linear,
    // `Default::default` is not const. so, we have to manually fill the default values
    address_mode_u: AddressMode::ClampToEdge,
    address_mode_v: AddressMode::ClampToEdge,
    address_mode_w: AddressMode::ClampToEdge,
    lod_min_clamp: 0.0,
    lod_max_clamp: f32::MAX,
    compare: None,
    anisotropy_clamp: None,
    border_color: None,
};

pub const EGUI_NEAREST_SAMPLER_DESCRIPTOR: SamplerDescriptor = SamplerDescriptor {
    label: Some("nearest sampler"),
    mag_filter: FilterMode::Nearest,
    min_filter: FilterMode::Nearest,
    mipmap_filter: FilterMode::Nearest,
    address_mode_u: AddressMode::ClampToEdge,
    address_mode_v: AddressMode::ClampToEdge,
    address_mode_w: AddressMode::ClampToEdge,
    lod_min_clamp: 0.0,
    lod_max_clamp: f32::MAX,
    compare: None,
    anisotropy_clamp: None,
    border_color: None,
};
