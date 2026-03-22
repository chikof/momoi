//! # render-core
//!
//! Abstract rendering interface for momoi.
//! All renderer backends implement the traits defined here.

pub mod error;
pub mod frame;
pub mod renderer;
pub mod surface;
pub mod texture;
pub mod uniforms;

pub use error::RenderError;
pub use frame::{Frame, FrameStats};
pub use renderer::{DynRenderer, RenderBackend, Renderer};
pub use surface::{OutputInfo, SurfaceDescriptor};
pub use texture::{PixelFormat, TextureDescriptor};
pub use uniforms::ShaderUniforms;
