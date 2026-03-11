mod params;
mod shader;
mod transform;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<transform::MirrorTransform>);
