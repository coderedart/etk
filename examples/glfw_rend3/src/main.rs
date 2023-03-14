use egui::Window;
use egui_backend::{egui, BackendConfig, GfxBackend, UserApp, WindowBackend};
use egui_render_rend3::*;
use egui_window_glfw_passthrough::GlfwBackend;
use rend3::types::DirectionalLightHandle;
use rend3_gltf::*;
use rend3_routine::{base::BaseRenderGraph, pbr::PbrRoutine, tonemapping::TonemappingRoutine};
struct App {
    frame_count: usize,
    bg_color: egui::Color32,
    _gltf_scene: (LoadedGltfScene, GltfSceneInstance),
    pbr_routine: PbrRoutine,
    tone_routine: TonemappingRoutine,
    base_render_graph: BaseRenderGraph,
    _directional_light_handle: DirectionalLightHandle,
    egui_context: egui::Context,
    rend3_backend: Rend3Backend,
    glfw_backend: GlfwBackend,
}
impl App {
    pub fn new(rend3_backend: Rend3Backend, glfw_backend: GlfwBackend) -> Self {
        let box_gltf = std::fs::read("./data.glb").unwrap();
        let renderer = rend3_backend.renderer.clone();
        let gltf_scene = pollster::block_on(load_gltf(
            &renderer,
            &box_gltf,
            &Default::default(),
            |io| async move { std::fs::read(io.as_str()) },
        ))
        .unwrap();

        // Create the shader preprocessor with all the default shaders added.
        let mut spp = rend3::ShaderPreProcessor::new();
        rend3_routine::builtin_shaders(&mut spp);

        // Create the base rendergraph.
        let base_rendergraph = rend3_routine::base::BaseRenderGraph::new(&renderer, &spp);

        let mut data_core = renderer.data_core.lock();
        let pbr_routine = rend3_routine::pbr::PbrRoutine::new(
            &renderer,
            &mut data_core,
            &spp,
            &base_rendergraph.interfaces,
        );
        drop(data_core);
        let tonemapping_routine = rend3_routine::tonemapping::TonemappingRoutine::new(
            &renderer,
            &spp,
            &base_rendergraph.interfaces,
            rend3_backend.surface_manager.surface_config.format,
        );

        let view_location = glam::Vec3::new(3.0, 3.0, -5.0);
        let view = glam::Mat4::from_euler(glam::EulerRot::XYZ, -0.55, 0.5, 0.0);
        let view = view * glam::Mat4::from_translation(-view_location);

        // Set camera's location
        renderer.set_camera_data(rend3::types::Camera {
            projection: rend3::types::CameraProjection::Perspective {
                vfov: 60.0,
                near: 0.1,
            },
            view,
        });
        let directional_light_handle =
            renderer.add_directional_light(rend3::types::DirectionalLight {
                color: glam::Vec3::ONE,
                intensity: 4.0,
                // Direction will be normalized
                direction: glam::Vec3::new(-1.0, -4.0, 2.0),
                distance: 20.0,
                resolution: 2048,
            });
        Self {
            frame_count: 0,
            bg_color: egui::Color32::LIGHT_BLUE,
            egui_context: Default::default(),
            rend3_backend,
            glfw_backend,
            _gltf_scene: gltf_scene,
            pbr_routine,
            tone_routine: tonemapping_routine,
            base_render_graph: base_rendergraph,
            _directional_light_handle: directional_light_handle,
        }
    }
}
impl UserApp for App {
    type UserGfxBackend = Rend3Backend;

    type UserWindowBackend = GlfwBackend;

    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    ) {
        (
            &mut self.glfw_backend,
            &mut self.rend3_backend,
            &self.egui_context,
        )
    }

    fn gui_run(&mut self) {
        let renderer = self.rend3_backend.renderer.clone();

        // Swap the instruction buffers so that our frame's changes can be processed.
        renderer.swap_instruction_buffers();
        // Evaluate our frame's world-change instructions
        let mut eval_output = renderer.evaluate_instructions();

        // Build a rendergraph
        let mut graph = rend3::graph::RenderGraph::new();

        // Import the surface texture into the render graph.
        let frame_handle = graph.add_imported_render_target(
            self.rend3_backend
                .surface_manager
                .surface_current_image
                .as_ref()
                .unwrap(),
            0..1,
            rend3::graph::ViewportRect::from_size(
                [
                    self.rend3_backend.surface_manager.surface_config.width,
                    self.rend3_backend.surface_manager.surface_config.height,
                ]
                .into(),
            ),
        );
        let rgba = self.bg_color.to_array();
        let rgba = rgba.map(|component| component as f32 / 255.0);
        // Add the default rendergraph without a skybox
        self.base_render_graph.add_to_graph(
            &mut graph,
            &eval_output,
            &self.pbr_routine,
            None,
            &self.tone_routine,
            frame_handle,
            [
                self.rend3_backend.surface_manager.surface_config.width,
                self.rend3_backend.surface_manager.surface_config.height,
            ]
            .into(),
            rend3::types::SampleCount::One,
            glam::Vec4::ZERO,
            rgba.into(),
        );

        // Dispatch a render using the built up rendergraph!
        graph.execute(&renderer, &mut eval_output);

        let egui_context = self.egui_context.clone();
        let egui_context = &egui_context;
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
            ui.color_edit_button_srgba(&mut self.bg_color);
        });
        egui_context.request_repaint();
    }
}

pub fn fake_main() {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = egui_window_glfw_passthrough::GlfwConfig {
        ..Default::default()
    };
    let mut window_backend = GlfwBackend::new(
        config,
        BackendConfig {
            is_opengl: false,
            ..Default::default()
        },
    );
    let rend3_backend = Rend3Backend::new(&mut window_backend, Default::default());
    let app = App::new(rend3_backend, window_backend);
    <App as UserApp>::UserWindowBackend::run_event_loop(app);
}

fn main() {
    fake_main()
}
