mod delay;
mod params;
mod registry;
mod shader;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<delay::DelayLine>);
