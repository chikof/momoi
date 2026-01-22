# Home Manager module for Momoi wallpaper daemon
# Can optionally receive `momoiFlake` via extraSpecialArgs, or use the package option directly

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

        1. Pass the momoi flake via extraSpecialArgs:
           extraSpecialArgs = { momoiFlake = inputs.momoi; };

        2. Set the package explicitly:
           services.momoi.package = inputs.momoi.packages.''${pkgs.system}.default;
      '';
in

{
  imports = [ sharedOptions ];

  config = mkIf cfg.enable {
    # Set the package default if not explicitly set
    services.momoi.package = mkDefault defaultPackage;

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
