use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroU64},
};

use bytemuck::cast_slice;
use egui_backend::{
    egui::{epaint::ImageDelta, ClippedPrimitive, Mesh, TextureId},
    EguiGfxOutput,
};
use intmap::IntMap;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferDescriptor,
    BufferUsages, CommandEncoder, Device, Extent3d, FilterMode, FragmentState, FrontFace,
    ImageCopyTexture, ImageDataLayout, IndexFormat, MultisampleState, Operations, Origin3d,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderStages, Texture,
    TextureAspect, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};

const EGUI_SHADER_SRC: &str = include_str!("egui.wgsl");
pub struct EguiPainter {
    screen_size_buffer: Buffer,
    screen_size_bind_group: BindGroup,
    pipeline: RenderPipeline,
    pub linear_sampler: Sampler,
    nearest_sampler: Sampler,
    texture_bindgroup_layout: BindGroupLayout,
    managed_textures: IntMap<(Texture, TextureView, BindGroup)>,
    vb_len: usize,
    ib_len: usize,
    vb: Buffer,
    ib: Buffer,
}

impl EguiPainter {
    pub fn new(dev: &Device, surface_format: TextureFormat) -> Self {
        let shader_module = dev.create_shader_module(ShaderModuleDescriptor {
            label: Some("egui shader module"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(EGUI_SHADER_SRC)),
        });
        let screen_size_buffer = dev.create_buffer(&BufferDescriptor {
            label: Some("screen size uniform buffer"),
            size: 8,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let screen_size_bind_group_layout =
            dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("egui screen size bindgroup layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(8).expect("impossible")),
                    },
                    count: None,
                }],
            });
        let texture_bindgroup_layout = dev.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("egui texture bind group layout"),
            entries: &[
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
            ],
        });
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
        let egui_pipeline_layout = dev.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("egui pipeline layout"),
            bind_group_layouts: &[&screen_size_bind_group_layout, &texture_bindgroup_layout],
            push_constant_ranges: &[],
        });
        let egui_pipeline = dev.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("egui pipeline"),
            layout: Some(&egui_pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: 20,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
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
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });
        let linear_sampler = dev.create_sampler(&SamplerDescriptor {
            label: Some("linear sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });
        let nearest_sampler = dev.create_sampler(&SamplerDescriptor {
            label: Some("nearest sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        Self {
            screen_size_buffer,
            pipeline: egui_pipeline,
            linear_sampler,
            nearest_sampler,
            managed_textures: IntMap::new(),
            vb: dev.create_buffer(&BufferDescriptor {
                label: Some("egui vertex buffer"),
                size: 0,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ib: dev.create_buffer(&BufferDescriptor {
                label: Some("egui index buffer"),
                size: 0,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            screen_size_bind_group,
            texture_bindgroup_layout,
            vb_len: 0,
            ib_len: 0,
        }
    }

    pub fn draw_egui(
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
        command_encoder: &mut CommandEncoder,
        frame_buffer: &TextureView,
        _width: u32,
        _height: u32,
    ) {
        self.set_textures(dev, queue, textures_delta.set);
        {
            let mut rpass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: frame_buffer,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            // rpass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
            rpass.set_pipeline(&self.pipeline);
            queue.write_buffer(
                &self.screen_size_buffer,
                0,
                cast_slice(&screen_size_logical),
            );
            rpass.set_bind_group(0, &self.screen_size_bind_group, &[]);

            let mut vb_len = 0;
            let mut ib_len = 0;
            for cp in &meshes {
                match cp.primitive {
                    egui_backend::egui::epaint::Primitive::Mesh(ref m) => {
                        ib_len += m.indices.len();
                        vb_len += m.vertices.len();
                    }
                    egui_backend::egui::epaint::Primitive::Callback(_) => todo!(),
                }
            }
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
            rpass.set_vertex_buffer(0, self.vb.slice(..));
            rpass.set_index_buffer(self.ib.slice(..), IndexFormat::Uint32);
            let mut vb_offset = 0;
            let mut ib_offset = 0;
            for clipped_primitive in meshes {
                let ClippedPrimitive {
                    clip_rect,
                    primitive,
                } = clipped_primitive;
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

                {
                    // Clip scissor rectangle to target size.
                    let x = clip_min_x.min(screen_size_physical[0]);
                    let y = clip_min_y.min(screen_size_physical[1]);
                    let width = width.min(screen_size_physical[0] - x);
                    let height = height.min(screen_size_physical[1] - y);

                    // Skip rendering with zero-sized clip areas.
                    if width == 0 || height == 0 {
                        continue;
                    }

                    rpass.set_scissor_rect(x, y, width, height);
                }
                match primitive {
                    egui_backend::egui::epaint::Primitive::Mesh(mesh) => {
                        let Mesh {
                            indices,
                            vertices,
                            texture_id,
                        } = mesh;
                        match texture_id {
                            TextureId::Managed(key) => {
                                rpass.set_bind_group(
                                    1,
                                    &self
                                        .managed_textures
                                        .get(key)
                                        .expect("cannot find managed texture")
                                        .2,
                                    &[],
                                );
                            }
                            TextureId::User(_) => todo!(),
                        }
                        queue.write_buffer(&self.vb, vb_offset * 20, cast_slice(&vertices));
                        queue.write_buffer(&self.ib, ib_offset * 4, cast_slice(&indices));
                        rpass.draw_indexed(
                            ib_offset as u32..(indices.len() as u32 + ib_offset as u32),
                            vb_offset as i32,
                            0..1,
                        );
                        vb_offset += vertices.len() as u64;
                        ib_offset += indices.len() as u64;
                    }
                    egui_backend::egui::epaint::Primitive::Callback(_) => todo!(),
                }
            }
        }
        for tid in textures_delta.free {
            match tid {
                TextureId::Managed(key) => {
                    self.managed_textures.remove(key);
                }
                TextureId::User(_) => todo!(),
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
                                    resource: BindingResource::Sampler(match delta.filter {
                                        egui_backend::egui::TextureFilter::Nearest => {
                                            &self.nearest_sampler
                                        }
                                        egui_backend::egui::TextureFilter::Linear => {
                                            &self.linear_sampler
                                        }
                                    }),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::TextureView(&view),
                                },
                            ],
                        });
                        self.managed_textures
                            .insert(tex_id, (new_texture, view, bindgroup));
                    }
                }
                egui_backend::egui::TextureId::User(_) => todo!(),
            }
        }
    }
}
