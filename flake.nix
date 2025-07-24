{
  description = "cuenv - A direnv alternative that uses CUE files for environment configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , rust-overlay
    , flake-utils
    , treefmt-nix
    ,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Platform-specific dependencies
        platformDeps =
          with pkgs;
          if stdenv.isDarwin then
            [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.CoreFoundation
            ]
          else if stdenv.isLinux then
            [
              glibc
            ]
          else
            [ ];

        # Build dependencies
        buildInputs =
          with pkgs;
          [
            go_1_24
            openssl
            pkg-config
          ]
          ++ platformDeps;

        # Native build dependencies
        nativeBuildInputs = with pkgs; [
          rust-bin.stable."1.88.0".default
          pkg-config
          gcc
          go_1_24
        ];

        # treefmt configuration
        treefmt = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            # Nix formatter
            nixpkgs-fmt.enable = true;

            cue.enable = true;

            # Rust formatter
            rustfmt = {
              enable = true;
              # Use same edition as Cargo.toml
              edition = "2021";
            };

            # Go formatter
            gofmt.enable = true;

            # YAML formatter
            yamlfmt.enable = true;

            # Prettier formatter - handles markdown with frontmatter support
            prettier = {
              enable = true;
              # Only format markdown files with prettier
              includes = [
                "*.md"
                "*.mdx"
              ];
            };

            # TOML formatter
            taplo.enable = true;
          };
        };

        # Development tools
        devTools = with pkgs; [
          cargo-watch
          cargo-edit
          cargo-outdated
          cargo-audit
          cue
          gopls
          gotools
          rust-analyzer
          treefmt.config.build.wrapper
        ];

        # Read version from Cargo.toml
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.package.version;

        # Vendor Go dependencies
        goVendor = pkgs.stdenv.mkDerivation {
          pname = "cuenv-go-vendor";
          version = version;
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
          outputHash = "sha256-l/urNOAK9q5nflBt2ovfbaM3WNCn0ouZlc9RJq/+eKk=";
        };

        cuenv = pkgs.rustPlatform.buildRustPackage {
          pname = "cuenv";
          version = version;

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
          RUSTFLAGS =
            if pkgs.stdenv.isDarwin then
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
            homepage = "https://github.com/rawkode/cuenv";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

      in
      {
        packages = {
          default = cuenv;
          cuenv = cuenv;
        };

        # Make treefmt available as a check
        checks = {
          formatting = treefmt.config.build.check self;

          # Run clippy
          clippy = cuenv.overrideAttrs (oldAttrs: {
            name = "cuenv-clippy";
            buildPhase = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CGO_ENABLED=1

              # Copy vendored dependencies
              cp -r ${goVendor}/vendor libcue-bridge/
              chmod -R u+w libcue-bridge

              cargo clippy --offline -- -D warnings
            '';
            installPhase = ''
              mkdir -p $out
              touch $out/clippy-passed
            '';
          });

          # Run tests
          tests = cuenv.overrideAttrs (oldAttrs: {
            name = "cuenv-tests";
            doCheck = true;
            checkPhase = ''
              runHook preCheck
              cargo test --offline
              runHook postCheck
            '';
            installPhase = ''
              mkdir -p $out
              touch $out/tests-passed
            '';
          });
        };

        # Make formatter available
        formatter = treefmt.config.build.wrapper;

        devShells.default = pkgs.mkShell {
          buildInputs = buildInputs ++ nativeBuildInputs ++ devTools;

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
            echo "  treefmt        - Format all code"
            echo "  nix flake check - Check code formatting"
            echo ""

            # Set up environment for building
            export CGO_ENABLED=1
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/.cache/go-build"

            # Platform-specific setup
            ${
              if pkgs.stdenv.isDarwin then
                ''
                  export RUSTFLAGS="-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation"
                ''
              else
                ""
            }
          '';
        };
      }
    );
}
