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
}
