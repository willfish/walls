# User Journeys

This guide starts from real workflows instead of command inventory. Use
`walls --help`, `walls <command> --help`, and `walls tui` `?` help for the
authoritative command and key surface.

## First Install And First Wallpaper

Install from a clone:

```bash
git clone https://github.com/willfish/walls.git
cd walls
nix develop
cargo install --path crates/cli --locked
cargo install --path crates/tray --locked
```

Create config and secrets files:

```bash
mkdir -p ~/.config/walls
cp config.example.json ~/.config/walls/config.json
cp secrets.example.json ~/.config/walls/secrets.json
chmod 600 ~/.config/walls/secrets.json
```

Check the machine:

```bash
walls doctor
```

Expected shape:

```text
walls doctor: ready

Config
- [ok] config.config_dir: config directory found at ...
- [ok] config.file: config file found at ...

Desktop/apply
- [ok] desktop.detected: desktop detected as ...
- [ok] desktop.apply_backend: apply backend resolved to ...
```

Warnings are not always blockers. Follow any `fix:` line before debugging lower
level commands.

Apply a known local image:

```bash
walls apply ~/Pictures/wallpaper.jpg
walls current
```

Expected shape:

```text
/home/alex/Pictures/wallpaper.jpg
```

Recovery:

- `No such file or directory`: check the path and file extension.
- Backend command missing: run `walls doctor`, then install the missing desktop
  command or set `apply.backend` explicitly. See [apply backends](apply-backends.md).
- No current wallpaper yet: `walls current --json` reports
  `"status": "missing_current"` until `apply`, `next`, or a TUI/tray action has
  successfully applied a wallpaper.

## Local-Only Rotation From A Folder

Use local-only rotation when you want a reliable setup with no API keys or
network dependency.

1. Put images in a folder, for example `~/Pictures/walls`.
2. Edit `~/.config/walls/config.json`.
3. Enable a `folder` source and disable online sources if you want offline-only
   operation.

Minimal source shape:

```json
{
  "enabled": true,
  "type": "folder",
  "label": "My wallpapers",
  "path": "~/Pictures/walls"
}
```

Validate and test:

```bash
walls config validate
walls doctor
walls next --manual
walls status
```

Expected shape:

```text
$ walls config validate
config ok

$ walls next --manual
/home/alex/Pictures/walls/example.jpg
```

Recovery:

- `providers.source_*.candidates` warning: add supported image files to the
  folder or correct the `path`.
- Automatic changes do not run: check `change.enabled`,
  `change.interval_secs`, and whether `walls-tray` is running.
- Manual next while paused: use `walls next --manual`; automatic `walls next`
  respects pause and rotation-off state.

## Online Providers

Online sources work best when there is also a local fallback source such as
`favorites`, `fetched`, or a `folder`. Provider failures then degrade instead of
leaving you with no candidate.

### Wallhaven

Wallhaven search can work without a key for public results. Add a key in
`~/.config/walls/secrets.json` when using private collections or authenticated
features.

Check readiness:

```bash
walls doctor
walls next --verbose
```

Expected verbose shape:

```text
/home/alex/.local/share/walls/downloaded/wallhaven-abc123.jpg

Provider attempts:
- wallhaven: applied ...
```

Recovery:

- Missing credential: add the key to `secrets.json`, then `chmod 600` the file.
- Rate limited or offline: keep a local fallback enabled and retry later.
- No candidates: broaden `wallhaven.search` filters or add a collection with
  images that match the configured purity/category constraints.

### Unsplash And Reddit

Unsplash and Reddit sources need credentials when enabled. Keep secrets out of
`config.json`; put them in `~/.config/walls/secrets.json`.

Validate and inspect:

```bash
chmod 600 ~/.config/walls/secrets.json
walls config validate
walls doctor
walls next --verbose
```

Recovery:

- `secrets file is readable by group or other users`: run
  `chmod 600 ~/.config/walls/secrets.json`.
- Provider skipped: use `walls next --verbose` or `walls next --json` to see
  skipped providers, missing credentials, offline checks, rate limits, retries,
  and fallbacks.
- Need a no-network mode: disable online sources and use a local folder source.

## Tray And Autostart

`walls-tray` hosts the automatic rotation scheduler in graphical sessions. The
tray menu uses the same CLI actions for manual previous/next, pause/resume, and
opening the TUI.

Check session readiness:

```bash
walls doctor
walls config sync
walls-tray
```

Expected shape:

```text
$ walls config sync
tray autostart: skipped (tray autostart disabled)
```

The exact action depends on `tray.autostart.desktops` and the current desktop.

Desktop autostart is controlled by `tray.autostart.desktops` in
`config.json` and reconciled into `~/.config/autostart/walls-tray.desktop` by
`walls config sync`.

Terminal selection for tray "Open TUI" follows README order:

1. `WALLS_TUI_CMD`
2. `$TERMINAL`
3. `xdg-terminal-exec`
4. `alacritty`

Recovery:

- Tray not visible: run `walls doctor`, install `walls-tray`, confirm the
  desktop has a tray/status notifier host, then start `walls-tray` manually.
- Autostart drift: edit `tray.autostart.desktops`, then run
  `walls config sync`.
- Open TUI launches the wrong terminal: set `WALLS_TUI_CMD`, for example
  `WALLS_TUI_CMD='ghostty -e {walls} tui' walls-tray`.

## Choosing And Fixing Apply Backends

Start with auto-detection:

```json
{
  "apply": {
    "backend": "auto"
  }
}
```

Then run:

```bash
walls doctor
walls next --manual
```

Expected shape:

```text
Desktop/apply
- [ok] desktop.detected: desktop detected as ...
- [ok] desktop.apply_backend: apply backend resolved to ...
```

Use an explicit backend when auto-detection is wrong for your session. See the
full [apply backend matrix](apply-backends.md) for desktop detection, commands,
and manual tests.

Recovery:

- GNOME-family: check `gsettings`.
- KDE: check `dbus-send` inside a Plasma session.
- XFCE: check `xfconf-query` and monitor paths.
- Sway/Hyprland/wlroots: check `swaymsg`, `hyprctl`, or `swaybg` as applicable.
- Fallback X11 sessions: install `feh` or `nitrogen`.
- Custom scripts: treat them as trusted code and read [security](security.md).

## TUI Usage And Config Editing

Start the TUI:

```bash
walls tui
```

Or run `walls` with no arguments on a TTY.

Use the in-app `?` help for current keys. Avoid scripting against TUI text or
hard-coding key details from old screenshots; the CLI and JSON output are the
stable automation surfaces.

Useful companion commands:

```bash
walls current --json
walls status --json
walls config validate
WALLS_TUI_PREVIEW=0 walls tui
```

Recovery:

- Preview problems: set `WALLS_TUI_PREVIEW=0` for metadata-only mode.
- Config edit looks invalid: leave the TUI, run `walls config validate`, fix the
  reported path, and restart `walls tui`.
- Need layout/design details for contribution: read [TUI design and verification](tui.md).

## Cache And Quota Management

Inspect state before removing anything:

```bash
walls cache status
walls cache inspect --provider wallhaven
```

Use dry-run for destructive cache operations:

```bash
walls cache prune --dry-run
walls cache clear-queue --dry-run
walls cache purge-provider-files --dry-run
```

Expected JSON shape:

```json
{
  "command": "cache status",
  "status": "ok",
  "queue": { "len": 0 },
  "quota": { "enabled": true, "over_quota": false }
}
```

Recovery:

- Queue keeps refilling: reduce online provider breadth or increase
  `selection.refetch_when_cache_below` only when you want more prefetching.
- Over quota: run `walls cache prune --dry-run`, inspect the plan, then rerun
  with `--force`.
- Missing current after purge: run `walls next --manual` with a local source
  enabled.

## CLI Scripting And JSON Output

Prefer JSON for scripts:

```bash
walls current --json
walls next --manual --json
walls prev --json
walls doctor --json
walls cache status --json
```

Stable command results use an envelope:

```json
{
  "command": "next",
  "changed": false,
  "status": "no_change",
  "path": null,
  "exit_code_reason": "no_change"
}
```

Script against `status`, `changed`, and `exit_code_reason` rather than human
phrasing. Use `walls --help` and `walls <command> --help` in CI or docs tooling
when you need the current command surface.
