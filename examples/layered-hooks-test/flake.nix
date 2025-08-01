{
  description = "Test flake for cuenv layered hooks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ nodejs_20 python3 curl ];

        shellHook = ''
          export FLAKE_SHELL_ACTIVE="true"
          export NODE_VERSION="$(node --version)"
          export PYTHON_VERSION="$(python3 --version)"
          echo "🔧 Nix flake environment loaded"
        '';
      };
    };
}
