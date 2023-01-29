/*
DON'T WORK UNTIL REND3 RELEASES 0.4. the git repo is full of patches and other weird incompatibilities.
*/

// use egui_backend::{egui, GfxBackend, WindowBackend};
// use egui_render_wgpu::{WgpuBackend, WgpuConfig};
// use rend3::{
//     graph::{
//         NodeResourceUsage, RenderGraph, RenderPassTarget, RenderPassTargets, RenderTargetHandle,
//     },
//     types::Color,
// };

// pub struct Rend3Backend {
//     pub wgpu_backend: WgpuBackend,
// }

// pub struct Rend3Config {}
// impl Default for Rend3Config {
//     fn default() -> Self {
//         Self {}
//     }
// }

// impl Rend3Backend {
//     // pub fn add_to_graph<'node>(
//     //     &'node mut self,
//     //     graph: &mut RenderGraph<'node>,
//     //     mut input: Input<'node>,
//     //     output: RenderTargetHandle,
//     // ) {
//     //     let mut builder = graph.add_node("egui");

//     //     let output_handle = builder.add_render_target(output, NodeResourceUsage::InputOutput);

//     //     let rpass_handle = builder.add_renderpass(RenderPassTargets {
//     //         targets: vec![RenderPassTarget {
//     //             color: output_handle,
//     //             clear: Color::TRANSPARENT,
//     //             resolve: None,
//     //         }],
//     //         depth_stencil: None,
//     //     });

//     //     builder.build(move |mut ctx| {
//     //         let rpass = ctx.encoder_or_pass.take_rpass(rpass_handle);

//     //         let mut cmd_buffer = ctx
//     //             .renderer
//     //             .device
//     //             .create_command_encoder(&Default::default());

//     //         drop(cmd_buffer);

//     //         render(rpass, input.clipped_meshes, &self.screen_descriptor);
//     //     });
//     // }
// }
// impl GfxBackend for Rend3Backend {
//     type Configuration = Rend3Config;

//     fn new(window_backend: &mut impl WindowBackend, config: Self::Configuration) -> Self {
//         let rend = pollster::block_on(rend3::create_iad(
//             None,
//             None,
//             Default::default(),
//             Default::default(),
//         ))
//         .expect("failed to create iad in rend3");
//         if let Some(w) = window_backend.get_window() {}
//         Self {
//             wgpu_backend: WgpuBackend::new(window_backend, Default::default()),
//         }
//     }

//     fn prepare_frame(&mut self, window_backend: &mut impl WindowBackend) {
//         self.wgpu_backend.prepare_frame(window_backend);
//     }

//     fn render_egui(
//         &mut self,
//         meshes: Vec<egui::ClippedPrimitive>,
//         textures_delta: egui::TexturesDelta,
//         logical_screen_size: [f32; 2],
//     ) {
//         <WgpuBackend as GfxBackend>::render_egui(
//             &mut self.wgpu_backend,
//             meshes,
//             textures_delta,
//             logical_screen_size,
//         );
//     }

//     fn present(&mut self, window_backend: &mut impl WindowBackend) {
//         <WgpuBackend as GfxBackend>::present(&mut self.wgpu_backend, window_backend)
//     }

//     fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend) {
//         self.wgpu_backend.resize_framebuffer(window_backend);
//     }
// }
