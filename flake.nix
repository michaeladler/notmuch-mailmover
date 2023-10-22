{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:

    flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        craneLib = crane.lib.${system};
        src = ./.;

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
        };

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        notmuch-mailmover = craneLib.buildPackage {
          inherit cargoArtifacts src;

          nativeBuildInputs = with pkgs; [
            installShellFiles
          ];

          buildInputs = with pkgs; [ notmuch ];

          postInstall = with pkgs; ''
            installManPage share/notmuch-mailmover.1
            installShellCompletion --cmd notmuch-mailmover \
              --bash share/notmuch-mailmover.bash \
              --fish share/notmuch-mailmover.fish \
              --zsh share/_notmuch-mailmover
          '';
        };
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit notmuch-mailmover;

          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          notmuch-mailmover-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src;
            cargoClippyExtraArgs = "-- --deny warnings";

          };

          # Check formatting
          notmuch-mailmover-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Check code coverage (note: this will not upload coverage anywhere)
          notmuch-mailmover-coverage = craneLib.cargoTarpaulin {
            inherit cargoArtifacts src;

            buildInputs = with pkgs; [ pkgs.notmuch ];
          };
        };

        packages.default = notmuch-mailmover;

        apps.default = flake-utils.lib.mkApp {
          drv = notmuch-mailmover;
        };

        devShells = {
          default = pkgs.mkShell {
            inputsFrom = builtins.attrValues self.checks;

            # Extra inputs can be added here
            nativeBuildInputs = with pkgs; [
              cargo
              rustc
            ];

          };
        };
      });
}
