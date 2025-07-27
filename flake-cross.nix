{
  description = "cuenv cross-compilation builds";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    let
      # Define the systems we want to build for
      targetSystems = {
        "x86_64-linux" = {
          rust = "x86_64-unknown-linux-gnu";
          nix = "x86_64-linux";
        };
        "aarch64-linux" = {
          rust = "aarch64-unknown-linux-gnu";
          nix = "aarch64-linux";
        };
        "x86_64-darwin" = {
          rust = "x86_64-apple-darwin";
          nix = "x86_64-darwin";
        };
        "aarch64-darwin" = {
          rust = "aarch64-apple-darwin";  
          nix = "aarch64-darwin";
        };
      };
      
      # Build cuenv for a specific target
      buildCuenvForTarget = hostSystem: targetSystem: targetRust:
        let
          pkgs = import nixpkgs {
            system = hostSystem;
            overlays = [ (import rust-overlay) ];
          };
          
          # Check if we're cross-compiling
          isCross = hostSystem != targetSystem;
          
          # Get the appropriate rust toolchain
          rustToolchain = if isCross then
            pkgs.rust-bin.stable."1.88.0".default.override {
              targets = [ targetRust ];
            }
          else
            pkgs.rust-bin.stable."1.88.0".default;
            
          # Platform-specific dependencies
          buildInputs = with pkgs; [
            go_1_24
            openssl
            pkg-config
          ] ++ (if pkgs.stdenv.isDarwin then [
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.CoreFoundation
          ] else if pkgs.stdenv.isLinux then [
            glibc
          ] else []);
          
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          version = cargoToml.package.version;
          
        in pkgs.rustPlatform.buildRustPackage {
          pname = "cuenv-${targetRust}";
          inherit version;
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
            gcc
            go_1_24
          ];
          
          inherit buildInputs;
          
          # Set target for cross-compilation
          CARGO_BUILD_TARGET = if isCross then targetRust else null;
          
          # Required for cross-compilation
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = if isCross && targetRust == "x86_64-unknown-linux-gnu" then "${pkgs.pkgsCross.gnu64.stdenv.cc}/bin/x86_64-unknown-linux-gnu-gcc" else null;
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER = if isCross && targetRust == "aarch64-unknown-linux-gnu" then "${pkgs.pkgsCross.aarch64-multiplatform.stdenv.cc}/bin/aarch64-unknown-linux-gnu-gcc" else null;
          
          preBuild = ''
            export HOME=$(mktemp -d)
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/go-cache"
            export CGO_ENABLED=1
          '';
          
          # Platform-specific linker flags
          RUSTFLAGS = if pkgs.stdenv.isDarwin then
            "-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation"
          else "";
          
          CGO_ENABLED = "1";
          GO = "${pkgs.go_1_24}/bin/go";
          
          checkPhase = ''
            runHook preCheck
            # Skip tests for cross-compilation
            ${if isCross then "echo 'Skipping tests for cross-compilation'" else "cargo test --offline"}
            runHook postCheck
          '';
        };
        
    in flake-utils.lib.eachDefaultSystem (system: {
      packages = {
        # Build native binary for current system
        cuenv = buildCuenvForTarget system system targetSystems.${system}.rust;
        
        # Cross-compile for all targets from current system
      } // (builtins.listToAttrs (map (target: {
        name = "cuenv-${target}";
        value = buildCuenvForTarget system targetSystems.${target}.nix targetSystems.${target}.rust;
      }) (builtins.attrNames targetSystems)));
    });
}