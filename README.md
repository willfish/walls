# walls

Personal wallpaper manager (Rust). JSON config under `~/.config/walls`, COSMIC + Wallhaven-first.

## Development (Nix)

```bash
cd ~/Repositories/walls
direnv allow   # if using direnv — loads flake via .envrc
nix develop    # or: direnv exec . cargo test

cargo build
cargo test           # 27 integration tests (core + CLI)
cargo test -p walls-tray   # tray builds; no automated tests yet
```

## Quick start

```bash
cargo build --release
mkdir -p ~/.config/walls
cp config.example.json ~/.config/walls/config.json
cp secrets.example.json ~/.config/walls/secrets.json   # add wallhaven_api_key for online next
walls apply ~/Pictures/wallpaper.jpg
walls next
walls status
walls tui          # or: walls (no args, on a TTY)
walls-tray         # tray menu → walls prev/next/toggle-pause
```

## Commands

| Command | Status |
|---------|--------|
| `walls apply <path>` | Works |
| `walls status [--json]` | Works |
| `walls pause` / `walls resume` / `walls toggle-pause` | Works |
| `walls next` / `walls prev` | Works (local + Wallhaven cache queue) |
| `walls tui` | Works (also runs when TTY + no subcommand) |
| `walls-tray` | Works (spawns `walls prev` / `next` / `toggle-pause`) |

## systemd timer

User timer units live in `systemd/`. Install `walls.service` and `walls.timer` under `~/.config/systemd/user/`, then:

```bash
systemctl --user daemon-reload
systemctl --user enable --now walls.timer
```

Rotation interval is configured in the **timer unit** (or home-manager), not in `config.json`. `walls pause` makes `walls next` a no-op (exit 0).

See `docs/home-manager.example.nix` for a home-manager sketch.