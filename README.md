# walls

Personal wallpaper manager (Rust). JSON config under `~/.config/walls`, COSMIC + Wallhaven-first.

## Scope

**walls is:** a small daily-driver for rotating wallpapers — local folders, Wallhaven search/cache, CLI + TUI + tray, and a systemd user timer. Apply targets **COSMIC** first (`cosmic-ext-bg-ctl` + RON patch), with **feh/nitrogen** fallback when detection does not find COSMIC.

**walls is not:** a [Variety](https://github.com/varietywalls/variety) clone. There is no quotes/clock overlay pipeline, no broad multi-DE matrix yet (see [v0.5 roadmap](docs/plans/2026-06-02-walls-roadmap-to-1.0.md)), and no image-effect pipeline (v0.6). PRs welcome, but the 1.0 bar is “install and rotate on COSMIC (or feh fallback)”.

**MSRV:** `1.78` (workspace `rust-version`). **License:** MIT.

## Install

### Nix (recommended)

```bash
nix build github:willfish/walls#walls
# binaries: result/bin/walls, result/bin/walls-tray

# or from a clone:
cd walls && nix build .#walls
```

Add `walls` and `walls-tray` to `home.packages`, wire the user timer and tray — see [`docs/home-manager.example.nix`](docs/home-manager.example.nix). Rotation interval lives in the **timer unit**, not `config.json`.

### Cargo (from source)

```bash
git clone https://github.com/willfish/walls.git && cd walls
nix develop   # optional: hooks, cosmic-bg, feh for local apply tests
cargo install --path crates/cli --locked
cargo install --path crates/tray --locked
```

On Linux, `walls-tray` needs GTK/libappindicator (see `flake.nix` `linuxTrayDeps`).

### Config

```bash
mkdir -p ~/.config/walls
cp config.example.json ~/.config/walls/config.json
cp secrets.example.json ~/.config/walls/secrets.json   # wallhaven_api_key for online next
```

## Architecture

Component flow (source: [`docs/diagrams/architecture.mmd`](docs/diagrams/architecture.mmd)):

```mermaid
flowchart TD
    CONFIG["~/.config/walls"]
    CACHE["~/.cache/walls"]
    LOCAL[Local folders]
    WH[Wallhaven API]
    TIMER[walls.timer]
    TRAY[walls-tray]
    CLI["walls CLI / TUI"]
    CORE[walls-core]
    APPLY["COSMIC or feh"]

    TIMER -->|walls next| CLI
    TRAY -->|subprocess| CLI
    CONFIG --> CORE
    CACHE --> CORE
    LOCAL --> CORE
    WH --> CORE
    CLI --> CORE
    CORE --> APPLY

    classDef box stroke:#6366f1,stroke-width:2px
    class CONFIG,CACHE,LOCAL,WH,TIMER,TRAY,CLI,CORE,APPLY box
```

- **Config** — `config.json`, `secrets.json`, locked `state.json` (history, queue, current).
- **Cache** — downloaded Wallhaven images and composed outputs.
- **Triggers** — `walls.timer` runs `walls next`; tray runs `walls prev` / `next` / `toggle-pause` / opens TUI.

### TUI layout (`walls tui`)

Terminal screen regions (not a second runtime — same `walls` binary as the CLI):

```
┌ walls ────────────────────────────────────────────────┐
│ [Status][Now][History][Browse][Search]     1-5 tabs   │
├───────────────────────────────────────────────────────┤
│ > list (tab-specific)                                 │
│   Status   paused, paths, queue count                 │
│   Now      current wallpaper paths                    │
│   History  j/k · Enter apply                          │
│   Browse   queue · locals · history · Enter apply     │
│   Search   i query · Enter search/apply (API key)     │
├ keys ─────────────────────────────────────────────────┤
│ n/p  f/d  space  :  q                                 │
└───────────────────────────────────────────────────────┘
  :next :prev :pause :status :quit   (Esc cancels : mode)
```

## Development (Nix)

```bash
cd ~/Repositories/walls
direnv allow   # if using direnv — loads flake via .envrc
nix develop    # installs git pre-commit + pre-push hooks (see flake.nix)

cargo build
cargo test           # integration tests (core + CLI + TUI smoke)
cargo test -p walls-tray   # tray builds; no automated tests yet
nix build .#checks.x86_64-linux.default   # Nix package + tests (PTY test skipped in sandbox)
```

CI (GitHub Actions) runs rustfmt, clippy, `cargo test`, release build, `cargo audit` / `cargo deny`, secret scan, and Nix builds on `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, and `aarch64-darwin`.

**Local hooks** (Nix-managed, like [forte#194](https://github.com/willfish/forte/pull/194)): on `nix develop` / direnv, **commit** runs hygiene + `nixfmt` + `rustfmt` + `actionlint` + shell checks; **push** runs `clippy` and `cargo test --workspace`. Run everything with `nix fmt` or `pre-commit run -a`.

## Demo

CLI workflow (isolated config; `custom-script` apply). The TUI uses the terminal alternate screen — record it with [`demo/record-tui.sh`](demo/record-tui.sh) (gpu-screen-recorder), or validate headless with [`scripts/validate-tui-pty.sh`](scripts/validate-tui-pty.sh).

![walls CLI demo](demo/demo.gif)

Regenerate: `nix-shell -p asciinema asciinema-agg --run './demo/record-cli.sh'`

## Quick start

After [install](#install):

```bash
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
| `walls current [--meta]` | Works |
| `walls favorite` | Works |
| `walls fetch <paths...> [--move]` | Works |
| `walls trash` | Works |
| `walls config validate` | Works |
| `walls pause` / `walls resume` / `walls toggle-pause` | Works |
| `walls next` / `walls prev` | Works (local + Wallhaven cache queue) |
| `walls tui` | Works — tabs: Status/Now/History/Browse/Search; `:` commands; `f`/`d` favorite/trash |
| `walls-tray` | Works (prev/next/pause, Open TUI, thumbnail icon) |

Set `WALLS_TUI_CMD` to override the terminal launch command (`{walls}` is substituted). Defaults to `$TERMINAL -e walls tui` (terminal: `alacritty`).

## systemd timer

User units live in `systemd/`. Install `walls.service`, `walls.timer`, and optionally `walls-tray.service` under `~/.config/systemd/user/`, then:

```bash
systemctl --user daemon-reload
systemctl --user enable --now walls.timer
systemctl --user enable --now walls-tray.service
```

Rotation interval is configured in the **timer unit** (or home-manager), not in `config.json`. `walls pause` makes `walls next` a no-op (exit 0).

See `docs/home-manager.example.nix` for a home-manager sketch.
