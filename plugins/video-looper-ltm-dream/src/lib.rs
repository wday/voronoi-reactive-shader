mod dream;
mod midi;
mod params;
mod pyramid;
mod shader;

use ffgl_core::{self, handler::simplified::SimpleFFGLHandler};

ffgl_core::plugin_main!(SimpleFFGLHandler<dream::DreamLooper>);
