mod params;
mod shader;
mod slipgrid;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<slipgrid::Slipgrid>);
