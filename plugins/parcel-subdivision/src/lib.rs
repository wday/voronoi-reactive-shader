mod params;
mod parcel;
mod shader;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<parcel::ParcelSubdivision>);
