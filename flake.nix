{
  description = "walls — personal wallpaper manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    flake-utils.lib.eachSystem supportedSystems (system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.cargo;
          rustc = pkgs.rustc;
        };

        wallsSrc = lib.cleanSource ./.;

        linuxTrayDeps = with pkgs; [
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

        wallsPkg = rustPlatform.buildRustPackage {
          pname = "walls";
          version = "0.1.0";
          src = wallsSrc;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "walls" "-p" "walls-tray" ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = lib.optionals pkgs.stdenv.isLinux linuxTrayDeps;
          # PTY integration test is unreliable in the Nix build sandbox; CI runs it via cargo.
          cargoTestFlags = [
            "--workspace"
            "--"
            "--skip"
            "tui_with_pty_exits_cleanly_on_quit"
          ];
          doCheck = true;

          meta = with lib; {
            description = "Personal wallpaper manager (CLI + tray)";
            license = licenses.mit;
            mainProgram = "walls";
            platforms = platforms.linux ++ platforms.darwin;
          };
        };
      in
      {
        packages = {
          default = wallsPkg;
          walls = wallsPkg;
        };

        checks = {
          default = wallsPkg;
          walls = wallsPkg;
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
              cargo-audit
              cargo-deny
              pkg-config
              openssl
              imagemagick
              feh
              nitrogen
              jq
              cosmic-bg
            ]
            ++ lib.optionals pkgs.stdenv.isLinux linuxTrayDeps;

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          RUST_BACKTRACE = "1";

          shellHook = ''
            echo "walls dev shell"
            echo "  cargo build          — debug build"
            echo "  cargo test           — run tests"
            echo "  cargo clippy -- -D warnings"
            echo "  cargo fmt --all -- --check"
            echo "  cargo audit && cargo deny check"
            echo "  nix build .#checks.${system}.default"
            echo "  config: ~/.config/walls/config.json"
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}