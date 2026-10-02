{ pkgs ? import <nixpkgs> {} }:

let
  guiLibs = with pkgs; [
    libGL
    libxkbcommon
    wayland
    xorg.libxcb
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr
    fontconfig
    freetype
    expat
    zlib
    openssl
  ];
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath guiLibs;
in
pkgs.mkShell {
  buildInputs = [ pkgs.cargo pkgs.rustc pkgs.pkg-config pkgs.makeWrapper ] ++ guiLibs;
  shellHook = ''
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}"
    run()   { cargo run -- "$@"; }
    build() { cargo build --release; }
    test()  { cargo test "$@"; }
    printf 'tuneshon shell ready: use `run`, `build`, or `test`.\n'
  '';
}