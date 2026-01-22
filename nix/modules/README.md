# Nix Module Structure

This directory contains the modular Nix configuration for Momoi.

## Structure

```
nix/modules/
├── options.nix         # Shared option definitions (403 lines)
├── config-builder.nix  # TOML config generator (90 lines)
├── nixos.nix          # NixOS module (59 lines)
└── home-manager.nix   # Home Manager module (56 lines)
```

## Files

### `options.nix`
Defines all configuration options shared between NixOS and Home Manager modules:
- General settings (log level, transitions, scaling)
- Advanced settings (video, performance, resource management)
- Playlist configuration
- Schedule entries
- Per-output configuration
- Wallpaper collections
- Shader presets

### `config-builder.nix`
Converts Nix configuration to TOML format:
- Takes a `cfg` parameter (the config object)
- Returns a TOML file path
- Handles optional sections (playlist, schedule, outputs, etc.)
- Maps Nix-style names (camelCase) to TOML-style (snake_case)

### `nixos.nix`
NixOS system module:
- Imports shared options
- Sets default package
- Installs config in `/etc/momoi/config.toml`
- Creates symlink to user's `~/.config/momoi/config.toml`
- Manages systemd user service

### `home-manager.nix`
Home Manager user module:
- Imports shared options
- Sets default package
- Installs config in `~/.config/momoi/config.toml`
- Installs package per-user
- Manages systemd user service

## Benefits

### Maintainability
- **Single source of truth**: Options defined once in `options.nix`
- **DRY principle**: Config building logic in one place
- **Clear separation**: Each file has a single responsibility
- **Easy updates**: Change options in one place, affects both modules

### Code Reduction
- **Before**: 1202 lines in `flake.nix`
- **After**: 163 lines in `flake.nix` + 608 lines in modules
- **Total reduction**: ~430 lines saved through deduplication

### Modularity
- Easy to add new options (just edit `options.nix`)
- Easy to create new module types (copy `nixos.nix` or `home-manager.nix`)
- Config building logic can be reused for other formats

## Usage in flake.nix

```nix
nixosModules.default = import ./nix/modules/nixos.nix {
  inherit (pkgs) lib;
  momoiPackage = self.packages.${system}.default;
};

homeManagerModules.default = import ./nix/modules/home-manager.nix {
  inherit (pkgs) lib;
  momoiPackage = self.packages.${system}.default;
};
```

## Adding New Options

1. Add option definition to `options.nix`:
   ```nix
   newOption = mkOption {
     type = types.str;
     default = "default-value";
     description = "Description of the option";
   };
   ```

2. Add TOML mapping to `config-builder.nix`:
   ```nix
   new_option = cfg.settings.newOption;
   ```

3. No changes needed in `nixos.nix` or `home-manager.nix`!

## Testing

Verify all modules work:
```bash
nix flake check
```

Build NixOS module:
```bash
nix eval .#nixosModules.x86_64-linux.default
```

Build Home Manager module:
```bash
nix eval .#homeManagerModules.default
```
