{
  description = "walls — personal wallpaper manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.cargo;
          rustc = pkgs.rustc;
        };

        wallsSrc = lib.cleanSource ./.;

        wallsPkg = rustPlatform.buildRustPackage {
          pname = "walls";
          version = "0.1.0";
          src = wallsSrc;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "walls" "-p" "walls-tray" ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs;
            lib.optionals stdenv.isLinux [
              gtk3
              gdk-pixbuf
              cairo
              pango
              glib
              atk
              libappindicator
              libdbusmenu-gtk3
              xdotool
            ];
          doCheck = true;

          meta = with lib; {
            description = "Personal wallpaper manager (CLI + tray)";
            license = licenses.mit;
            mainProgram = "walls";
          };
        };
      in
      {
        packages = {
          default = wallsPkg;
          walls = wallsPkg;
          walls-tray = wallsPkg;
        };

        apps.default = {
          type = "app";
          program = "${wallsPkg}/bin/walls";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs;
            [
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer
              pkg-config
              openssl
              imagemagick
              feh
              nitrogen
              jq
              cosmic-bg
            ]
            ++ lib.optionals stdenv.isLinux [
              gtk3
              gdk-pixbuf
              cairo
              pango
              glib
              atk
              libappindicator
              libdbusmenu-gtk3
              xdotool
            ];

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          RUST_BACKTRACE = "1";

          shellHook = ''
            echo "walls dev shell"
            echo "  cargo build          — debug build"
            echo "  cargo test           — run tests"
            echo "  cargo run -- apply <path>"
            echo "  cargo run -- tui"
            echo "  cargo build -p walls-tray && ./target/debug/walls-tray"
            echo "  config: ~/.config/walls/config.json"
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}