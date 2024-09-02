{ pkgs ? import <nixpkgs> { } }:

with pkgs;

mkShell {
  buildInputs = [
    notmuch
    nodePackages.markdown-link-check
  ];
}
