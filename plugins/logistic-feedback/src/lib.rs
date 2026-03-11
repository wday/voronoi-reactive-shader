mod params;
mod shader;
mod logistic;

ffgl_core::plugin_main!(ffgl_core::handler::simplified::SimpleFFGLHandler<logistic::LogisticFeedback>);
