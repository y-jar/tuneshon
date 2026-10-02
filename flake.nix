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
    let
      perSystem = flake-utils.lib.eachDefaultSystem (system:
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
          gtk3
          glib
          glib-networking
          gdk-pixbuf
          cairo
          pango
          atk
          harfbuzz
        ];

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath guiLibs;

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustToolchain
          makeWrapper
        ];

        buildInputs = with pkgs; [ openssl ] ++ guiLibs;

        mkDevShell = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          shellHook = ''
            export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}"
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}"
          '';
        };

        # Desktop launcher entry + icon so tuneshon shows in app grids/launchers.
        desktopItem = pkgs.makeDesktopItem {
          name = "tuneshon";
          exec = "tuneshon";
          icon = "tuneshon";
          desktopName = "update";
          comment = "NixOS update tool (GUI + CLI)";
          categories = [ "System" ];
          type = "Application";
        };

        mkPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "tuneshon";
          version = "0.1.0";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./.) (toString path);
                keep = [ "/Cargo.toml" "/Cargo.lock" "/flake.nix" "/flake.lock" "/src" "/tuneshon_logo.png" ];
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

            # desktop entry + icon (launcher registration)
            install -Dm644 ${desktopItem}/share/applications/tuneshon.desktop \
              $out/share/applications/tuneshon.desktop
            install -Dm644 ${./tuneshon_logo.png} \
              $out/share/icons/hicolor/scalable/apps/tuneshon.png
          '';
        };
      in
      {
        devShells.default = mkDevShell;
        packages.default = mkPackage;
        packages.tuneshon = mkPackage;
      });
    in
    perSystem // {
    # hjem user module (see https://github.com/feel-co/hjem). Lets flake
    # consumers pre-set tuneshon's config dir without in-app setup. This is
    # a hjem user-scope module: `pkgs` resolves at the consumer's system, and
    # `self.packages.${pkgs.stdenv.hostPlatform.system}` picks the built bin.
    hjemModules.tuneshon = { config, lib, pkgs, ... }:
      let
        hjem = config.tuneshon;
      in
      {
        options.tuneshon = {
          enable = lib.mkEnableOption "tuneshon (NixOS update tool)";
          configDir = lib.mkOption {
            type = lib.types.str;
            default = "/etc/nixos";
            description = "NixOS config directory tuneshon operates on.";
          };
          bootLoader = lib.mkOption {
            type = lib.types.str;
            default = "systemd-boot";
            description = "Boot loader name (e.g. systemd-boot, grub).";
          };
          logFile = lib.mkOption {
            type = lib.types.str;
            default = "$HOME/.config/tuneshon/tuneshon.log";
            description = "Path to the tuneshon log file.";
          };
        };
        config = lib.mkIf hjem.enable {
          packages = [
            self.packages.${pkgs.stdenv.hostPlatform.system}.tuneshon
          ];
          environment.sessionVariables.TUNESHON_CONFIG_DIR = hjem.configDir;
        };
      };
    hjemModules.default = self.hjemModules.tuneshon;
    };
}