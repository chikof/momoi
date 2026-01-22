# Easy Home Manager module - automatically imports momoi when added to modules
# This wrapper makes it easier for users by auto-configuring extraSpecialArgs
#
# Usage in flake.nix:
# {
#   inputs.momoi.url = "github:chikof/momoi";
#
#   outputs = { home-manager, momoi, ... }: {
#     homeConfigurations.myuser = home-manager.lib.homeManagerConfiguration {
#       modules = [
#         momoi.homeManagerModules.easy  # Just add this!
#         ./home.nix
#       ];
#     };
#   };
# }
#
# Then in home.nix:
# services.momoi.enable = true;

{ inputs, ... }:

{
  imports = [
    # Import the main momoi module
    (import ./home-manager.nix)
  ];

  # Provide helpful error if inputs is not available
  _module.args = {
    momoiFlake =
      inputs.momoi or (throw ''
        The momoi flake input is not available.

        Make sure you have:
        1. Added momoi to your flake inputs
        2. Included 'inputs' in your module arguments
        3. Used: momoi.homeManagerModules.easy (not .default)
      '');
  };
}
