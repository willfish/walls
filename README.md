# walls

![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust 1.86+](https://img.shields.io/badge/rust-1.86%2B-orange.svg)
![Platform: Linux](https://img.shields.io/badge/platform-Linux-brightgreen.svg)
![Nix friendly](https://img.shields.io/badge/Nix-friendly-5277C3.svg)
![Interfaces: CLI TUI Tray](https://img.shields.io/badge/interfaces-CLI%20%7C%20TUI%20%7C%20Tray-7C3AED.svg)

`walls` is a Linux wallpaper manager for gradually building a personal wallpaper
library from Wallhaven: rotate through candidates, favorite the ones that land,
and let the good set get better over time.

It is built around the way I actually use wallpapers: Wallhaven search/cache,
one-key favorites, local folders as a durable library, COSMIC first,
Nix-friendly, scriptable from the CLI, and quiet enough to leave running all day
from a small JSON config under `~/.config/walls`.

## Why walls?

| | |
|-|-|
| **Best for** | Linux users who want a slow, low-friction wallpaper curation loop |
| **Workflow** | Wallhaven candidates -> favorite what works -> grow a local collection |
| **Sources** | Wallhaven search/cache plus local folders and favorites |
| **Interfaces** | CLI, keyboard-first TUI, and tray controls |
| **State** | Plain config/cache/state files under predictable directories |
| **Desktop targets** | COSMIC, GNOME-family, KDE Plasma, XFCE, sway/Hyprland, `feh`/`nitrogen` |

- Rotate through Wallhaven results and keep the wallpapers that actually work.
- Favorite a good wallpaper from the CLI, TUI, or tray.
- Keep coming back to a growing local library of wallpapers you already liked.
- Use the same state and actions from `walls`, `walls tui`, and `walls-tray`.
- Apply on COSMIC, GNOME-family desktops, KDE Plasma, XFCE,
  sway/wlroots/Hyprland, or `feh`/`nitrogen` fallback.
- Script it with compact human output or stable `--json` results.

## Screenshots

The TUI shots are captured in Ghostty with a transparent terminal background, so
the active wallpaper remains visible behind the control surface.

| Configure rotation and sources | Manage the sources you keep coming back to |
|---|---|
| <img width="2424" height="1684" alt="walls TUI config overview showing source count, rotation interval, library queue, COSMIC apply backend, and key hints" src="https://github.com/user-attachments/assets/b8bf7f3c-0cbf-4f77-a953-8d7d6bedcb9b" /> | <img width="2192" height="1538" alt="walls TUI source list showing favorites, fetched imports, system backgrounds, Bing image of the day, and Wallhaven query source toggles" src="https://github.com/user-attachments/assets/701f884c-442d-47a2-8857-8823e3505ff2" /> |

| Tune Wallhaven queries | Launch it like a desktop app |
|---|---|
| <img width="2196" height="1532" alt="walls TUI Wallhaven source edit form showing query, category, purity, sorting, aspect ratio, minimum resolution, and API key location" src="https://github.com/user-attachments/assets/0e891b52-9dec-40ff-b276-2d57a60360b9" /> | <img width="2870" height="1920" alt="COSMIC desktop launcher showing the walls application entry over an applied wallpaper" src="https://github.com/user-attachments/assets/995f1803-10be-44a1-ba58-39bcc058da1e" /> |

## Fastest Path To A Working Wallpaper

> [!TIP]
> Start with Wallhaven, then favorite aggressively. Over time `walls` becomes less about finding wallpapers and more about rotating through ones you already know you like.

From a clone, this is the shortest reliable path to install, create config, check
the machine, and apply one known local image:

```bash
nix develop
cargo install --path crates/cli --locked
cargo install --path crates/tray --locked

mkdir -p ~/.config/walls
cp config.example.json ~/.config/walls/config.json
cp secrets.example.json ~/.config/walls/secrets.json

walls doctor
walls apply ~/Pictures/wallpaper.jpg
walls current
walls tui
```

Expected shape:

```text
$ walls doctor
walls doctor: ready
...

$ walls apply ~/Pictures/wallpaper.jpg
/home/alex/Pictures/wallpaper.jpg

$ walls current
/home/alex/Pictures/wallpaper.jpg
```

If a step fails, run `walls doctor` first and follow its `fix:` lines. For
journey-led setup and recovery paths, see:

- [First install and first wallpaper](docs/journeys.md#first-install-and-first-wallpaper)
- [Local-only rotation](docs/journeys.md#local-only-rotation-from-a-folder)
- [Online providers](docs/journeys.md#online-providers)
- [Tray and autostart](docs/journeys.md#tray-and-autostart)
- [TUI usage and config editing](docs/journeys.md#tui-usage-and-config-editing)
- [Cache and quota management](docs/journeys.md#cache-and-quota-management)
- [Troubleshooting guide](docs/troubleshooting.md)

## Fit

`walls` is a good fit if you want a small daily-driver for wallpaper curation on
Linux: Wallhaven search/cache, favorites, local folders, CLI + TUI + tray
controls, and an in-process scheduler driven by `change.interval_secs`.

It is probably not the right fit if you want a [Variety](https://github.com/varietywalls/variety)
clone, quotes or clock overlays, or a broad polished GUI for every desktop. The
1.0 bar is simple: install and rotate reliably on COSMIC/GNOME-family/KDE/XFCE
and sway/Hyprland desktops, with `feh`/`nitrogen` fallback. See the
[apply backend matrix](docs/apply-backends.md) and
[roadmap issues](https://github.com/willfish/walls/issues?q=is%3Aopen+label%3Aepic)
for current gaps.

**MSRV:** `1.86` (workspace `rust-version`). **License:** MIT. See [CHANGELOG](CHANGELOG.md) for release history.

<details>
<summary>Install options and config details</summary>

## Install

### Nix (recommended)

```bash
nix build github:willfish/walls#walls
# binaries: result/bin/walls, result/bin/walls-tray

# or from a clone:
cd walls && nix build .#walls
```

Add `walls` and `walls-tray` to `home.packages` and enable the tray — see [`docs/home-manager.example.nix`](docs/home-manager.example.nix). Rotation interval lives in `config.json` (`change.interval_secs`); `walls-tray` runs the scheduler while the session is active.

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
cp secrets.example.json ~/.config/walls/secrets.json   # API keys: wallhaven (optional), unsplash/reddit (required when enabled)
```

Machine-readable JSON schemas are checked in for editor validation and
declarative config tooling:

- [`docs/schemas/config.schema.json`](docs/schemas/config.schema.json)
- [`docs/schemas/secrets.schema.json`](docs/schemas/secrets.schema.json)

Keep the example `$schema` paths when copying from the repo, or replace them with
raw GitHub URLs when managing config outside the checkout.

Run `walls config validate` after hand-editing config or secrets.

</details>

<details>
<summary>Everyday commands</summary>

## Everyday Commands

| Command | Use |
|---------|-----|
| `walls doctor` | Check config, desktop backend, tray, providers, storage, and TUI preview readiness |
| `walls apply <path>` | Apply a known local image |
| `walls current` | Print the current wallpaper path |
| `walls next --manual` / `walls prev` | Move through rotation candidates and history |
| `walls favorite` | Mark the current wallpaper as a favorite |
| `walls pause` / `walls resume` | Stop or restart automatic rotation |
| `walls tui` | Open the keyboard-first control surface |
| `walls-tray` | Run tray controls and the rotation scheduler |

Most commands support `--json` for scripts. Use `walls --help`,
`walls <command> --help`, or the [journey guide](docs/journeys.md) for the full
surface.

</details>

<details>
<summary>How it works</summary>

## How It Works

`walls-core` owns config, provider queues, cache, history, selection, and apply
backends. The CLI, TUI, and tray call the same runtime, so manual actions,
automatic rotation, and scripted commands all update the same state.

- Config lives in `~/.config/walls`.
- Cache and provider downloads live under the configured cache/download paths.
- `walls-tray` runs the normal scheduler in graphical sessions.
- Custom apply scripts are supported as [trusted user code](docs/security.md#custom-apply-scripts).

The detailed component diagram lives in
[`docs/diagrams/architecture.mmd`](docs/diagrams/architecture.mmd). TUI design,
preview support, and verification contracts live in [`docs/tui.md`](docs/tui.md).

</details>

## Guides

Use [journeys.md](docs/journeys.md) when setting up a real machine or recovering
a workflow. It covers first install, local folders, online providers,
tray/autostart, apply backends, TUI config editing, cache/quota, and JSON
scripting.

Use [troubleshooting.md](docs/troubleshooting.md) when you already have a
symptom. Start with `walls doctor`; it prints concrete `fix:` lines for common
config, desktop, provider, storage, and preview problems.

<details>
<summary>Development</summary>

## Development (Nix)

```bash
cd ~/Repositories/walls
direnv allow   # if using direnv — loads flake via .envrc
nix develop    # installs git pre-commit + pre-push hooks (see flake.nix)

cargo build
cargo test           # integration tests (core + CLI + TUI smoke)
cargo test -p walls-tray   # tray helper tests + tray build
cargo bench -p walls-core --bench hot_paths -- --sample-size 10 --measurement-time 1   # benchmark smoke run; see docs/benchmarks.md
nix build .#checks.x86_64-linux.default   # Nix package + tests (PTY test skipped in sandbox)
```

CI (GitHub Actions) runs rustfmt, clippy, `cargo test`, `cargo llvm-cov` with a documented [coverage floor](docs/coverage.md), release build, `cargo audit` / `cargo deny`, secret scan, and Nix builds on `x86_64-linux`, `aarch64-linux`, `x86_64-darwin`, and `aarch64-darwin`.

Rust conventions for module boundaries, errors, validation, provider I/O, and
tests are documented in [`docs/rust-style.md`](docs/rust-style.md).

**Local hooks:** on `nix develop` / direnv, **commit** runs hygiene + `nixfmt` +
`rustfmt` + `actionlint` + shell checks; **push** runs `clippy` and
`cargo test --workspace`. Run everything with `nix fmt` or `pre-commit run -a`.

</details>
