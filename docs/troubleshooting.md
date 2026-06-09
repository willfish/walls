# Troubleshooting

Start with:

```bash
walls doctor
```

`walls doctor` groups checks by setup journey and prints `fix:` lines for common
recovery actions. Use `walls doctor --json` when collecting diagnostics for a
script or issue report.

## Quick Symptom Map

| Symptom | Check | Recovery |
| --- | --- | --- |
| First command creates config but no wallpaper changes | `walls doctor` | Apply a known local file with `walls apply <path>` or enable a source with candidates. |
| `walls current` says no current wallpaper | `walls current --json` | Run `walls apply <path>` or `walls next --manual`; look for `"status": "missing_current"`. |
| `walls prev` or `walls undo` reports a missing previous wallpaper | `walls prev --json` | The history entry points at a deleted file; re-apply an available wallpaper with `walls apply <path>` or inspect state with `walls current --json`. |
| `walls next` does nothing | `walls next --manual --verbose` | If paused or rotation is disabled, use `--manual`; inspect provider attempts and local candidates. |
| Local folder ignored | `walls doctor` | Fix the folder `path`, add supported image files, then run `walls config validate`. |
| Backend command missing | `walls doctor` | Install the command for your desktop or set `apply.backend` explicitly. |
| Online provider skipped | `walls next --verbose` | Add credentials to `secrets.json`, loosen filters, retry after rate limits, or keep a local fallback enabled. |
| Tray does not appear | `walls doctor` | Install/run `walls-tray`, confirm the desktop has a tray host, and run `walls config sync`. |
| TUI image preview fails | `WALLS_TUI_PREVIEW=0 walls tui` | Use metadata-only mode or a terminal with Kitty graphics/iTerm2 image support. |
| Cache is too large | `walls cache status` | Run `walls cache prune --dry-run`, then rerun with `--force` if the plan is correct. |
| Trash/cache command refuses to delete | Command `--help` | Use `--dry-run` first, then add `--force` only when the plan is correct. |

## Blank-Canvas Checks

For isolated debugging, set all XDG roots so you do not accidentally reuse your
real config, cache, or state:

```bash
tmp=$(mktemp -d)
XDG_CONFIG_HOME="$tmp/config" \
XDG_DATA_HOME="$tmp/data" \
XDG_CACHE_HOME="$tmp/cache" \
XDG_STATE_HOME="$tmp/state" \
  walls current --json
rm -rf "$tmp"
```

Expected blank state shape:

```json
{
  "changed": false,
  "command": "current",
  "current": null,
  "exit_code_reason": "missing_current",
  "status": "missing_current"
}
```

## Config And Secrets

Validate after every manual edit:

```bash
walls config validate
```

Expected success:

```text
config ok
```

Recovery:

- Schema or path error: fix the reported config path and rerun validation.
- Secret permissions warning: `chmod 600 ~/.config/walls/secrets.json`.
- Secrets missing: create `~/.config/walls/secrets.json` from
  `secrets.example.json` when enabling providers that need credentials.

## Apply Backend Failures

Run:

```bash
walls doctor
walls next --manual
```

If auto-detection picks the wrong path for your session, set `apply.backend` in
`~/.config/walls/config.json`. See [apply backends](apply-backends.md) for the
backend matrix.

Recovery by family:

- COSMIC: ensure `cosmic-ext-bg-ctl` or the COSMIC config patch path is usable.
- GNOME/Unity/Budgie: ensure `gsettings` works in the graphical session.
- KDE Plasma: test inside Plasma with `dbus-send` available.
- XFCE: ensure `xfconf-query` and monitor properties are present.
- Sway/Hyprland/wlroots: check `swaymsg`, `hyprctl`, and `swaybg` as applicable.
- X11 fallback: install `feh` or `nitrogen`.

## Provider And No-Candidate Failures

Run:

```bash
walls doctor
walls next --verbose
walls next --json
```

Use `--verbose` for human diagnosis and `--json` for stable fields such as
skipped providers, missing credentials, offline providers, rate limits, retries,
and fallback provider attempts.

Recovery:

- Keep at least one local source enabled so provider outages do not block manual
  wallpaper changes.
- For Wallhaven, loosen `wallhaven.search` filters or add collection IDs that
  contain matching images.
- For Unsplash, Reddit, Pixabay, or Immich, check provider-specific keys in
  `secrets.json` and file permissions.
- For local sources, add image files with supported extensions or correct the
  configured folder path.

## Tray And Autostart

Run:

```bash
walls doctor
walls config sync --dry-run
walls config sync
walls-tray
```

Recovery:

- `walls-tray` missing: install the tray crate/package and make sure it is on
  `PATH`, or set `WALLS_TRAY_BIN` where supported.
- Autostart mismatch: update `tray.autostart.desktops` and run
  `walls config sync --dry-run`, then `walls config sync` if the planned file
  change is correct.
- Desktop has no tray host: use `walls tui` or CLI commands directly, or install
  a status notifier host for the desktop.
- Open TUI terminal mismatch: set `WALLS_TUI_CMD`, for example
  `ghostty --class=walls -e {walls} tui`. Ghostty documents `--class` as the
  X11 `WM_CLASS` and Wayland application ID override; tray Open TUI passes
  `xdg-terminal-exec --app-id=walls` automatically when that launcher is used.

## TUI Recovery

The TUI is interactive, so generated CLI help and in-app `?` help are more
reliable than copied key tables.

Recovery:

- Start in metadata-only mode: `WALLS_TUI_PREVIEW=0 walls tui`.
- Validate edited config outside the TUI: `walls config validate`.
- Use CLI commands for automation: `walls current --json`, `walls next --json`,
  `walls cache status --json`.
- For contribution-level checks, run the scripts listed in
  [TUI verification](tui-verification.md).

## Safe Destructive Operations

Destructive commands require either a dry run or explicit force:

```bash
walls trash --dry-run --json
walls trash --force --json
walls cache prune --dry-run --json
walls cache prune --force --json
```

Recovery:

- If `trash` reports `force_required`, rerun with `--dry-run` first to inspect
  affected paths.
- If `prev` or `undo` reports `missing_previous`, the selected history entry was
  deleted before it could be restored; `walls` does not advance the history index
  on that failure.
- If cache pruning would remove too much, inspect with
  `walls cache inspect --json` before using `--force`.
