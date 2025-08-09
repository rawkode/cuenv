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
    (flake-utils.lib.eachDefaultSystem (
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
          prettier

          protobuf
          grpcurl
          netcat
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          # cargo-llvm-cov only works on Linux
          cargo-llvm-cov
        ];

        # Read version from Cargo.toml
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.workspace.package.version;

        # Vendor Go dependencies
        goVendor = pkgs.stdenv.mkDerivation {
          pname = "cuenv-go-vendor";
          version = version;
          src = ./crates/libcue-ffi-bridge;

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

            # Setup CUE root to use bundled schemas, avoiding external fetching
            export CUE_ROOT="$PWD/cue"

            # Copy vendored dependencies
            cp -r ${goVendor}/vendor crates/libcue-ffi-bridge/
            chmod -R u+w crates/libcue-ffi-bridge
          '';

          inherit buildInputs nativeBuildInputs;

          # Platform-specific linker flags
          RUSTFLAGS =
            if pkgs.stdenv.isDarwin then
              "-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation -C link-arg=-framework -C link-arg=SystemConfiguration"
            else
              "";

          # Ensure Go is available during build
          CGO_ENABLED = "1";
          GO = "${pkgs.go_1_24}/bin/go";

          # Disable tests during build phase - we'll run them separately
          doCheck = false;

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

        # Comprehensive checks for nix flake check
        checks =
          let
            # Vendor cargo dependencies so checks can run offline
            cargoVendor = pkgs.rustPlatform.fetchCargoVendor {
              src = ./.;
              name = "cuenv-cargo-vendor";
              hash = "sha256-b9BVkdzBk8/fqlb9DWLqoHxb636HM97gIqGG7wX3Hmg=";
            };

            # Common preBuild steps for checks
            checksPreBuild = ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CGO_ENABLED=1

              # Setup CUE root to use bundled schemas, avoiding external fetching
              export CUE_ROOT="$PWD/cue"

              # Copy Go vendor
              cp -r ${goVendor}/vendor crates/libcue-ffi-bridge/

              # Platform-specific setup for macOS framework linking
              ${
                if pkgs.stdenv.isDarwin then
                  ''
                    export RUSTFLAGS="-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation -C link-arg=-framework -C link-arg=SystemConfiguration"
                  ''
                else
                  ""
              }
              chmod -R u+w crates/libcue-ffi-bridge

              # Setup cargo vendor for offline build
              mkdir -p .cargo
              cat > .cargo/config.toml <<EOF
              [source.crates-io]
              replace-with = "vendored-sources"

              [source.vendored-sources]
              directory = "vendor"
              EOF

              cp -r ${cargoVendor} vendor
              chmod -R u+w vendor
            '';
          in
          {
            # Formatting check
            formatting = treefmt.config.build.check self;

            # Build check - ensure the package builds (without tests)
            build = cuenv;

            # Clippy check
            clippy = pkgs.stdenv.mkDerivation {
              pname = "cuenv-clippy-check";
              version = version;
              src = ./.;

              nativeBuildInputs = nativeBuildInputs;
              buildInputs = buildInputs;

              preBuild = checksPreBuild;

              buildPhase = ''
                runHook preBuild
                cargo clippy --all-targets --all-features -- -D warnings \
                  -A clippy::duplicate_mod \
                  -A clippy::uninlined_format_args \
                  -A clippy::too_many_arguments \
                  -A clippy::new_without_default \
                  -A clippy::ptr_arg \
                  -A clippy::needless_borrows_for_generic_args \
                  -A clippy::io_other_error \
                  -A clippy::manual_strip \
                  -A clippy::collapsible_if \
                  -A clippy::derivable_impls \
                  -A clippy::missing_safety_doc \
                  -A clippy::field_reassign_with_default \
                  -A clippy::manual_map \
                  -A clippy::not_unsafe_ptr_arg_deref \
                  -A clippy::question_mark \
                  -A clippy::needless_borrow \
                  -A clippy::await_holding_lock \
                  -A clippy::type_complexity \
                  -A clippy::enum_variant_names
                runHook postBuild
              '';

              installPhase = "touch $out";
            };

            # Unit tests (sandbox-compatible)
            unit-tests = pkgs.stdenv.mkDerivation {
              pname = "cuenv-unit-tests";
              version = version;
              src = ./.;

              nativeBuildInputs = nativeBuildInputs ++ [ pkgs.cargo-nextest ];
              buildInputs = buildInputs;

              preBuild = checksPreBuild;

              buildPhase = ''
                export RUST_TEST_THREADS=2
                export GOMAXPROCS=2
                runHook preBuild
                # Run only unit tests that work in sandbox
                cargo nextest run --lib --bins \
                  -E 'not test(/concurrent|thread_safe|monitored_cache|profiling|tree_operations|confidence|sequential_pattern|streaming|prop_test_cache|statistics|parse_shell|process_guard/)'
                runHook postBuild
              '';

              installPhase = "touch $out";
            };

            # Integration tests (requires network, runs in CI only)
            integration-tests = pkgs.stdenv.mkDerivation {
              pname = "cuenv-integration-tests";
              version = version;
              src = ./.;

              nativeBuildInputs = nativeBuildInputs ++ [ pkgs.cargo-nextest ];
              buildInputs = buildInputs;

              preBuild = ''
                export HOME=$(mktemp -d)
                export GOPATH="$HOME/go"
                export GOCACHE="$HOME/go-cache"
                export CGO_ENABLED=1
                export RUST_TEST_THREADS=4
                export GOMAXPROCS=4

                # Setup CUE root to use bundled schemas, avoiding external fetching
                export CUE_ROOT="$PWD/cue"

                cp -r ${goVendor}/vendor crates/libcue-ffi-bridge/
                chmod -R u+w crates/libcue-ffi-bridge
              '';

              buildPhase = ''
                runHook preBuild
                # This will fail in sandbox but documents what tests need network
                echo "Integration tests require network access and should be run in CI"
                echo "Would run: cargo nextest run --tests"
                runHook postBuild
              '';

              installPhase = "touch $out";
            };
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
    ))
    // {
      # Home Manager module (using standard flake schema to avoid warnings)
      homeManagerModule =
        { config
        , lib
        , pkgs
        , ...
        }:
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
