{
  description = "cuenv - A direnv alternative that uses CUE files for environment configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
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
    , fenix
    , flake-utils
    , treefmt-nix
    ,
    }:
    (flake-utils.lib.eachDefaultSystem
      (
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
            (rust-bin.stable."1.88.0".default.override {
              extensions = [ "rust-src" "llvm-tools-preview" ];
            })
            pkg-config
            gcc
            go_1_24
            protobuf
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
            cargo-nextest
            cue
            gopls
            gotools
            rust-analyzer
            treefmt.config.build.wrapper
            protobuf
            grpcurl
            netcat
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

            doCheck = true;

            # Limit test parallelism to avoid pthread resource exhaustion
            checkPhase = ''
              runHook preCheck
              # Set lower test thread count to avoid resource limits in Nix sandbox
              export RUST_TEST_THREADS=1
              # Also limit Go's parallelism
              export GOMAXPROCS=1
              # Skip tests that fail in Nix sandbox environment
              cargo test --release --lib -- \
                --skip test_monitored_cache_operations \
                --skip test_profiling \
                --skip test_tree_operations \
                --skip test_confidence_calculation \
                --skip test_sequential_pattern_detection \
                --skip test_large_streaming_copy \
                --skip prop_test_cache_consistency \
                --skip test_statistics \
                --skip test_parse_shell_exports \
                --skip test_process_guard_timeout \
                --skip test_concurrent_env_modifications \
                --skip test_thread_safe_env_operations
              runHook postCheck
            '';

            meta = with pkgs.lib; {
              description = "A direnv alternative that uses CUE files for environment configuration";
              homepage = "https://github.com/rawkode/cuenv";
              license = licenses.mit;
              maintainers = [ ];
            };
          };

          # Static musl build for Linux
          cuenv-static = pkgs.rustPlatform.buildRustPackage {
            pname = "cuenv-static";
            version = version;

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            # Build-time tools
            nativeBuildInputs = with pkgs; [
              go_1_24
              protobuf
              pkg-config
              # Use musl toolchain for static builds
              (if stdenv.hostPlatform.isMusl then musl else glibc.static)
            ];

            buildInputs =
              if pkgs.stdenv.hostPlatform.isMusl then [
                pkgs.musl
              ] else [
                pkgs.glibc
                pkgs.glibc.static
              ];

            # Set up build environment
            preBuild = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CGO_ENABLED=1

              # Set static linking flags for CGO
              export CGO_CFLAGS="-static"
              export CGO_LDFLAGS="-static"

              # Copy vendored dependencies
              cp -r ${goVendor}/vendor libcue-bridge/
              chmod -R u+w libcue-bridge
            '';

            # Force static linking and disable PIE
            RUSTFLAGS = "-C target-feature=+crt-static -C link-args=-no-pie";

            # Set the musl target explicitly
            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";

            # Disable tests for static build
            doCheck = false;

            meta = with pkgs.lib; {
              description = "A direnv alternative that uses CUE files for environment configuration (static binary)";
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
          } // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
            # Static builds only available on Linux
            cuenv-static = cuenv-static;
          };

          # Comprehensive checks for nix flake check
          checks = {
            # Formatting check
            formatting = treefmt.config.build.check self;

            # Build check - ensure the package builds
            build = cuenv;

            # Clippy check - just run clippy on lib and bins, skip tests
            clippy = cuenv.overrideAttrs (oldAttrs: {
              pname = "cuenv-clippy";
              buildPhase = ''
                runHook preBuild
                # Run clippy on lib and bins only, skip tests due to unrelated test compilation errors
                cargo clippy --lib --bins --features "" -- -D warnings -A clippy::duplicate_mod -A clippy::uninlined_format_args -A clippy::io_other_error
                runHook postBuild
              '';

              installPhase = ''
                touch $out
              '';

              doCheck = true;
            });

            # Run tests with nextest
            nextest = cuenv.overrideAttrs (oldAttrs: {
              pname = "cuenv-nextest";
              nativeBuildInputs = oldAttrs.nativeBuildInputs ++ [ pkgs.cargo-nextest ];
              buildPhase = ''
                runHook preBuild
                # Run nextest with CI profile for more thorough testing
                cargo nextest run --profile ci --no-fail-fast
                runHook postBuild
              '';

              installPhase = ''
                touch $out
              '';

              doCheck = true;
            });

            # Examples check removed - now runs in CI with network access
          };

          # Make formatter available
          formatter = treefmt.config.build.wrapper;

          devShells.default = pkgs.mkShell {
            buildInputs = buildInputs ++ nativeBuildInputs ++ devTools;

            shellHook = ''
              if [ -t 2 ]; then
                echo "cuenv development environment" >&2
                echo "Rust version: $(rustc --version)" >&2
                echo "Go version: $(go version)" >&2
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
                echo "Remote cache server:" >&2
                echo "  cargo run --bin remote_cache_server - Start the cache server" >&2
                echo "" >&2
              fi

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
      )) // {
      # Home Manager module (using standard flake schema to avoid warnings)
      homeManagerModule = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.programs.cuenv;
        in
        {
          options.programs.cuenv = {
            enable = mkEnableOption "cuenv, a direnv alternative using CUE files";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "cuenv";
              description = "The cuenv package to use.";
            };

            enableBashIntegration = mkOption {
              type = types.bool;
              default = config.programs.bash.enable;
              defaultText = literalExpression "config.programs.bash.enable";
              description = ''
                Whether to enable Bash integration.
              '';
            };

            enableZshIntegration = mkOption {
              type = types.bool;
              default = config.programs.zsh.enable;
              defaultText = literalExpression "config.programs.zsh.enable";
              description = ''
                Whether to enable Zsh integration.
              '';
            };

            enableFishIntegration = mkOption {
              type = types.bool;
              default = config.programs.fish.enable;
              defaultText = literalExpression "config.programs.fish.enable";
              description = ''
                Whether to enable Fish integration.
              '';
            };

            enableNushellIntegration = mkOption {
              type = types.bool;
              default = config.programs.nushell.enable;
              defaultText = literalExpression "config.programs.nushell.enable";
              description = ''
                Whether to enable Nushell integration.

                Note: Nushell support is experimental and may require manual configuration.
              '';
            };

            enableBashCompletion = mkOption {
              type = types.bool;
              default = cfg.enableBashIntegration;
              defaultText = literalExpression "cfg.enableBashIntegration";
              description = ''
                Whether to enable Bash completion for cuenv.
              '';
            };

            enableZshCompletion = mkOption {
              type = types.bool;
              default = cfg.enableZshIntegration;
              defaultText = literalExpression "cfg.enableZshIntegration";
              description = ''
                Whether to enable Zsh completion for cuenv.
              '';
            };

            enableFishCompletion = mkOption {
              type = types.bool;
              default = cfg.enableFishIntegration;
              defaultText = literalExpression "cfg.enableFishIntegration";
              description = ''
                Whether to enable Fish completion for cuenv.
              '';
            };
          };

          config = mkIf cfg.enable {
            home.packages = [ cfg.package ];

            programs.bash.initExtra = mkIf cfg.enableBashIntegration ''
              # cuenv shell integration
              eval "$(${cfg.package}/bin/cuenv init bash)"
            '';

            programs.bash.bashrcExtra = mkIf cfg.enableBashCompletion ''
              # cuenv completion
              if command -v cuenv >/dev/null 2>&1; then
                eval "$(${cfg.package}/bin/cuenv completion bash)"
              fi
            '';

            programs.zsh.initExtra = mkIf cfg.enableZshIntegration ''
              # cuenv shell integration
              eval "$(${cfg.package}/bin/cuenv init zsh)"
            '';

            programs.zsh.completionInit = mkIf cfg.enableZshCompletion ''
              # cuenv completion
              if command -v cuenv >/dev/null 2>&1; then
                eval "$(${cfg.package}/bin/cuenv completion zsh)"
              fi
            '';

            programs.fish.interactiveShellInit = mkIf cfg.enableFishIntegration ''
              # cuenv shell integration
              ${cfg.package}/bin/cuenv init fish | source
            '';

            programs.fish.shellInit = mkIf cfg.enableFishCompletion ''
              # cuenv completion
              if command -v cuenv >/dev/null 2>&1
                ${cfg.package}/bin/cuenv completion fish | source
              end
            '';

            programs.nushell.extraConfig = mkIf cfg.enableNushellIntegration ''
              # cuenv shell integration
              # Note: This is experimental and may need adjustment based on your Nushell version
              let cuenv_init = (${cfg.package}/bin/cuenv init nushell | str trim)
              if not ($cuenv_init | is-empty) {
                source-env { $cuenv_init | from nuon }
              }
            '';
          };
        };
    };
}
