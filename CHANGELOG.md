# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

GitHub milestone names (v0.2, v0.4, …) do not always match crate versions: **0.4.0** shipped the v0.2 hardening work; **0.5.0** shipped the v0.4 TUI polish work.

## [Unreleased]

## [0.5.4] - 2026-06-04

### Added

- Sway, wlroots, and Hyprland apply backends via `swaymsg`, `swaybg`, and `hyprctl monitors` ([#78](https://github.com/willfish/walls/pull/78))

## [0.5.3] - 2026-06-04

### Added

- XFCE apply backend via `xfconf-query`, configurable with `"backend": "xfce"` ([#76](https://github.com/willfish/walls/pull/76))

## [0.5.2] - 2026-06-04

### Added

- KDE Plasma apply backend via `dbus-send`, configurable with `"backend": "kde"` ([#74](https://github.com/willfish/walls/pull/74))

## [0.5.1] - 2026-06-04

### Added

- README install guide, scope statement, and MIT `LICENSE` ([#66](https://github.com/willfish/walls/pull/66))
- README architecture (Mermaid) and TUI layout diagrams ([#65](https://github.com/willfish/walls/pull/65))
- GNOME-family apply backend for GNOME, Unity, and Budgie via `gsettings` ([#71](https://github.com/willfish/walls/pull/71))
- Apply backend manual test matrix ([#71](https://github.com/willfish/walls/pull/71))

## [0.5.0] - 2026-06-03

### Added

- TUI: Browse tab (cache queue, local folder candidates, history)
- TUI: Wallhaven Search tab (query edit, search, download + apply)
- TUI: command mode (`:next`, `:prev`, `:pause`, `:status`, `:quit`)
- TUI: `f` favorite and `d` trash current wallpaper
- `WallsCtx::prioritize_cache_id` for queue selection from Browse

## [0.4.0] - 2026-06-03

v0.2 **hardening** milestone (crate semver 0.4.0).

### Added

- Exclusive `fs2` lock on `state.json`; reload state under lock
- `walls config validate`
- `walls-tray` resolves sibling `walls` binary; Open TUI menu item; thumbnail from composed wallpaper
- `systemd/walls-tray.service` and home-manager example tray unit

### Fixed

- Deadlock when `advance_next` held the state lock and called `apply_file`

## [0.3.0] - 2026-06-03

v0.3 **CLI library** milestone.

### Added

- `walls current [--meta]`
- `walls favorite` (copy current original to favorites dir)
- `walls fetch <paths...> [--move]`
- `walls trash` (delete file; clear current, history, cache queue)
- Shared `library` module for collision-safe copy/move

## [0.1.0] - 2026-06-02

Initial share of the workspace (M1–M5 scaffold).

### Added

- `walls-core`: config/state/paths, local sources, Wallhaven client (search, refill, download, quota), `advance_next` / `advance_prev`, COSMIC apply + feh/nitrogen fallback
- `walls` CLI: `apply`, `next`, `prev`, `status`, pause/resume/toggle-pause, minimal TUI (status, now, history, browse queue)
- `walls-tray`: prev / next / toggle-pause
- `systemd` user `walls.service` + `walls.timer`; Nix flake; CI (rustfmt, clippy, test, audit/deny, multi-arch Nix)
- Example `config.json` / `secrets.json` and home-manager timer sketch

[unreleased]: https://github.com/willfish/walls/compare/v0.5.4...HEAD
[0.5.4]: https://github.com/willfish/walls/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/willfish/walls/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/willfish/walls/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/willfish/walls/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/willfish/walls/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/willfish/walls/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/willfish/walls/compare/v0.1.0...v0.3.0
[0.1.0]: https://github.com/willfish/walls/releases/tag/v0.1.0
