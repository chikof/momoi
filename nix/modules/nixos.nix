# NixOS module for Momoi wallpaper daemon
# Can optionally receive `momoiFlake` via specialArgs, or use the package option directly

{
  config,
  lib,
  pkgs,
  momoiFlake ? null,
  ...
}:

with lib;

let
  cfg = config.services.momoi;

  # Import shared options
  sharedOptions = import ./options.nix { inherit lib pkgs; };

  # Build config file
  configFile = import ./config-builder.nix { inherit lib pkgs; } cfg;

  # Get default package from momoiFlake if available
  defaultPackage =
    if momoiFlake != null then
      momoiFlake.packages.${pkgs.system}.default
        or (throw "Momoi package not available for system ${pkgs.system}")
    else
      throw ''
        Momoi package not found. You have two options:

        1. Pass the momoi flake via specialArgs:
           specialArgs = { momoiFlake = inputs.momoi; };

        2. Set the package explicitly:
           services.momoi.package = inputs.momoi.packages.''${pkgs.system}.default;
      '';
in

{
  imports = [ sharedOptions ];

  config = mkIf cfg.enable {
    # Set the package default if not explicitly set
    services.momoi.package = mkDefault defaultPackage;

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
