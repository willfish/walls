# Example home-manager module for walls.
# Rotation interval is in config.json (change.interval_secs); walls-tray runs the scheduler.
{ pkgs, ... }:
{
  home.packages = [
    pkgs.walls # includes walls.desktop + walls-tray.svg launcher icon on Linux
    pkgs.walls-tray
  ];

  # Optional: pin the terminal used by tray “Open TUI” (desktop launcher uses xdg-terminal-exec via walls.desktop).
  # xdg-desktop-portal provides xdg-terminal-exec on many desktops; otherwise set TERMINAL.
  # systemd.user.services.walls-tray.serviceConfig.Environment = [ "TERMINAL=ghostty" ];

  # JSON schemas for editor/declarative validation:
  # - docs/schemas/config.schema.json
  # - docs/schemas/secrets.schema.json
  xdg.configFile."walls/config.json".source = ./walls-config.json;
  # Prefer sops-nix for secrets:
  xdg.configFile."walls/secrets.json".source = ./walls-secrets.json;

  systemd.user.services.walls = {
    description = "walls rotate wallpaper";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${pkgs.walls}/bin/walls next";
      Environment = "RUST_LOG=walls=info";
    };
  };

  systemd.user.services.walls-tray = {
    description = "walls system tray";
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.walls-tray}/bin/walls-tray";
      Restart = "on-failure";
      Environment = "RUST_LOG=walls_tray=info";
    };
    installConfig.WantedBy = [ "graphical-session.target" ];
  };

  # Optional: legacy systemd timer if you cannot run walls-tray (headless cron-style rotation).
  # Prefer change.interval_secs in config.json + walls-tray.service for graphical sessions.
  # systemd.user.timers.walls = { ... };
}
