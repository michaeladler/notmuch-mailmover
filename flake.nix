{
  description = "notmuch-mailmover";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      advisory-db,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            let
              pkgs = nixpkgs.legacyPackages.${system};

              src = pkgs.lib.fileset.toSource {
                root = ./.;
                fileset = pkgs.lib.fileset.unions [
                  ./src
                  ./lib
                  ./tests
                  ./Cargo.toml
                  ./Cargo.lock
                  ./deny.toml
                  ./share
                ];
              };
            in
            {
              inherit pkgs src;

              # Common arguments can be set here to avoid repeating them later
              commonArgs = {
                inherit src;
                strictDeps = true;

                nativeBuildInputs = [
                  pkgs.pkg-config
                  pkgs.installShellFiles
                ];

                nativeCheckInputs = [
                  pkgs.notmuch
                ];

                buildInputs = [
                  pkgs.notmuch
                  pkgs.lua5_4
                ];
              };
            }
          )
        );
    in
    {
      checks = forAllSystems (
        {
          pkgs,
          src,
          commonArgs,
        }:
        let
          craneLib = crane.mkLib pkgs;

          # Build *just* the cargo dependencies, so we can reuse
          # all of that work (e.g. via cachix) when running in CI
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Build the actual crate itself, reusing the dependency
          # artifacts from above.
          my-crate = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;

              # cachix needs cargoArtifacts pushed separately: buildDepsOnly output is a
              # build-time input, so it is not part of the package's runtime closure.
              passthru.cargoArtifacts = cargoArtifacts;

              nativeBuildInputs = commonArgs.nativeBuildInputs;

              nativeCheckInputs = commonArgs.nativeCheckInputs;

              postInstall = ''
                installManPage share/notmuch-mailmover.1
                installShellCompletion --cmd notmuch-mailmover \
                  --bash share/notmuch-mailmover.bash \
                  --fish share/notmuch-mailmover.fish \
                  --zsh share/_notmuch-mailmover
              '';
            }
          );
        in
        {
          # Build the crate as part of `nix flake check` for convenience
          inherit my-crate;

          # Run clippy (and deny all warnings) on the crate source,
          # reusing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          my-crate-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              # `vendored` skipped on purpose: it compiles lua from source.
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          my-crate-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          # Check formatting
          my-crate-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          my-crate-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses
          my-crate-deny = craneLib.cargoDeny {
            inherit src;
          };

          # Run tests with cargo-nextest
          my-crate-nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = (commonArgs.nativeBuildInputs or [ ]) ++ (commonArgs.nativeCheckInputs or [ ]);
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            }
          );

          # Same, but for the rule engine without its notmuch dependency
          my-crate-nextest-no-default-features = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoNextestExtraArgs = "-p notmuch-mailmover-lib --no-default-features";
            }
          );
        }
      );

      packages = forAllSystems (
        { pkgs, ... }:
        let
          system = pkgs.stdenv.hostPlatform.system;
          my-crate = self.checks.${system}.my-crate;
        in
        {
          default = my-crate;
          inherit my-crate;
        }
      );

      devShells = forAllSystems (
        { pkgs, commonArgs, ... }:
        let
          system = pkgs.stdenv.hostPlatform.system;
          craneLib = crane.mkLib pkgs;

          # `checks` ships release-profile artifacts, so a plain `cargo build` in the
          # shell would recompile every dependency. Build the dev profile too, with
          # the same buildInputs, and unpack it into ./target on shell entry.
          cargoDebugArtifacts = craneLib.buildDepsOnly (
            commonArgs
            // {
              # CARGO_PROFILE is what crane's `cargoWithProfile` reads; "debug" is a
              # reserved cargo name, the profile is called "dev".
              env.CARGO_PROFILE = "dev";
            }
          );

          # Must be the same store path buildDepsOnly vendored from, otherwise cargo
          # sees every crate at a new source path (PathToSourceChanged) and rebuilds.
          cargoVendorDir = craneLib.vendorCargoDeps commonArgs;

          hydrate = ''
            mkdir -p target
            STAMP="target/.cargo-debug-artifacts-stamp"

            if [ ! -f "$STAMP" ] || [ "$(< "$STAMP")" != "${cargoDebugArtifacts}" ]; then
              tar -I zstd -xf ${cargoDebugArtifacts}/target.tar.zst -C target
              # Nix paths are content-addressed and immutable; when cargoDebugArtifacts changes, path string changes
              echo -n "${cargoDebugArtifacts}" > "$STAMP"
            fi

            export CARGO_HOME="$PWD/target/cargo-home"
            mkdir -p "$CARGO_HOME"

            # Update config only if vendor dir changed
            if ! cmp -s "${cargoVendorDir}/config.toml" "$CARGO_HOME/config.toml"; then
              rm -f "$CARGO_HOME/config.toml"
              cp "${cargoVendorDir}/config.toml" "$CARGO_HOME/config.toml"
            fi
          '';

          # Script, not a shell function in shellHook: `nix develop -c` runs the hook
          # in a throwaway shell, so functions defined there do not survive. Use it
          # to drop a target/ that a rustc bump invalidated.
          reseedTarget = pkgs.writeShellScriptBin "reseed-target" ''
            rm -rf target
            ${hydrate}
            echo "target/ reset to the Nix cached state."
          '';
        in
        {
          default = craneLib.devShell {
            checks = self.checks.${system};

            packages = [
              pkgs.cargo-llvm-cov
              pkgs.zstd
              cargoDebugArtifacts
              reseedTarget
            ]
            ++ commonArgs.buildInputs;

            # cargo-llvm-cov wants llvm-profdata and llvm-cov, which nixpkgs
            # ships outside the rustc sysroot, named by path. The version has to
            # match the compiler's, hence rustc.llvmPackages.
            env.LLVM_COV = "${pkgs.rustc.llvmPackages.llvm}/bin/llvm-cov";
            env.LLVM_PROFDATA = "${pkgs.rustc.llvmPackages.llvm}/bin/llvm-profdata";

            shellHook = ''
              ${hydrate}
              echo "target/ hydrated: third-party dependencies pre-compiled for the dev profile."
              echo "Escape hatches: reseed-target, or unset CARGO_HOME to go back to crates.io."
            '';
          };

          ci = pkgs.mkShell {
            nativeBuildInputs = [
              pkgs.nfpm
              pkgs.zstd
              pkgs.git-cliff
            ];
          };
        }
      );
    };
}
