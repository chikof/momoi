# Momoi Installation Guide

This guide shows different ways to install and configure Momoi on NixOS, from easiest to most flexible.

## Table of Contents

- [Quick Start (Easiest)](#quick-start-easiest)
- [Method 1: Autoload Module (Recommended)](#method-1-autoload-module-recommended)
- [Method 2: Manual specialArgs](#method-2-manual-specialargs)
- [Method 3: Explicit Package](#method-3-explicit-package)
- [Home Manager Installation](#home-manager-installation)

---

## Quick Start (Easiest)

This is the simplest way to get started with Momoi:

### 1. Add to your flake.nix

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { self, nixpkgs, momoi, ... }@inputs: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      specialArgs = { inherit inputs; };  # Pass inputs to modules
      modules = [
        # Add this single line - it auto-configures everything!
        (momoi.nixosModules.autoload { inherit inputs; })
        ./configuration.nix
      ];
    };
  };
}
```

### 2. Enable in configuration.nix

```nix
{
  services.momoi.enable = true;

  # Optional: configure wallpapers
  services.momoi.settings.outputs = [
    {
      name = "HDMI-1";
      wallpaper = "~/wallpaper.jpg";
    }
  ];
}
```

### 3. Rebuild

```bash
sudo nixos-rebuild switch --flake .#yourhost
```

That's it! Momoi is now installed and running.

---

## Method 1: Autoload Module (Recommended)

The `autoload` helper automatically configures everything for you. You just need to pass `inputs` to it.

### Setup

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { nixpkgs, momoi, ... }@inputs: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";

      # Option A: Pass inputs via specialArgs
      specialArgs = { inherit inputs; };
      modules = [
        (momoi.nixosModules.autoload { inherit inputs; })
        ./configuration.nix
      ];

      # Option B: If inputs is already in specialArgs, modules can access it directly
      # specialArgs = { inherit inputs; };  # Already passed elsewhere
      # modules = [
      #   ({ inputs, ... }: {
      #     imports = [ (momoi.nixosModules.autoload { inherit inputs; }) ];
      #   })
      #   ./configuration.nix
      # ];
    };
  };
}
```

### Usage in configuration.nix

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      general = {
        logLevel = "info";
        defaultTransition = "fade";
      };

      outputs = [
        {
          name = "DP-1";
          wallpaper = "/home/user/wallpaper.mp4";
          scale = "fill";
        }
      ];
    };
  };
}
```

**Advantages:**

- ✅ Simplest setup - just one line
- ✅ No manual specialArgs configuration needed
- ✅ Works with your existing flake structure

---

## Method 2: Manual specialArgs

If you prefer more control, you can manually configure `specialArgs`.

### Setup

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { nixpkgs, momoi, ... }: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";

      specialArgs = {
        momoiFlake = momoi;  # Pass momoi flake to modules
      };

      modules = [
        momoi.nixosModules.default  # Use default module
        ./configuration.nix
      ];
    };
  };
}
```

### Usage in configuration.nix

Same as Method 1 - just configure `services.momoi`.

**Advantages:**

- ✅ More explicit control
- ✅ Good for complex flake setups
- ✅ Clear dependency tracking

---

## Method 3: Explicit Package

Set the package explicitly in your configuration instead of via specialArgs.

### Setup

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { nixpkgs, momoi, ... }@inputs: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";

      specialArgs = { inherit inputs; };

      modules = [
        momoi.nixosModules.default
        ./configuration.nix
      ];
    };
  };
}
```

### Usage in configuration.nix

```nix
{ pkgs, inputs, ... }:

{
  services.momoi = {
    enable = true;

    # Explicitly set the package
    package = inputs.momoi.packages.${pkgs.system}.default;

    settings = {
      # Your configuration here
    };
  };
}
```

**Advantages:**

- ✅ Works without momoiFlake in specialArgs
- ✅ Can override package version easily
- ✅ Useful for testing different versions

---

## Home Manager Installation

Momoi can also be installed via Home Manager for per-user configuration.

### Setup

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager.url = "github:nix-community/home-manager";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { nixpkgs, home-manager, momoi, ... }@inputs: {
    homeConfigurations.youruser = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;

      # Use autoload for easy setup
      modules = [
        (momoi.homeManagerModules.autoload { inherit inputs; })
        ./home.nix
      ];

      # OR use manual method:
      # extraSpecialArgs = { momoiFlake = momoi; };
      # modules = [
      #   momoi.homeManagerModules.default
      #   ./home.nix
      # ];
    };
  };
}
```

### Usage in home.nix

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      general.logLevel = "info";

      playlist = {
        enabled = true;
        interval = 300;
        sources = [ "~/Pictures/Wallpapers" ];
      };
    };
  };
}
```

### Differences from NixOS Module

- Config file location: `~/.config/momoi/config.toml` (not `/etc/momoi/config.toml`)
- Service runs as user service (managed by Home Manager)
- Package installed to user profile (not system-wide)

---

## Configuration Reference

See [NIXOS_CONFIGURATION.md](./NIXOS_CONFIGURATION.md) for complete configuration options.

---

## Troubleshooting

### Error: "Momoi package not found"

**Solution:** Make sure you're using one of these methods:

1. Use `autoload`: `(momoi.nixosModules.autoload { inherit inputs; })`
2. Add `momoiFlake = momoi;` to `specialArgs`
3. Set `services.momoi.package = inputs.momoi.packages.${pkgs.system}.default;`

### Error: "momoiFlake not found"

**Solution:** Either:

- Use the `autoload` module instead of `default`
- Add `momoiFlake = inputs.momoi;` to your `specialArgs`

### Service not starting

Check the service status:

```bash
systemctl --user status momoi
journalctl --user -u momoi -f
```

Common issues:

- Wallpaper file doesn't exist
- Invalid output name (check with `hyprctl monitors` or `wlr-randr`)
- Video codec not supported (install GStreamer plugins)

---

## Next Steps

- [View full configuration options](./NIXOS_CONFIGURATION.md)
- [Read the README](./README.md)
