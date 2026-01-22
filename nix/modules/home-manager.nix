{
  config,
  lib,
  pkgs,
  momoiPackage,
  ...
}:

with lib;

let
  cfg = config.services.momoi;

  # Import shared options
  sharedOptions = import ./options.nix { inherit lib pkgs; };

  # Build config file
  configFile = import ./config-builder.nix { inherit lib pkgs; } cfg;
in

{
  imports = [ sharedOptions ];

  options.services.momoi.package = mkOption {
    type = types.package;
    default = momoiPackage;
    description = "The momoi package to use";
  };

  config = mkIf cfg.enable {
    # Install config file in home directory
    home.file.".config/momoi/config.toml".source = configFile;

    # Install package
    home.packages = [ cfg.package ];

    # Start service with Home Manager
    systemd.user.services.momoi = {
      Unit = {
        Description = "Momoi Wallpaper Daemon";
        PartOf = [ "graphical-session.target" ];
        After = [ "graphical-session.target" ];
      };

      Service = {
        ExecStart = "${cfg.package}/bin/momoi";
        Restart = "on-failure";
        RestartSec = 3;
      };

      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };
  };
}
