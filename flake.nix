{
  description = "cuenv - A direnv alternative that uses CUE files for environment configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , crane
    , fenix
    , flake-utils
    , rust-overlay
    , treefmt-nix
    ,
    }:
    {
      # Home Manager module for cuenv
      homeManagerModules.default = { config, pkgs, lib, ... }:
        let
          cfg = config.programs.cuenv;
          inherit (lib) mkIf mkOption types;
        in
        {
          meta.maintainers = with lib.maintainers; [ ];

          options.programs.cuenv = {
            enable = lib.mkEnableOption "cuenv - A direnv alternative that uses CUE files";

            package = lib.mkPackageOption pkgs "cuenv" { };

            enableBashIntegration = lib.hm.shell.mkBashIntegrationOption {
              inherit config;
              extraDescription = "Enables cuenv shell hooks for Bash.";
            };

            enableZshIntegration = lib.hm.shell.mkZshIntegrationOption {
              inherit config;
              extraDescription = "Enables cuenv shell hooks for Zsh.";
            };

            enableFishIntegration = lib.hm.shell.mkFishIntegrationOption {
              inherit config;
              extraDescription = "Enables cuenv shell hooks for Fish.";
            };

            enableNushellIntegration = lib.hm.shell.mkNushellIntegrationOption {
              inherit config;
              extraDescription = "Enables cuenv shell hooks for Nushell.";
            };

          };

          config = mkIf cfg.enable (lib.mkMerge [
            {
              # Always add the configured cuenv package
              home.packages = [ cfg.package ];

              # Shell integrations
              programs.bash.initExtra = mkIf cfg.enableBashIntegration ''
                if [[ -n "''${BASH_VERSION:-}" ]]; then
                  eval "$(${lib.getExe cfg.package} shell init bash)"
                fi
              '';

              programs.zsh.initExtra = mkIf cfg.enableZshIntegration ''
                if [[ -n "''${ZSH_VERSION:-}" ]]; then
                  eval "$(${lib.getExe cfg.package} shell init zsh)"
                fi
              '';

              programs.fish.interactiveShellInit = mkIf cfg.enableFishIntegration ''
                ${lib.getExe cfg.package} shell init fish | source
              '';

              programs.nushell = mkIf cfg.enableNushellIntegration {
                extraConfig = ''
                  source ${
                    pkgs.runCommand "cuenv-nushell-config.nu"
                      {
                        nativeBuildInputs = [ cfg.package ];
                      }
                      ''
                        ${lib.getExe cfg.package} shell init nushell > "$out"
                      ''
                  }
                '';
              };
            }
          ]);
        };
    }
    // (flake-utils.lib.eachDefaultSystem (
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
              darwin.apple_sdk.frameworks.SystemConfiguration
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
          (rust-bin.stable."1.88.0".default.override {
            extensions = [
              "rust-src"
              "llvm-tools-preview"
            ];
          })
          pkg-config
          gcc
          go_1_24
          protobuf
        ];

        # Crane library for advanced Rust builds
        craneLib = crane.mkLib pkgs;

        # treefmt configuration
        treefmt = treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs = {
            # Rust formatter
            rustfmt = {
              enable = true;
              # Use consistent edition across all files
              edition = "2021";
            };

            # Nix formatter
            nixpkgs-fmt.enable = true;

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
        devTools =
          with pkgs;
          [
            cargo-watch
            cargo-edit
            cargo-outdated
            cargo-audit
            cargo-nextest
            cue
            gopls
            gotools
            nodejs
            rust-analyzer
            treefmt.config.build.wrapper
            prettier

            protobuf
            grpcurl
            netcat
            jq

            # Shells for testing
            bash
            zsh
            fish
            nushell
            elvish
            dash
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            # cargo-llvm-cov only works on Linux
            cargo-llvm-cov
          ];

        # Shells available for testing
        testShells = with pkgs; {
          bash = bash;
          zsh = zsh;
          fish = fish;
          nushell = nushell;
          elvish = elvish;
          dash = dash;
        };

        # Read version from Cargo.toml
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.workspace.package.version;

        # Pre-compiled Go FFI bridge using buildGoModule - eliminates manual hash management
        goBridge = pkgs.buildGoModule {
          pname = "cuenv-go-bridge";
          version = version;
          src = ./crates/libcue-ffi-bridge;

          vendorHash = "sha256-mU40RCeO0R286fxfgONJ7kw6kFDHPMUzHw8sjsBgiRg=";

          nativeBuildInputs = [ pkgs.go_1_24 ];

          # Build as C archive instead of regular Go binary
          buildPhase = ''
            export CGO_ENABLED=1
            go build -buildmode=c-archive -o libcue_bridge.a bridge.go
          '';

          installPhase = ''
            mkdir -p $out/lib
            cp libcue_bridge.{a,h} $out/lib/
          '';

          # Skip the default Go binary installation since we're building a C archive
          dontInstallBinaries = true;

          meta = with pkgs.lib; {
            description = "CUE FFI bridge for cuenv";
            license = licenses.mit;
          };
        };

        # Common build environment for all Crane builds
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          inherit buildInputs nativeBuildInputs;

          # Platform-specific linker flags
          RUSTFLAGS =
            if pkgs.stdenv.isDarwin then
              "-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation -C link-arg=-framework -C link-arg=SystemConfiguration"
            else
              "";

          # Common environment
          CGO_ENABLED = "1";
          GO = "${pkgs.go_1_24}/bin/go";
          CUE_ROOT = "$PWD/cue";

          meta = with pkgs.lib; {
            description = "A direnv alternative that uses CUE files for environment configuration";
            homepage = "https://github.com/rawkode/cuenv";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        # Shared clippy arguments - zero tolerance for warnings
        clippyArgs = "-- -D warnings";

        # Cargo dependencies for all crates - shared build artifacts
        cargoArtifacts = craneLib.buildDepsOnly (
          commonArgs
          // {
            pname = "cuenv-deps";
          }
        );

        # Pure Rust crates (no FFI bridge needed) - corrected names to match workspace
        pureRustCrates = [
          "cuenv-cache"
          "cuenv-cli"
          "cuenv-core"
          "cuenv-env"
          "cuenv-hooks"
          "cuenv-security"
          "cuenv-shell"
          "cuenv-task"
          "cuenv-tui"
          "cuenv-utils"
        ];

        # Build pure Rust crates efficiently (parallel, cached)
        pureRustPackages = builtins.listToAttrs (
          map
            (crateName: {
              name = crateName;
              value = craneLib.buildPackage (
                commonArgs
                // {
                  inherit cargoArtifacts;
                  pname = crateName;
                  cargoExtraArgs = "-p ${crateName}";

                  # No FFI bridge setup needed for pure Rust crates
                  preBuild = ''
                    export CUE_ROOT="$PWD/cue"
                  '';
                }
              );
            })
            pureRustCrates
        );

        # FFI-dependent crates (need the Go bridge)
        ffiBridge = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "cuenv-libcue-ffi-bridge";
            cargoExtraArgs = "-p cuenv-libcue-ffi-bridge";

            preBuild = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CUE_ROOT="$PWD/cue"

              # Use pre-compiled Go bridge - fail fast if copy fails
              mkdir -p crates/libcue-ffi-bridge/target/debug
              mkdir -p crates/libcue-ffi-bridge/target/release
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
            '';
          }
        );

        configCrate = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "cuenv-config";
            cargoExtraArgs = "-p cuenv-config";

            preBuild = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CUE_ROOT="$PWD/cue"

              # Use pre-compiled Go bridge - fail fast if copy fails
              mkdir -p crates/libcue-ffi-bridge/target/debug
              mkdir -p crates/libcue-ffi-bridge/target/release
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
            '';
          }
        );

        # Main cuenv binary - depends on all crates
        cuenv = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "cuenv";

            preBuild = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CUE_ROOT="$PWD/cue"

              # Use pre-compiled Go bridge - fail fast if copy fails
              mkdir -p crates/libcue-ffi-bridge/target/debug
              mkdir -p crates/libcue-ffi-bridge/target/release
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
              cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
            '';

            # Only build the main CLI binary
            cargoExtraArgs = "-p cuenv";
            doCheck = false;
          }
        );

      in
      {
        packages = {
          default = cuenv;
          cuenv = cuenv;

          # Individual crate packages for granular builds
          inherit ffiBridge configCrate;
          go-bridge = goBridge;
          cargo-deps = cargoArtifacts;
        }
        // pureRustPackages;


        # Optimized checks using Crane - massive performance improvement
        checks = {
          # Fast formatting check (no compilation needed)
          formatting = treefmt.config.build.check self;

          # Build check - reuse main cuenv package
          build = cuenv;

          # Clippy check using shared cargo artifacts
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets --all-features " + clippyArgs;

              preBuild = ''
                export HOME=$(mktemp -d)
                export GOPATH="$HOME/go"
                export GOCACHE="$HOME/go-cache"
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge - fail fast if copy fails
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );

          # Unit tests using shared cargo artifacts
          unit-tests = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestExtraArgs = "--lib --bins -E 'not test(/concurrent|thread_safe|monitored_cache|profiling|tree_operations|confidence|sequential_pattern|streaming|prop_test_cache|statistics|parse_shell|process_guard/)'";

              preBuild = ''
                export HOME=$(mktemp -d)
                export GOPATH="$HOME/go"
                export GOCACHE="$HOME/go-cache"
                export CUE_ROOT="$PWD/cue"
                export RUST_TEST_THREADS=2
                export GOMAXPROCS=2

                # Use pre-compiled Go bridge - fail fast if copy fails
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );

          # Integration tests - disabled in nix environment
          # These tests require access to example files and aren't suitable
          # for the sandboxed nix build environment. Use scripts/test-examples.sh
          # for manual testing instead.
          # integration-tests = craneLib.cargoNextest (
          #   commonArgs
          #   // {
          #     inherit cargoArtifacts;
          #     partitions = 1;
          #     partitionType = "count";
          #     cargoNextestExtraArgs = "--test test_examples";
          #
          #     preBuild = ''
          #       export HOME=$(mktemp -d)
          #       export GOPATH="$HOME/go"
          #       export GOCACHE="$HOME/go-cache"
          #       export CUE_ROOT="$PWD/cue"
          #       export RUST_TEST_THREADS=1
          #       export GOMAXPROCS=1
          #
          #       # Use pre-compiled Go bridge - fail fast if copy fails
          #       mkdir -p crates/libcue-ffi-bridge/target/debug
          #       mkdir -p crates/libcue-ffi-bridge/target/release
          #       cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
          #       cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
          #     '';
          #   }
          # );

          # Per-crate clippy checks for parallel validation (pure Rust crates only)
          clippy-core = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "cuenv-core-clippy";
              cargoExtraArgs = "-p cuenv-core";
              cargoClippyExtraArgs = clippyArgs;
              preBuild = ''
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge to avoid build failures
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );

          clippy-cache = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "cuenv-cache-clippy";
              cargoExtraArgs = "-p cuenv-cache";
              cargoClippyExtraArgs = clippyArgs;
              preBuild = ''
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge to avoid build failures
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );

          clippy-env = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "cuenv-env-clippy";
              cargoExtraArgs = "-p cuenv-env";
              cargoClippyExtraArgs = clippyArgs;
              preBuild = ''
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge to avoid build failures
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );

          clippy-utils = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              pname = "cuenv-utils-clippy";
              cargoExtraArgs = "-p cuenv-utils";
              cargoClippyExtraArgs = clippyArgs;
              preBuild = ''
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge to avoid build failures
                mkdir -p crates/libcue-ffi-bridge/target/debug
                mkdir -p crates/libcue-ffi-bridge/target/release
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/debug/
                cp -r ${goBridge}/lib/* crates/libcue-ffi-bridge/target/release/
              '';
            }
          );
        };

        # Make formatter available
        formatter = treefmt.config.build.wrapper;

        devShells.default = pkgs.mkShell {
          buildInputs = buildInputs ++ nativeBuildInputs ++ devTools;

          shellHook = ''
            # Setup CUE root to use bundled schemas, avoiding external fetching
            export CUE_ROOT="$PWD/cue"

            if [ -t 2 ]; then
              echo "cuenv development environment" >&2
              echo "Rust version: $(rustc --version)" >&2
              echo "Go version: $(go version)" >&2
              echo "CUE root: $CUE_ROOT" >&2
              echo "" >&2
              echo "Available commands:" >&2
              echo "  cargo build    - Build the project" >&2
              echo "  cargo test     - Run tests" >&2
              echo "  cargo nextest run - Run tests with nextest (faster)" >&2
              echo "  cargo run      - Run cuenv" >&2
              echo "  cargo watch    - Watch for changes and rebuild" >&2
              echo "  treefmt        - Format all code" >&2
              echo "  nix flake check - Check code formatting" >&2
              echo "" >&2
              echo "Individual crate builds (faster):" >&2
              echo "  nix build .#cuenv-core" >&2
              echo "  nix build .#cuenv-cache" >&2
              echo "  nix build .#cuenv-config" >&2
              echo "  nix build .#go-bridge" >&2
            fi

            # Set up environment for building
            export CGO_ENABLED=1
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/.cache/go-build"

            # Platform-specific setup
            ${
              if pkgs.stdenv.isDarwin then
                ''
                  export RUSTFLAGS="-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation -C link-arg=-framework -C link-arg=SystemConfiguration"
                ''
              else
                ""
            }
          '';
        };
      }
    ));
}
