# Example home-manager module for walls.
# Timer interval is configured here (not duplicated in config.json).
{ pkgs, ... }:
{
  home.packages = [
    pkgs.walls
    pkgs.walls-tray
  ];

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

  systemd.user.timers.walls = {
    description = "walls wallpaper rotation";
    timerConfig = {
      OnBootSec = "3min";
      OnUnitActiveSec = "30min";
      Persistent = true;
    };
    installConfig.WantedBy = [ "timers.target" ];
  };
}
