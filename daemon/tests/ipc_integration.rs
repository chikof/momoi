/// Integration tests for IPC communication
/// These tests verify that commands and responses serialize correctly
/// and can be sent over IPC boundaries
use common::{Command, OverlayParams, Response, ScaleMode, ShaderParams, TransitionType};

#[test]
fn test_command_response_roundtrip() {
    // Test SetWallpaper command
    let cmd = Command::SetWallpaper {
        path: "/tmp/test.png".to_string(),
        output: Some("DP-1".to_string()),
        transition: Some(TransitionType::Fade { duration_ms: 500 }),
        scale: Some(ScaleMode::Fill),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: Command = serde_json::from_str(&json).unwrap();

    // Verify the command deserializes correctly
    match deserialized {
        Command::SetWallpaper {
            path,
            output,
            transition,
            scale,
        } => {
            assert_eq!(path, "/tmp/test.png");
            assert_eq!(output, Some("DP-1".to_string()));
            assert!(matches!(
                transition,
                Some(TransitionType::Fade { duration_ms: 500 })
            ));
            assert!(matches!(scale, Some(ScaleMode::Fill)));
        }
        _ => panic!("Wrong command type"),
    }
}

#[test]
fn test_shader_command_with_params() {
    let params = ShaderParams {
        speed: Some(2.0),
        color1: Some("FF0000".to_string()),
        color2: Some("00FF00".to_string()),
        color3: Some("0000FF".to_string()),
        scale: Some(1.5),
        intensity: Some(0.8),
        count: Some(100),
    };

    let cmd = Command::SetShader {
        shader: "plasma".to_string(),
        output: Some("DP-2".to_string()),
        transition: None,
        params: Some(params),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: Command = serde_json::from_str(&json).unwrap();

    match deserialized {
        Command::SetShader { shader, params, .. } => {
            assert_eq!(shader, "plasma");
            let p = params.unwrap();
            assert_eq!(p.speed, Some(2.0));
            assert_eq!(p.color1, Some("FF0000".to_string()));
            assert_eq!(p.count, Some(100));
        }
        _ => panic!("Wrong command type"),
    }
}

#[test]
fn test_playlist_commands() {
    let commands = vec![
        Command::PlaylistNext,
        Command::PlaylistPrev,
        Command::PlaylistToggleShuffle,
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd).unwrap();
        let _deserialized: Command = serde_json::from_str(&json).unwrap();
        // Just verify it serializes and deserializes without error
    }
}

#[test]
fn test_query_commands() {
    let commands = vec![
        Command::Query,
        Command::Ping,
        Command::ListOutputs,
        Command::GetResources,
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd).unwrap();
        let _deserialized: Command = serde_json::from_str(&json).unwrap();
    }
}

#[test]
fn test_response_types() {
    // Test Ok response
    let resp = Response::Ok;
    let json = serde_json::to_string(&resp).unwrap();
    let deserialized: Response = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Response::Ok));

    // Test Pong response
    let resp = Response::Pong;
    let json = serde_json::to_string(&resp).unwrap();
    let deserialized: Response = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, Response::Pong));
}

#[test]
fn test_overlay_commands() {
    let cmd = Command::SetOverlay {
        overlay: "blur".to_string(),
        params: Some(OverlayParams {
            intensity: Some(0.5),
            strength: Some(2.0),
            ..Default::default()
        }),
        output: Some("DP-1".to_string()),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: Command = serde_json::from_str(&json).unwrap();

    match deserialized {
        Command::SetOverlay {
            overlay, params, ..
        } => {
            let params = params.unwrap().clone();

            assert_eq!(overlay, "blur");
            assert_eq!(params.intensity, Some(0.5));
            assert_eq!(params.strength, Some(2.0));
        }
        _ => panic!("Wrong command type"),
    }

    // Test ClearOverlay
    let cmd = Command::ClearOverlay { output: None };
    let json = serde_json::to_string(&cmd).unwrap();
    let _: Command = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_transition_types_serialization() {
    let transitions = vec![
        TransitionType::None,
        TransitionType::Fade { duration_ms: 300 },
        TransitionType::WipeLeft { duration_ms: 500 },
        TransitionType::WipeRight { duration_ms: 500 },
        TransitionType::WipeTop { duration_ms: 400 },
        TransitionType::WipeBottom { duration_ms: 400 },
        TransitionType::WipeAngle {
            angle_degrees: 45.0,
            duration_ms: 600,
        },
        TransitionType::Center { duration_ms: 350 },
        TransitionType::Outer { duration_ms: 350 },
        TransitionType::Random { duration_ms: 500 },
    ];

    for transition in transitions {
        let json = serde_json::to_string(&transition).unwrap();
        let _: TransitionType = serde_json::from_str(&json).unwrap();
    }
}

#[test]
fn test_shader_params_optional_fields() {
    // Test params with only some fields set
    let params = ShaderParams {
        speed: Some(1.5),
        color1: Some("FF0000".to_string()),
        color2: None,
        color3: None,
        scale: None,
        intensity: None,
        count: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    let deserialized: ShaderParams = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.speed, Some(1.5));
    assert_eq!(deserialized.color1, Some("FF0000".to_string()));
    assert!(deserialized.color2.is_none());
    assert!(deserialized.count.is_none());
}

#[test]
fn test_color_command() {
    let cmd = Command::SetColor {
        color: "FF5733".to_string(),
        output: Some("DP-1".to_string()),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: Command = serde_json::from_str(&json).unwrap();

    match deserialized {
        Command::SetColor { color, output } => {
            assert_eq!(color, "FF5733");
            assert_eq!(output, Some("DP-1".to_string()));
        }
        _ => panic!("Wrong command type"),
    }
}

#[test]
fn test_performance_mode_command() {
    let cmd = Command::SetPerformanceMode {
        mode: "performance".to_string(),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: Command = serde_json::from_str(&json).unwrap();

    match deserialized {
        Command::SetPerformanceMode { mode } => {
            assert_eq!(mode, "performance");
        }
        _ => panic!("Wrong command type"),
    }
}
