mod slew;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<slew::SlewLimiter>);
