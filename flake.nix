{
  description = "walls — personal wallpaper manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      pre-commit-hooks,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    flake-utils.lib.eachSystem supportedSystems (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        pre-commit = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          excludes = [
            "(^|.*/)target(/|$)"
            "(^|.*/)result(/|$)"
            "(^|.*/)\\.git(/|$)"
          ];
          hooks = {
            # Fast, auto-fixing hygiene (pre-commit)
            trim-trailing-whitespace.enable = true;
            end-of-file-fixer.enable = true;
            check-merge-conflicts.enable = true;
            detect-private-keys.enable = true;
            check-added-large-files.enable = true;
            check-yaml.enable = true;
            check-shebang-scripts-are-executable.enable = true;

            nixfmt.enable = true;

            shellcheck = {
              enable = true;
              excludes = [ "\\.envrc$" ];
            };
            shfmt.enable = true;

            actionlint.enable = true;

            rustfmt.enable = true;

            # Heavier checks aligned with CI (pre-push)
            walls-clippy = {
              enable = true;
              name = "clippy";
              entry = "${pkgs.cargo}/bin/cargo clippy --workspace --all-targets -- -D warnings";
              language = "system";
              pass_filenames = false;
              stages = [ "pre-push" ];
            };

            walls-test = {
              enable = true;
              name = "cargo test";
              entry = "${pkgs.cargo}/bin/cargo test --workspace";
              language = "system";
              pass_filenames = false;
              stages = [ "pre-push" ];
            };
          };
        };

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
          version = "0.6.5";
          src = wallsSrc;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "-p"
            "walls"
            "-p"
            "walls-tray"
          ];
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
          pre-commit = pre-commit;
        };

        apps.default = {
          type = "app";
          program = "${wallsPkg}/bin/walls";
        };

        formatter = pkgs.writeShellScriptBin "pre-commit-run" ''
          exec ${pre-commit.package}/bin/pre-commit run --all-files --config ${pre-commit.configFile}
        '';

        devShells.default = pkgs.mkShell {
          packages =
            pre-commit.enabledPackages
            ++ (
              with pkgs;
              [
                rustc
                cargo
                clippy
                rustfmt
                rust-analyzer
                cargo-audit
                cargo-deny
                cargo-llvm-cov
                pkg-config
                openssl
                llvm
                imagemagick
                feh
                nitrogen
                jq
                cosmic-bg
              ]
              ++ lib.optionals pkgs.stdenv.isLinux linuxTrayDeps
            );

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          RUST_BACKTRACE = "1";
          LLVM_COV = "${pkgs.llvm}/bin/llvm-cov";
          LLVM_PROFDATA = "${pkgs.llvm}/bin/llvm-profdata";

          shellHook = pre-commit.shellHook + ''
            echo "walls dev shell"
            echo "  git hooks install on enter (pre-commit + pre-push)"
            echo "  nix fmt              — run all format/lint hooks"
            echo "  pre-commit run -a    — same, manually"
            echo "  cargo build / test / clippy"
            echo "  nix build .#checks.${system}.pre-commit"
            echo "  config: ~/.config/walls/config.json"
          '';
        };
      }
    );
}
