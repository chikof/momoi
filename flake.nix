{
  description = "Momoi - Advanced Wayland wallpaper daemon with multi-format support";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };

        # Build dependencies for momoi
        buildInputs = with pkgs; [
          wayland
          wayland-protocols
          libxkbcommon
          vulkan-loader
          vulkan-headers
          libGL
          ffmpeg
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gst_all_1.gst-libav
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          cmake
          rustToolchain
          makeWrapper
        ];

        # Runtime libraries needed
        runtimeLibs = with pkgs; [
          wayland
          libxkbcommon
          vulkan-loader
          libGL
          ffmpeg
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
        ];

      in
      {
        packages = {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "momoi";
            version = "0.1.0";

            src = pkgs.lib.cleanSource ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            inherit nativeBuildInputs buildInputs;

            # Build with all features
            buildFeatures = [ "all" ];

            # Add runtime library paths
            postInstall = ''
              wrapProgram $out/bin/momoi \
                --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath runtimeLibs}"
            '';

            meta = with pkgs.lib; {
              description = "Momoi - Advanced Wayland wallpaper daemon with support for images, videos, and animated wallpapers";
              homepage = "https://github.com/chikof/momoi";
              license = licenses.mit;
              platforms = platforms.linux;
              mainProgram = "momoi";
            };
          };
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;

          nativeBuildInputs =
            nativeBuildInputs
            ++ (with pkgs; [
              # Development tools
              cargo-watch
              cargo-edit
              cargo-expand
              rustfmt
              clippy

              # Debugging and profiling
              gdb
              valgrind

              # Additional utilities
              wayland-utils
              vulkan-tools
            ]);

          # Environment variables for development
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (buildInputs ++ runtimeLibs);
          PKG_CONFIG_PATH = "${pkgs.lib.makeSearchPath "lib/pkgconfig" buildInputs}";

          shellHook = ''
            echo "🎨 Momoi - Wayland Wallpaper Daemon"
            echo "===================================="
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build          - Build the project"
            echo "  cargo run            - Run the daemon"
            echo "  cargo test           - Run tests"
            echo "  cargo watch          - Watch for changes and rebuild"
            echo "  cargo clippy         - Run linter"
            echo ""
            echo "To build the Nix package: nix build"
            echo "To update flake: nix flake update"
            echo ""
          '';
        };

        # NixOS module to manage momoi service with full config support
        nixosModules.default =
          {
            config,
            lib,
            pkgs,
            ...
          }:
          with lib;
          let
            cfg = config.services.momoi;

            # Convert Nix config to TOML format
            tomlFormat = pkgs.formats.toml { };

            # Build the configuration attrset
            configFile = tomlFormat.generate "momoi-config.toml" (
              {
                general = {
                  log_level = cfg.settings.general.logLevel;
                  default_transition = cfg.settings.general.defaultTransition;
                  default_duration = cfg.settings.general.defaultDuration;
                  default_scale = cfg.settings.general.defaultScale;
                };
                advanced = {
                  enable_video = cfg.settings.advanced.enableVideo;
                  video_muted = cfg.settings.advanced.videoMuted;
                  video_loop = cfg.settings.advanced.videoLoop;
                  max_fps = cfg.settings.advanced.maxFps;
                  cache_limit_mb = cfg.settings.advanced.cacheLimitMb;
                  preload_next = cfg.settings.advanced.preloadNext;
                  performance_mode = cfg.settings.advanced.performanceMode;
                  auto_battery_mode = cfg.settings.advanced.autoBatteryMode;
                  enforce_memory_limits = cfg.settings.advanced.enforceMemoryLimits;
                  max_memory_mb = cfg.settings.advanced.maxMemoryMb;
                  cpu_threshold = cfg.settings.advanced.cpuThreshold;
                };
              }
              // optionalAttrs (cfg.settings.playlist != null) {
                playlist = {
                  enabled = cfg.settings.playlist.enabled;
                  interval = cfg.settings.playlist.interval;
                  shuffle = cfg.settings.playlist.shuffle;
                  transition = cfg.settings.playlist.transition;
                  transition_duration = cfg.settings.playlist.transitionDuration;
                  sources = cfg.settings.playlist.sources;
                  extensions = cfg.settings.playlist.extensions;
                };
              }
              // optionalAttrs (cfg.settings.schedule != [ ]) {
                schedule = map (s: {
                  name = s.name;
                  start_time = s.startTime;
                  end_time = s.endTime;
                  wallpaper = s.wallpaper;
                  transition = s.transition;
                  duration = s.duration;
                }) cfg.settings.schedule;
              }
              // optionalAttrs (cfg.settings.outputs != [ ]) {
                output = map (o: {
                  name = o.name;
                  wallpaper = o.wallpaper;
                  scale = o.scale;
                  transition = o.transition;
                  duration = o.duration;
                  playlist = o.playlist;
                  playlist_sources = o.playlistSources;
                }) cfg.settings.outputs;
              }
              // optionalAttrs (cfg.settings.collections != [ ]) {
                collection = map (c: {
                  name = c.name;
                  description = c.description;
                  wallpapers = c.wallpapers;
                }) cfg.settings.collections;
              }
              // optionalAttrs (cfg.settings.shaderPresets != [ ]) {
                shader_preset = map (
                  s:
                  {
                    name = s.name;
                    shader = s.shader;
                    description = s.description;
                  }
                  // optionalAttrs (s.speed != null) { speed = s.speed; }
                  // optionalAttrs (s.color1 != null) { color1 = s.color1; }
                  // optionalAttrs (s.color2 != null) { color2 = s.color2; }
                  // optionalAttrs (s.color3 != null) { color3 = s.color3; }
                  // optionalAttrs (s.scale != null) { scale = s.scale; }
                  // optionalAttrs (s.intensity != null) { intensity = s.intensity; }
                  // optionalAttrs (s.count != null) { count = s.count; }
                ) cfg.settings.shaderPresets;
              }
            );
          in
          {
            options.services.momoi = {
              enable = mkEnableOption "Momoi wallpaper daemon";

              package = mkOption {
                type = types.package;
                default = self.packages.${system}.default;
                description = "The momoi package to use";
              };

              settings = {
                general = {
                  logLevel = mkOption {
                    type = types.enum [
                      "trace"
                      "debug"
                      "info"
                      "warn"
                      "error"
                    ];
                    default = "info";
                    description = "Log level for the daemon";
                  };

                  defaultTransition = mkOption {
                    type = types.enum [
                      "none"
                      "fade"
                      "wipe-left"
                      "wipe-right"
                      "wipe-top"
                      "wipe-bottom"
                      "wipe-angle"
                      "center"
                      "outer"
                      "random"
                    ];
                    default = "fade";
                    description = "Default transition effect";
                  };

                  defaultDuration = mkOption {
                    type = types.ints.positive;
                    default = 500;
                    description = "Default transition duration in milliseconds";
                  };

                  defaultScale = mkOption {
                    type = types.enum [
                      "center"
                      "fill"
                      "fit"
                      "stretch"
                      "tile"
                    ];
                    default = "fill";
                    description = "Default image scaling mode";
                  };
                };

                advanced = {
                  enableVideo = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Enable video wallpaper support";
                  };

                  videoMuted = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Mute video audio by default";
                  };

                  videoLoop = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Loop videos automatically";
                  };

                  maxFps = mkOption {
                    type = types.ints.positive;
                    default = 60;
                    description = "Maximum frames per second";
                  };

                  cacheLimitMb = mkOption {
                    type = types.ints.unsigned;
                    default = 500;
                    description = "Cache size limit in megabytes";
                  };

                  preloadNext = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Preload next wallpaper in playlist";
                  };

                  performanceMode = mkOption {
                    type = types.enum [
                      "performance"
                      "balanced"
                      "powersave"
                    ];
                    default = "balanced";
                    description = "Performance mode";
                  };

                  autoBatteryMode = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Automatically switch to powersave mode on battery";
                  };

                  enforceMemoryLimits = mkOption {
                    type = types.bool;
                    default = true;
                    description = "Enforce memory usage limits";
                  };

                  maxMemoryMb = mkOption {
                    type = types.ints.positive;
                    default = 300;
                    description = "Maximum memory usage in megabytes";
                  };

                  cpuThreshold = mkOption {
                    type = types.float;
                    default = 80.0;
                    description = "CPU usage threshold percentage";
                  };
                };

                playlist = mkOption {
                  type = types.nullOr (
                    types.submodule {
                      options = {
                        enabled = mkOption {
                          type = types.bool;
                          default = true;
                          description = "Enable playlist mode";
                        };

                        interval = mkOption {
                          type = types.ints.positive;
                          default = 300;
                          description = "Wallpaper rotation interval in seconds";
                        };

                        shuffle = mkOption {
                          type = types.bool;
                          default = false;
                          description = "Shuffle playlist order";
                        };

                        transition = mkOption {
                          type = types.str;
                          default = "fade";
                          description = "Transition effect for playlist changes";
                        };

                        transitionDuration = mkOption {
                          type = types.ints.positive;
                          default = 500;
                          description = "Transition duration in milliseconds";
                        };

                        sources = mkOption {
                          type = types.listOf types.str;
                          default = [ ];
                          example = [
                            "~/Pictures/Wallpapers"
                            "~/wallpapers/*.jpg"
                          ];
                          description = "List of wallpaper sources (directories or glob patterns)";
                        };

                        extensions = mkOption {
                          type = types.listOf types.str;
                          default = [
                            "jpg"
                            "jpeg"
                            "png"
                            "webp"
                            "gif"
                            "mp4"
                            "webm"
                            "mkv"
                          ];
                          description = "File extensions to include";
                        };
                      };
                    }
                  );
                  default = null;
                  description = "Playlist configuration";
                };

                schedule = mkOption {
                  type = types.listOf (
                    types.submodule {
                      options = {
                        name = mkOption {
                          type = types.str;
                          description = "Schedule entry name";
                        };

                        startTime = mkOption {
                          type = types.strMatching "[0-2][0-9]:[0-5][0-9]";
                          example = "07:00";
                          description = "Start time (HH:MM format)";
                        };

                        endTime = mkOption {
                          type = types.strMatching "[0-2][0-9]:[0-5][0-9]";
                          example = "19:00";
                          description = "End time (HH:MM format)";
                        };

                        wallpaper = mkOption {
                          type = types.str;
                          description = "Path to wallpaper file";
                        };

                        transition = mkOption {
                          type = types.str;
                          default = "fade";
                          description = "Transition effect";
                        };

                        duration = mkOption {
                          type = types.ints.positive;
                          default = 500;
                          description = "Transition duration in milliseconds";
                        };
                      };
                    }
                  );
                  default = [ ];
                  description = "Time-based schedule entries";
                };

                outputs = mkOption {
                  type = types.listOf (
                    types.submodule {
                      options = {
                        name = mkOption {
                          type = types.str;
                          description = "Output name (e.g., DP-1)";
                        };

                        wallpaper = mkOption {
                          type = types.nullOr types.str;
                          default = null;
                          description = "Wallpaper path for this output";
                        };

                        scale = mkOption {
                          type = types.str;
                          default = "fill";
                          description = "Scaling mode for this output";
                        };

                        transition = mkOption {
                          type = types.str;
                          default = "fade";
                          description = "Transition effect for this output";
                        };

                        duration = mkOption {
                          type = types.ints.positive;
                          default = 500;
                          description = "Transition duration in milliseconds";
                        };

                        playlist = mkOption {
                          type = types.bool;
                          default = false;
                          description = "Enable playlist for this output";
                        };

                        playlistSources = mkOption {
                          type = types.listOf types.str;
                          default = [ ];
                          description = "Playlist sources for this output";
                        };
                      };
                    }
                  );
                  default = [ ];
                  description = "Per-output configuration";
                };

                collections = mkOption {
                  type = types.listOf (
                    types.submodule {
                      options = {
                        name = mkOption {
                          type = types.str;
                          description = "Collection name";
                        };

                        description = mkOption {
                          type = types.str;
                          default = "";
                          description = "Collection description";
                        };

                        wallpapers = mkOption {
                          type = types.listOf types.str;
                          default = [ ];
                          description = "List of wallpaper paths";
                        };
                      };
                    }
                  );
                  default = [ ];
                  description = "Named wallpaper collections";
                };

                shaderPresets = mkOption {
                  type = types.listOf (
                    types.submodule {
                      options = {
                        name = mkOption {
                          type = types.str;
                          description = "Preset name";
                        };

                        shader = mkOption {
                          type = types.enum [
                            "plasma"
                            "waves"
                            "matrix"
                            "gradient"
                            "starfield"
                            "raymarching"
                            "tunnel"
                          ];
                          description = "Shader type";
                        };

                        description = mkOption {
                          type = types.str;
                          default = "";
                          description = "Preset description";
                        };

                        speed = mkOption {
                          type = types.nullOr types.float;
                          default = null;
                          description = "Animation speed multiplier";
                        };

                        color1 = mkOption {
                          type = types.nullOr types.str;
                          default = null;
                          example = "FF0000";
                          description = "Primary color (hex format)";
                        };

                        color2 = mkOption {
                          type = types.nullOr types.str;
                          default = null;
                          example = "0000FF";
                          description = "Secondary color (hex format)";
                        };

                        color3 = mkOption {
                          type = types.nullOr types.str;
                          default = null;
                          example = "00FF00";
                          description = "Tertiary color (hex format)";
                        };

                        scale = mkOption {
                          type = types.nullOr types.float;
                          default = null;
                          description = "Scale parameter";
                        };

                        intensity = mkOption {
                          type = types.nullOr types.float;
                          default = null;
                          description = "Intensity parameter (0.0-1.0)";
                        };

                        count = mkOption {
                          type = types.nullOr types.ints.positive;
                          default = null;
                          description = "Count parameter (e.g., number of objects)";
                        };
                      };
                    }
                  );
                  default = [ ];
                  description = "Shader presets";
                };
              };
            };

            config = mkIf cfg.enable {
              # Install package system-wide
              environment.systemPackages = [ cfg.package ];

              # Create config file in /etc
              environment.etc."momoi/config.toml" = {
                source = configFile;
                mode = "0644";
              };

              # User service that links the config
              systemd.user.services.momoi = {
                description = "Momoi Wallpaper Daemon";
                wantedBy = [ "graphical-session.target" ];
                partOf = [ "graphical-session.target" ];
                after = [ "graphical-session.target" ];

                preStart = ''
                  mkdir -p ''${HOME}/.config/momoi
                  ln -sf /etc/momoi/config.toml ''${HOME}/.config/momoi/config.toml || true
                '';

                serviceConfig = {
                  ExecStart = "${cfg.package}/bin/momoi";
                  Restart = "on-failure";
                  RestartSec = 3;
                };
              };
            };
          };
      }
    );
}
