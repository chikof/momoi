//! # overlay-system
//!
//! Composable overlay elements rendered on top of wallpapers.

pub mod clock;
pub mod compositor;
pub mod error;
pub mod widget;

pub use clock::ClockWidget;
pub use compositor::OverlayCompositor;
pub use error::OverlayError;
pub use widget::{OverlayWidget, WidgetRect};
