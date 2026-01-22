{ lib, pkgs, ... }:

with lib;

{
  options.services.momoi = {
    enable = mkEnableOption "Momoi wallpaper daemon";

    package = mkOption {
      type = types.package;
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
}
