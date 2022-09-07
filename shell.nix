{ pkgs ? import <nixpkgs> { } }:

with pkgs;

mkShell {
  buildInputs = [ notmuch ];

  shellHook = ''
    export RUST_LOG=debug
  '';
}
