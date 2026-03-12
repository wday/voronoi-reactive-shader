mod params;
mod shader;
mod displace;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<displace::ChannelDisplace>);
