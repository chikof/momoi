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
    let
      # System-specific outputs
      systemOutputs = flake-utils.lib.eachDefaultSystem (
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
        }
      );
    in
    systemOutputs
    // {
      # NixOS modules
      nixosModules = {
        # Default module - requires momoiFlake to be passed via specialArgs
        # Usage:
        #   specialArgs = { momoiFlake = inputs.momoi; };
        #   modules = [ inputs.momoi.nixosModules.default ];
        default = import ./nix/modules/nixos.nix;

        # Helper for easy setup - automatically passes momoiFlake
        # Usage:
        #   modules = [ (inputs.momoi.nixosModules.autoload { inherit inputs; }) ];
        autoload =
          { inputs }:
          {
            imports = [ (import ./nix/modules/nixos.nix) ];
            _module.args.momoiFlake = inputs.momoi;
          };
      };

      # Home Manager modules
      homeManagerModules = {
        # Default module - requires momoiFlake to be passed via extraSpecialArgs
        # Usage:
        #   extraSpecialArgs = { momoiFlake = inputs.momoi; };
        #   modules = [ inputs.momoi.homeManagerModules.default ];
        default = import ./nix/modules/home-manager.nix;

        # Helper for easy setup - automatically passes momoiFlake
        # Usage:
        #   modules = [ (inputs.momoi.homeManagerModules.autoload { inherit inputs; }) ];
        autoload =
          { inputs }:
          {
            imports = [ (import ./nix/modules/home-manager.nix) ];
            _module.args.momoiFlake = inputs.momoi;
          };
      };
    };
}
