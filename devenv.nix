{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  # https://devenv.sh/packages/
  packages = [
    pkgs.pkg-config
    pkgs.notmuch
    pkgs.lua5_4
  ];

  # https://devenv.sh/languages/
  languages.rust.enable = true;

  # https://devenv.sh/git-hooks/
  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
    prettier.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
