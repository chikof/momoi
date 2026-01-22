# Easy NixOS module - automatically imports momoi when added to modules
# This wrapper makes it easier for users by auto-configuring specialArgs
#
# Usage in flake.nix:
# {
#   inputs.momoi.url = "github:chikof/momoi";
#
#   outputs = { nixpkgs, momoi, ... }: {
#     nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
#       modules = [
#         momoi.nixosModules.easy  # Just add this!
#         ./configuration.nix
#       ];
#     };
#   };
# }
#
# Then in configuration.nix:
# services.momoi.enable = true;

{ inputs, ... }:

{
  imports = [
    # Import the main momoi module
    (import ./nixos.nix)
  ];

  # Provide helpful error if inputs is not available
  _module.args = {
    momoiFlake =
      inputs.momoi or (throw ''
        The momoi flake input is not available.

        Make sure you have:
        1. Added momoi to your flake inputs
        2. Included 'inputs' in your module arguments
        3. Used: momoi.nixosModules.easy (not .default)
      '');
  };
}
