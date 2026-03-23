//! # overlay-system
//!
//! Composable overlay elements rendered on top of wallpapers.

pub mod compositor;
pub mod error;
pub mod widget;
pub mod widgets;

pub use compositor::OverlayCompositor;
pub use error::OverlayError;
pub use widget::{OverlayWidget, WidgetAnchor, WidgetRect};
pub use widgets::{ClockWidget, SystemStatsWidget};
