mod clock;
mod params;
mod shader;
mod voronoi;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<voronoi::VoronoiFractal>);
