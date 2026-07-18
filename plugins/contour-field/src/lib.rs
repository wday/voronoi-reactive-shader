mod contour;
mod params;
mod shader;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<contour::ContourField>);
