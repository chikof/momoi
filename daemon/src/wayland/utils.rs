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

/// Parse transition type string to TransitionType enum
pub(super) fn parse_transition(transition: &str, duration: i32) -> common::TransitionType {
    match transition {
        "none" => common::TransitionType::None,
        "fade" => common::TransitionType::Fade {
            duration_ms: duration as u32,
        },
        "wipe-left" => common::TransitionType::WipeLeft {
            duration_ms: duration as u32,
        },
        "wipe-right" => common::TransitionType::WipeRight {
            duration_ms: duration as u32,
        },
        "wipe-top" => common::TransitionType::WipeTop {
            duration_ms: duration as u32,
        },
        "wipe-bottom" => common::TransitionType::WipeBottom {
            duration_ms: duration as u32,
        },
        "wipe-angle" => common::TransitionType::WipeAngle {
            angle_degrees: 45.0,
            duration_ms: duration as u32,
        },
        "center" => common::TransitionType::Center {
            duration_ms: duration as u32,
        },
        "outer" => common::TransitionType::Outer {
            duration_ms: duration as u32,
        },
        "random" => {
            use rand::Rng;
            let mut rng = rand::rng();
            let dur_ms = duration as u32;

            match rng.random_range(0..8) {
                0 => common::TransitionType::Fade {
                    duration_ms: dur_ms,
                },
                1 => common::TransitionType::WipeLeft {
                    duration_ms: dur_ms,
                },
                2 => common::TransitionType::WipeRight {
                    duration_ms: dur_ms,
                },
                3 => common::TransitionType::WipeTop {
                    duration_ms: dur_ms,
                },
                4 => common::TransitionType::WipeBottom {
                    duration_ms: dur_ms,
                },
                5 => common::TransitionType::WipeAngle {
                    angle_degrees: 45.0,
                    duration_ms: dur_ms,
                },
                6 => common::TransitionType::Center {
                    duration_ms: dur_ms,
                },
                _ => common::TransitionType::Outer {
                    duration_ms: dur_ms,
                },
            }
        }
        _ => common::TransitionType::Fade {
            duration_ms: duration as u32,
        },
    }
}
