# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

GitHub milestone names (v0.2, v0.4, …) do not always match crate versions: **0.4.0** shipped the v0.2 hardening work; **0.5.0** shipped the v0.4 TUI polish work.

## [Unreleased]

## [0.13.2] - 2026-06-14

Reissued mutable release for the completed v0.10.0, v0.11.0, v0.12.0, and v0.13.0 milestone work after moving the repository to rewritten linear conventional-commit history.

### Added

- Online provider maturity: configurable source providers, monitor-aware Wallhaven defaults, provider retries/timeouts, provider status reporting, and clearer skip/failure explanations.
- TUI maturity: complete safe config editing, source editing, key-profile support, preview browsing, search improvements, startup polish, help/discoverability, and safer wallpaper actions.
- Tray maturity: StatusNotifier and AppIndicator share action dispatch, richer action feedback, favorite/current actions, preview prewarming, TUI launch recovery, and scroll-wheel next/previous support where the tray host forwards SNI scroll events.
- Observability and recovery: event journal foundation, `walls logs`, last-run summaries, first-run doctor guidance, recovery text for missing current/previous wallpapers, provider failures, and credential/setup issues.
- Cache and desktop tooling: cache inspect/prune/quota clarity, tray/autostart diagnostics, wrapped terminal desktop identity, and backend diagnostics.
- Quality infrastructure: config/secrets schemas, benchmark coverage for hot paths, Rust style documentation, stronger CI coverage floor, deterministic TUI smoke coverage, and TUI module extraction.

### Changed

- Bumped workspace and packaged release version metadata to `0.13.2`.
- Added release automation for maintaining the classic `Unreleased` changelog section, cutting mutable tags, and publishing GitHub releases from Cocogitto output.
- Reworked documentation around user journeys, troubleshooting, README positioning, and the TUI-first project experience.

### Fixed

- Multiple TUI consistency, navigation, status-colour, fallback-state, and recovery edge cases from the quality follow-up backlog.

## [0.8.0] - 2026-06-05

### Added

- Provider classification support for v0.10 stories (TDD-driven skeletons for config compatibility): Reddit (#158), Bing (#159), NASA APOD (#160), Media RSS (#161), attribution metadata (#162). Merged after rebase, fmt fixes, and CI babysitting (PRs #180-#183).
- Closes epic #148 and stories.

## [0.7.0] - 2026-06-05

### Added

- TUI: Config tab is now the first page, with provider/configuration blocks (Rotation, Local sources, Wallhaven, Library, Apply/display). Focused block expands with details; others remain scannable. Toggle ('t') and cycle ('e') for safe edits on supported fields. Pause/status not duplicated in body. (Stories #150–#155; PRs #169–#176)
- Provider compatibility layer: `SourceEntry` + `ProviderKind`/`Descriptor` + `Unsupported` handling allows new `type` values in config.json without schema breaks or load failures. (Story #156; PR #177)
- Unsplash as configurable per-source provider (`type: "unsplash"` in sources list, with query/collection/user/topic/orientation/url shorthand, dedicated client/refill/cache, metadata/attribution). (Story #157; PR #178)

### Changed

- GitHub milestones (v0.9+) now track remaining roadmap; crate versions follow semver independently (as noted in prior changelog).

## [0.6.5] - 2026-06-05

### Fixed

- `WallsCtx::load` now warns on Unix when `secrets.json` is readable by group or other users, recommending `chmod 600` without failing startup ([#120](https://github.com/willfish/walls/pull/120)).
- ImageMagick filter/display-mode command spawning now retries transient `Text file busy` errors, stabilizing CI and freshly-created wrapper scripts ([#121](https://github.com/willfish/walls/pull/121)).

## [0.6.4] - 2026-06-04

### Added

- Optional `tui-preview` feature for terminal image previews in Ghostty/Kitty via Kitty graphics and iTerm2 via inline images, with metadata-only fallback in unsupported terminals ([#90](https://github.com/willfish/walls/pull/90)).

### Changed

- Raised the workspace MSRV to Rust 1.86 for the maintained Ratatui image preview dependency.

### Fixed

- Wallhaven cache path lookup now probes standard filenames directly before falling back to legacy scans ([#116](https://github.com/willfish/walls/pull/116)).
- `WallsCtx::load` now warns on non-fatal config validation issues instead of leaving them hidden until manual validation ([#117](https://github.com/willfish/walls/pull/117)).
- `walls-tray` no longer unwraps executable parent paths while resolving the `walls` binary, and keeps sibling/WALLS_BIN/PATH fallback behavior covered by tests ([#118](https://github.com/willfish/walls/pull/118)).

## [0.6.3] - 2026-06-04

### Added

- `walls next --refresh <level>` can reapply the current wallpaper at refresh levels `all`, `filters-and-texts`, `texts`, and `clock-only` ([#88](https://github.com/willfish/walls/pull/88))

## [0.6.2] - 2026-06-04

### Added

- `display.mode` can compose configured target-size outputs for `zoom`, `fill-with-black`, and `fill-with-blur` ([#86](https://github.com/willfish/walls/pull/86))

## [0.6.1] - 2026-06-04

### Added

- `display.filters` can opt into random ImageMagick filter commands during wallpaper composition ([#84](https://github.com/willfish/walls/pull/84))

## [0.6.0] - 2026-06-04

### Added

- `display.auto_rotate` now applies EXIF orientation into the composed wallpaper output before applying wallpapers ([#82](https://github.com/willfish/walls/pull/82))

## [0.5.5] - 2026-06-04

### Added

- Auto-detection now selects KDE, XFCE, Sway, and Hyprland native apply backends before feh/nitrogen fallback ([#80](https://github.com/willfish/walls/pull/80))

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

[unreleased]: https://github.com/willfish/walls/compare/v0.13.2...HEAD
[0.13.2]: https://github.com/willfish/walls/releases/tag/v0.13.2
[0.9.0]: https://github.com/willfish/walls/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/willfish/walls/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/willfish/walls/compare/v0.6.5...v0.7.0
[0.6.5]: https://github.com/willfish/walls/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/willfish/walls/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/willfish/walls/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/willfish/walls/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/willfish/walls/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/willfish/walls/compare/v0.5.5...v0.6.0
[0.5.5]: https://github.com/willfish/walls/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/willfish/walls/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/willfish/walls/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/willfish/walls/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/willfish/walls/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/willfish/walls/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/willfish/walls/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/willfish/walls/compare/v0.1.0...v0.3.0
[0.1.0]: https://github.com/willfish/walls/releases/tag/v0.1.0
