/// Utility functions for the Wayland manager

/// Parse scale mode string to ScaleMode enum
pub(super) fn parse_scale_mode(scale: &str) -> common::ScaleMode {
    match scale {
        "center" => common::ScaleMode::Center,
        "fill" => common::ScaleMode::Fill,
        "fit" => common::ScaleMode::Fit,
        "stretch" => common::ScaleMode::Stretch,
        "tile" => common::ScaleMode::Tile,
        _ => common::ScaleMode::Fill,
    }
}
