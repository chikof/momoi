//! # shader-engine
//!
//! Shader compilation, validation, uniform management, and hot-reload.
//!
//! # Features
//! - `compile` — enables WGSL/GLSL compilation via `naga`
//! - `hot-reload` — enables filesystem watching via `notify`
//! - `gpu-pipeline` — enables `wgpu` render pipeline creation

pub mod error;
pub mod registry;

pub use error::ShaderError;
pub use registry::{ShaderId, ShaderRegistry};

#[cfg(feature = "compile")]
pub mod compiler;
#[cfg(feature = "compile")]
pub use compiler::{CompiledShader, ShaderCompiler, ShaderLanguage, ShaderSource};

#[cfg(feature = "hot-reload")]
pub mod hot_reload;
#[cfg(feature = "hot-reload")]
pub use hot_reload::ShaderWatcher;

#[cfg(feature = "gpu-pipeline")]
pub mod pipeline;
#[cfg(feature = "gpu-pipeline")]
pub use pipeline::ShaderPipeline;
