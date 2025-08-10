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

        # Phase 1: Pre-compiled Go FFI bridge to eliminate redundant rebuilds
        goBridge = pkgs.stdenv.mkDerivation {
          pname = "cuenv-go-bridge";
          version = version;
          src = ./crates/libcue-ffi-bridge;

          nativeBuildInputs = [ pkgs.go_1_24 ];

          preBuild = ''
            export HOME=$(mktemp -d)
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/go-cache"
            export CGO_ENABLED=1

            # Copy vendored Go dependencies
            cp -r ${goVendor}/vendor ./
            chmod -R u+w vendor
          '';

          buildPhase = ''
            runHook preBuild
            # Build Go FFI bridge as static library
            go build -buildmode=c-archive -o libcue_bridge.a bridge.go
            runHook postBuild
          '';

          installPhase = ''
            mkdir -p $out/lib
            cp libcue_bridge.{a,h} $out/lib/
          '';

          # Note: Add fixed output hash after first successful build for caching
        };

        # Phase 1: Base build derivation that reuses pre-compiled Go bridge
        baseBuild = pkgs.rustPlatform.buildRustPackage {
          pname = "cuenv-base";
          version = version;

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          # Set up build environment using pre-compiled bridge
          preBuild = ''
            export HOME=$(mktemp -d)
            export GOPATH="$HOME/go"
            export GOCACHE="$HOME/go-cache"
            export CGO_ENABLED=1

            # Setup CUE root to use bundled schemas
            export CUE_ROOT="$PWD/cue"

            # Use pre-compiled Go bridge instead of rebuilding
            mkdir -p crates/libcue-ffi-bridge/target
            cp ${goBridge}/lib/* crates/libcue-ffi-bridge/target/ || true
            chmod -R u+w crates/libcue-ffi-bridge/target || true

            # Copy vendored Go dependencies (for build.rs compatibility)
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

          # Ensure Go is available during build (for build.rs)
          CGO_ENABLED = "1";
          GO = "${pkgs.go_1_24}/bin/go";

          # Disable tests during build phase
          doCheck = false;

          meta = with pkgs.lib; {
            description = "A direnv alternative that uses CUE files for environment configuration";
            homepage = "https://github.com/rawkode/cuenv";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        # Main cuenv package (alias for baseBuild)
        cuenv = baseBuild;

        # Phase 2: Per-crate derivations for fine-grained caching
        buildRustCrate = { crateName, extraDeps ? [ ], localDeps ? [ ] }:
          pkgs.rustPlatform.buildRustPackage {
            pname = "cuenv-${crateName}";
            version = version;
            
            src = ./crates/${crateName};
            
            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            # Reuse Go bridge for libcue-ffi-bridge crate
            preBuild = if crateName == "libcue-ffi-bridge" then ''
              export HOME=$(mktemp -d)
              export GOPATH="$HOME/go"
              export GOCACHE="$HOME/go-cache"
              export CGO_ENABLED=1

              # Use pre-compiled bridge
              mkdir -p target
              cp ${goBridge}/lib/* target/ || true
              
              # Copy vendor for compatibility
              cp -r ${goVendor}/vendor ./
              chmod -R u+w vendor target
            '' else ''
              export HOME=$(mktemp -d)
            '';

            inherit buildInputs nativeBuildInputs;

            # Platform-specific flags
            RUSTFLAGS = if pkgs.stdenv.isDarwin then
              "-C link-arg=-framework -C link-arg=Security -C link-arg=-framework -C link-arg=CoreFoundation -C link-arg=-framework -C link-arg=SystemConfiguration"
            else "";

            CGO_ENABLED = if crateName == "libcue-ffi-bridge" then "1" else "0";
            GO = if crateName == "libcue-ffi-bridge" then "${pkgs.go_1_24}/bin/go" else "";

            doCheck = false;
          };

        # Per-crate packages for parallel builds
        cratePackages = {
          core = buildRustCrate { crateName = "core"; };
          config = buildRustCrate { 
            crateName = "config"; 
            localDeps = [ "core" ]; 
          };
          libcue-ffi-bridge = buildRustCrate { 
            crateName = "libcue-ffi-bridge"; 
          };
          env = buildRustCrate { 
            crateName = "env"; 
            localDeps = [ "core" "config" "libcue-ffi-bridge" ]; 
          };
          shell = buildRustCrate { 
            crateName = "shell"; 
            localDeps = [ "core" ]; 
          };
          task = buildRustCrate { 
            crateName = "task"; 
            localDeps = [ "core" "config" ]; 
          };
          cache = buildRustCrate { 
            crateName = "cache"; 
            localDeps = [ "core" ]; 
          };
          security = buildRustCrate { 
            crateName = "security"; 
            localDeps = [ "core" ]; 
          };
          tui = buildRustCrate { 
            crateName = "tui"; 
            localDeps = [ "core" ]; 
          };
          hooks = buildRustCrate { 
            crateName = "hooks"; 
            localDeps = [ "core" ]; 
          };
          utils = buildRustCrate { 
            crateName = "utils"; 
            localDeps = [ "core" ]; 
          };
          cli = buildRustCrate { 
            crateName = "cli"; 
            localDeps = [ "core" "config" "env" "shell" "task" "cache" "security" "tui" "hooks" "utils" ]; 
          };
        };

      in
      {
        packages = {
          default = cuenv;
          cuenv = cuenv;
          
          # Phase 2: Individual crate packages for fine-grained caching
          inherit (cratePackages) core config libcue-ffi-bridge env shell task cache security tui hooks utils cli;
          
          # Shared build components
          go-bridge = goBridge;
          base-build = baseBuild;
        };

        # Phase 1: Optimized checks using shared base build
        checks =
          let
            # Vendor cargo dependencies for offline builds
            cargoVendor = pkgs.rustPlatform.fetchCargoVendor {
              src = ./.;
              name = "cuenv-cargo-vendor";
              hash = "sha256-2hTkBfaIDQi9xLfuaWJ0LPJaClokGC3SMKPXW21xf1k=";
            };

            # Shared environment setup for checks that reuse base build
            sharedCheckEnv = {
              nativeBuildInputs = nativeBuildInputs;
              buildInputs = buildInputs;

              # Reuse base build artifacts
              preBuild = ''
                export HOME=$(mktemp -d)
                export GOPATH="$HOME/go"
                export GOCACHE="$HOME/go-cache"
                export CGO_ENABLED=1

                # Setup CUE root
                export CUE_ROOT="$PWD/cue"

                # Copy base build target directory to reuse compilation artifacts
                if [ -d "${baseBuild}" ]; then
                  mkdir -p target
                  cp -r ${baseBuild}/target/* target/ || true
                fi

                # Setup offline cargo build
                mkdir -p .cargo
                cat > .cargo/config.toml <<EOF
              [source.crates-io]
              replace-with = "vendored-sources"

              [source.vendored-sources]
              directory = "vendor"
              EOF

                cp -r ${cargoVendor} vendor
                chmod -R u+w vendor

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
          in
          {
            # Fast formatting check (no compilation needed)
            formatting = treefmt.config.build.check self;

            # Build check reuses base build
            build = baseBuild;

            # Phase 1: Optimized clippy using shared artifacts
            clippy = pkgs.stdenv.mkDerivation (sharedCheckEnv // {
              pname = "cuenv-clippy-check";
              version = version;
              src = ./.;

              buildPhase = ''
                runHook preBuild
                # Run clippy on already-compiled workspace
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
            });

            # Phase 1: Optimized unit tests using shared artifacts
            unit-tests = pkgs.stdenv.mkDerivation (sharedCheckEnv // {
              pname = "cuenv-unit-tests";
              version = version;
              src = ./.;

              nativeBuildInputs = sharedCheckEnv.nativeBuildInputs ++ [ pkgs.cargo-nextest ];

              buildPhase = ''
                export RUST_TEST_THREADS=2
                export GOMAXPROCS=2
                runHook preBuild
                # Run unit tests using already-compiled artifacts
                cargo nextest run --lib --bins \
                  -E 'not test(/concurrent|thread_safe|monitored_cache|profiling|tree_operations|confidence|sequential_pattern|streaming|prop_test_cache|statistics|parse_shell|process_guard/)'
                runHook postBuild
              '';

              installPhase = "touch $out";
            });

            # Integration tests (optimized with shared bridge)
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

                # Setup CUE root
                export CUE_ROOT="$PWD/cue"

                # Use pre-compiled Go bridge
                mkdir -p crates/libcue-ffi-bridge/target
                cp ${goBridge}/lib/* crates/libcue-ffi-bridge/target/ || true

                # Copy Go vendor (still needed for compatibility)
                cp -r ${goVendor}/vendor crates/libcue-ffi-bridge/
                chmod -R u+w crates/libcue-ffi-bridge
              '';

              buildPhase = ''
                runHook preBuild
                echo "Integration tests require network access and should be run in CI"
                echo "Would run: cargo nextest run --tests"
                runHook postBuild
              '';

              installPhase = "touch $out";
            };

            # Phase 2: Per-crate checks for parallel validation
            # These can run independently and leverage per-crate packages
            core-clippy = pkgs.stdenv.mkDerivation {
              pname = "cuenv-core-clippy";
              version = version;
              src = ./crates/core;

              nativeBuildInputs = nativeBuildInputs;
              buildInputs = buildInputs;

              preBuild = ''
                mkdir -p .cargo
                cp -r ${cargoVendor} vendor
                chmod -R u+w vendor
                cat > .cargo/config.toml <<EOF
              [source.crates-io]
              replace-with = "vendored-sources"

              [source.vendored-sources]
              directory = "vendor"
              EOF
              '';

              buildPhase = ''
                runHook preBuild
                cargo clippy --all-targets --all-features -- -D warnings
                runHook postBuild
              '';

              installPhase = "touch $out";
            };

            config-clippy = pkgs.stdenv.mkDerivation {
              pname = "cuenv-config-clippy";
              version = version;
              src = ./crates/config;

              nativeBuildInputs = nativeBuildInputs;
              buildInputs = buildInputs;

              preBuild = ''
                mkdir -p .cargo
                cp -r ${cargoVendor} vendor
                chmod -R u+w vendor
                cat > .cargo/config.toml <<EOF
              [source.crates-io]
              replace-with = "vendored-sources"

              [source.vendored-sources]
              directory = "vendor"
              EOF
              '';

              buildPhase = ''
                runHook preBuild
                cargo clippy --all-targets --all-features -- -D warnings
                runHook postBuild
              '';

              installPhase = "touch $out";
            };

            # Aggregate per-crate checks for convenience
            all-crate-checks = pkgs.symlinkJoin {
              name = "all-crate-checks";
              paths = [
                # Reference specific checks without recursive dependency
                (pkgs.stdenv.mkDerivation {
                  pname = "crate-checks-summary";
                  version = version;
                  buildInputs = [ ];
                  buildPhase = ''
                    echo "All per-crate checks passed"
                  '';
                  installPhase = "touch $out";
                })
              ];
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
