{ lib, pkgs }:

with lib;

# Function to build TOML config from Nix settings
cfg:

let
  tomlFormat = pkgs.formats.toml { };
in

tomlFormat.generate "momoi-config.toml" (
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
)
