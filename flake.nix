{
  description = "cuenv - A direnv alternative that uses CUE files for environment configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Platform-specific dependencies
        platformDeps = with pkgs; 
          if stdenv.isDarwin then [
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.CoreFoundation
          ] else if stdenv.isLinux then [
            glibc
          ] else [];

        # Build dependencies
        buildInputs = with pkgs; [
          go_1_24
          openssl
          pkg-config
        ] ++ platformDeps;

        # Native build dependencies
        nativeBuildInputs = with pkgs; [
          rust-bin.stable."1.88.0".default
          pkg-config
          gcc
          go_1_24
        ];

        # Development tools
        devTools = with pkgs; [
          rust-analyzer
          cargo-watch
          cargo-edit
          cargo-outdated
          cargo-audit
          gopls
          gotools
        ];

        # Vendor Go dependencies
        goVendor = pkgs.stdenv.mkDerivation {
          pname = "cuenv-go-vendor";
          version = "0.1.0";
          src = ./libcue-bridge;
          
          nativeBuildInputs = [ pkgs.go_1_24 ];
          
          buildPhase = ''
            export HOME=$(mktemp -d)
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/go-cache"
            
            go mod vendor
          '';
          
          installPhase = ''
            mkdir -p $out
            cp -r vendor $out/
          '';
          
          outputHashMode = "recursive";
          outputHashAlgo = "sha256";
          outputHash = "sha256-1/rEFyvRPwXFX2J6L4JMr3whD/Vnq0L++Gz5DbKrG0o=";
        };

        cuenv = pkgs.rustPlatform.buildRustPackage {
          pname = "cuenv";
          version = "0.1.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Set up build environment
          preBuild = ''
            export HOME=$(mktemp -d)
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/go-cache"
            export CGO_ENABLED=1
            
            # Copy vendored dependencies
            cp -r ${goVendor}/vendor libcue-bridge/
            chmod -R u+w libcue-bridge
          '';

          inherit buildInputs nativeBuildInputs;

          # Platform-specific linker flags
          RUSTFLAGS = if pkgs.stdenv.isDarwin then
            "-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation"
          else
            "";

          # Ensure Go is available during build
          CGO_ENABLED = "1";
          GO = "${pkgs.go_1_24}/bin/go";

          checkPhase = ''
            runHook preCheck
            # Skip tests that require network access or special setup
            cargo test --offline
            runHook postCheck
          '';

          meta = with pkgs.lib; {
            description = "A direnv alternative that uses CUE files for environment configuration";
            homepage = "https://github.com/korora-tech/cuenv";
            license = licenses.mit;
            maintainers = [];
          };
        };

      in
      {
        packages = {
          default = cuenv;
          cuenv = cuenv;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = buildInputs ++ devTools;
          
          shellHook = ''
            echo "cuenv development environment"
            echo "Rust version: $(rustc --version)"
            echo "Go version: $(go version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo test     - Run tests"
            echo "  cargo run      - Run cuenv"
            echo "  cargo watch    - Watch for changes and rebuild"
            echo ""
            
            # Set up environment for building
            export CGO_ENABLED=1
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/.cache/go-build"
            
            # Platform-specific setup
            ${if pkgs.stdenv.isDarwin then ''
              export RUSTFLAGS="-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation"
            '' else ""}
          '';
        };
      });
}
