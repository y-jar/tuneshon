{
  description = "tuneshon - a lightweight NixOS update tool (Rust GUI + CLI).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Runtime/build libraries required by eframe (egui) on Linux/NixOS.
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

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath guiLibs;

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
          makeWrapper
        ] ++ guiLibs;

        buildInputs = with pkgs; [ openssl ];

        mkDevShell = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          shellHook = ''
            export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}"
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}"
          '';
        };

        mkPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "tuneshon";
          version = "0.1.0";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./.) (toString path);
                keep = [ "/Cargo.toml" "/Cargo.lock" "/flake.nix" "/flake.lock" "/src" ];
              in
              type == "directory" || builtins.any (s: pkgs.lib.hasPrefix s rel) keep;
          };
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          inherit nativeBuildInputs buildInputs;
          postInstall = ''
            wrapProgram $out/bin/tuneshon \
              --prefix LD_LIBRARY_PATH : "${LD_LIBRARY_PATH}"
          '';
        };
      in
      {
        devShells.default = mkDevShell;
        packages.default = mkPackage;
        packages.tuneshon = mkPackage;
      });
}