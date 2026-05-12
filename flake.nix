{
  description = "Standalone C4 Rust port development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              git
              gnumake
              mise
              rustup
            ];

            shellHook = ''
              export MISE_DISABLE_VERSION_CHECK=1
              mise install
              echo "c4-rs dev shell: rust is managed by rustup via mise"
              echo "Useful commands: mise run build, mise run test"
            '';
          };
        });
    };
}
