use std::sync::Arc;

use crate::glow_error;
use crate::*;
use glow::*;
#[derive(Debug)]
pub struct PipeLine {
    pub program: NativeProgram,
    pub uniforms: Vec<ProgramUniform>,
    pub attributes: Vec<ProgramAttribute>,
    pub gl: Arc<glow::Context>,
}
impl Drop for PipeLine {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgramUniform {
    pub name: String,
    pub size: i32,
    pub opengl_type: u32,
    pub binding: NativeUniformLocation,
}

#[derive(Debug, Clone)]
pub struct ProgramAttribute {
    pub name: String,
    pub size: i32,
    pub opengl_type: u32,
    pub location: u32,
}

impl PipeLine {
    pub fn new(glow_context: Arc<glow::Context>, vertex_src: &str, fragment_src: &str) -> Self {
        unsafe {
            glow_error!(glow_context);
            // create shaders
            let vs = glow_context
                .create_shader(glow::VERTEX_SHADER)
                .expect("vertex shader creation failed");
            let fs = glow_context
                .create_shader(glow::FRAGMENT_SHADER)
                .expect("failed to create frag shader");
            // set source strings
            glow_context.shader_source(vs, vertex_src);
            glow_context.shader_source(fs, fragment_src);
            // compile and check for errors
            glow_context.compile_shader(vs);
            let info_log = glow_context.get_shader_info_log(vs);
            if !glow_context.get_shader_compile_status(vs) {
                panic!("failed to compile vertex shader. info_log: {}", info_log);
            }
            glow_error!(glow_context);
            glow_context.compile_shader(fs);
            let info_log = glow_context.get_shader_info_log(fs);
            if !glow_context.get_shader_compile_status(fs) {
                panic!("failed to compile fragment shader. info_log: {}", info_log);
            }
            glow_error!(glow_context);

            // create program
            let program = glow_context
                .create_program()
                .expect("failed to create glow program");
            // attach shaders to program
            glow_context.attach_shader(program, vs);
            glow_context.attach_shader(program, fs);
            // link and check for errors
            glow_context.link_program(program);
            dbg!(glow_context.get_program_info_log(program));
            if !glow_context.get_program_link_status(program) {
                let info_log = glow_context.get_program_info_log(program);
                panic!("failed to link glow program. info_log: {}", info_log);
            }
            glow_error!(glow_context);

            // detach and delete shaders
            glow_context.detach_shader(program, vs);
            glow_context.detach_shader(program, fs);
            glow_context.delete_shader(vs);
            glow_context.delete_shader(fs);

            let attr_count = glow_context.get_active_attributes(program);
            let mut attributes = vec![];
            for index in 0..attr_count {
                let attr_info = glow_context
                    .get_active_attribute(program, index)
                    .expect("failed to get attribute info");
                attributes.push(ProgramAttribute {
                    name: attr_info.name,
                    size: attr_info.size,
                    opengl_type: attr_info.atype,
                    location: index,
                });
            }

            let uniform_count = glow_context.get_active_uniforms(program);
            let mut uniforms = vec![];
            for index in 0..uniform_count {
                let uniform_info = glow_context
                    .get_active_uniform(program, index)
                    .expect("failed to get uniform info");
                let uniform_location = glow_context
                    .get_uniform_location(program, &uniform_info.name)
                    .expect("failed ot get uniform location");
                uniforms.push(ProgramUniform {
                    name: uniform_info.name,
                    size: uniform_info.size,
                    opengl_type: uniform_info.utype,
                    binding: uniform_location,
                });
            }
            glow_error!(glow_context);

            Self {
                program,
                uniforms,
                attributes,
                gl: glow_context.clone(),
            }
        }
    }
}
