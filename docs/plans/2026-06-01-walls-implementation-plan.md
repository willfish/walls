# walls — Full implementation plan

**Status:** **Superseded** — historical Variety-parity inventory. For current scope and execution order use [`2026-06-02-walls-roadmap-to-1.0.md`](2026-06-02-walls-roadmap-to-1.0.md). M2–M5 tasks from the older [`2026-06-01-walls-m2-m5-execution-plan.md`](2026-06-01-walls-m2-m5-execution-plan.md) are largely done.

**Goal:** Personal Rust wallpaper manager (`walls`) with JSON config under `~/.config/walls` (home-manager), runtime state under `~/.local/state/walls`, CLI + systemd timer + tray + TUI. Primary online source: Wallhaven. Desktop apply: full parity with Variety’s `set_wallpaper` / `get_wallpaper` / `set_lock_screen` backends (user wants most DE “plugins”), with COSMIC as the author’s daily driver.

**Non-goals for v1:** GTK preferences GUI, Jumble runtime plugins, Variety “smart” analytics/sync, serverside config.

**Reference:** Variety (`varietywalls/variety`) — especially `variety/data/scripts/set_wallpaper`, `variety/data/config/variety.conf`, `variety/Options.py`, downloaders under `variety/plugins/builtin/downloaders/`.

---

## Design principles

1. **One core library** (`walls-core`) — config, state, sources, pipeline, apply backends, Wallhaven client.
2. **One primary binary** (`walls`) — subcommands; TUI behind `walls tui` or auto when TTY.
3. **No daemon required** — `systemd` timer runs `walls next`; optional `walls daemon` later for tray status only.
4. **systemd does not invoke the TUI** — only `walls next`, `walls prev`, etc.
5. **Config vs state** — home-manager owns `config.json` + `secrets.json`; tool owns `state.json` + cache.
6. **Apply backends are Rust modules** — not bash; each maps to Variety’s DE branch with tests/fixtures where possible.
7. **Online sources are Cargo features** — `sources-wallhaven` (default), `sources-unsplash`, … — not `.so` plugins at first.
8. **Image pipeline** — prefer `image` + optional `magick` CLI for user filter strings (Variety-compatible).

---

## Repository layout (target)

```text
walls/
├── Cargo.toml                    # workspace
├── .gitignore
├── README.md                     # later
├── config.example.json
├── secrets.example.json
├── docs/
│   ├── plans/
│   │   └── 2026-06-01-walls-implementation-plan.md   # this file
│   └── home-manager.example.nix
├── systemd/
│   ├── walls.service
│   └── walls.timer
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── paths.rs
│   │   │   ├── config.rs
│   │   │   ├── state.rs
│   │   │   ├── selection.rs
│   │   │   ├── pipeline/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── auto_rotate.rs
│   │   │   │   ├── filters.rs
│   │   │   │   ├── display_mode.rs
│   │   │   │   ├── clock.rs
│   │   │   │   ├── quotes.rs
│   │   │   │   └── copyto.rs
│   │   │   ├── sources/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── local.rs
│   │   │   │   ├── favorites.rs
│   │   │   │   ├── fetched.rs
│   │   │   │   └── registry.rs
│   │   │   ├── apply/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── detect.rs
│   │   │   │   ├── fill_mode.rs
│   │   │   │   ├── gnome.rs
│   │   │   │   ├── kde.rs
│   │   │   │   ├── xfce.rs
│   │   │   │   ├── mate.rs
│   │   │   │   ├── cinnamon.rs
│   │   │   │   ├── lxde.rs
│   │   │   │   ├── lxqt.rs
│   │   │   │   ├── cosmic.rs
│   │   │   │   ├── sway.rs
│   │   │   │   ├── hyprland.rs
│   │   │   │   ├── wlroots.rs
│   │   │   │   ├── enlightenment.rs
│   │   │   │   ├── awesome.rs
│   │   │   │   ├── fluxbox.rs
│   │   │   │   ├── trinity.rs
│   │   │   │   ├── lingmo.rs
│   │   │   │   ├── deepin.rs
│   │   │   │   ├── feh_nitrogen.rs
│   │   │   │   └── lock_screen.rs
│   │   │   └── wallhaven/
│   │   │       ├── mod.rs
│   │   │       ├── client.rs
│   │   │       └── types.rs
│   │   └── tests/
│   │       ├── fixtures/
│   │       └── ...
│   ├── cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       └── tui/
│   └── tray/                     # optional crate or bin + feature
│       └── ...
└── sources/                      # reference: Variety defaults (optional copy)
    └── variety-defaults.md
```

---

## Config & state (complete schemas)

### Paths (XDG)

| Path | Purpose |
|------|---------|
| `~/.config/walls/config.json` | Policy (home-manager) |
| `~/.config/walls/secrets.json` | API keys (mode 0600, sops) |
| `~/.local/state/walls/state.json` | History, queue, indices, pause |
| `~/.local/share/walls/cache/` | Downloaded Wallhaven images |
| `~/.local/share/walls/downloaded/` | Variety-equivalent “Downloaded” |
| `~/.local/share/walls/favorites/` | Favorites copies |
| `~/.local/share/walls/fetched/` | Clipboard / URL fetch inbox |
| `~/.local/share/walls/wallpaper/` | Composed output (filters, clock, quotes) |

### `config.json` — full schema (maps from Variety `variety.conf`)

```json
{
  "change": {
    "enabled": true,
    "on_start": false,
    "interval_secs": 300,
    "internet_enabled": true,
    "safe_mode": false,
    "change_lock_screen": false,
    "download_preference_ratio": 0.9
  },
  "paths": {
    "cache_dir": "~/.local/share/walls/cache",
    "download_dir": "~/.local/share/walls/downloaded",
    "favorites_dir": "~/.local/share/walls/favorites",
    "fetched_dir": "~/.local/share/walls/fetched",
    "compose_dir": "~/.local/share/walls/wallpaper"
  },
  "quota": {
    "enabled": true,
    "size_mb": 1000
  },
  "apply": {
    "backend": "auto",
    "cosmic": {
      "method": "cosmic-config",
      "config_path": "~/.config/cosmic/com.system76.CosmicBackground/v1/all"
    },
    "custom_script": null,
    "lock_screen_backend": "auto"
  },
  "display": {
    "mode": "os",
    "auto_rotate": false
  },
  "filters": [
    { "enabled": false, "name": "Keep original", "magick_args": "" },
    { "enabled": false, "name": "Grayscale", "magick_args": "-type Grayscale" }
  ],
  "clock": {
    "enabled": false,
    "font": "Serif 70",
    "date_font": "Serif 30",
    "filter": "-density 100 ..."
  },
  "quotes": {
    "enabled": false,
    "font": "Serif 30",
    "text_color": [255, 255, 255],
    "bg_color": [80, 80, 80],
    "bg_opacity": 55,
    "width_percent": 70,
    "hpos_percent": 100,
    "vpos_percent": 40,
    "max_length": 250,
    "text_shadow": false,
    "disabled_sources": ["Urban Dictionary"],
    "tags": "",
    "authors": "",
    "change_enabled": false,
    "change_interval_secs": 300,
    "favorites_file": "~/.config/walls/favorite_quotes.txt",
    "favorites_format": "fortune"
  },
  "selection": {
    "desired_color_enabled": false,
    "desired_color": null,
    "min_size_enabled": false,
    "min_size_percent": 80,
    "use_landscape_enabled": true,
    "lightness_enabled": false,
    "lightness_mode": "dark",
    "min_rating_enabled": false,
    "min_rating": 4,
    "name_regex_enabled": false,
    "name_regex": ".*",
    "avoid_recent": 50,
    "refetch_when_cache_below": 5,
    "strategy": "random"
  },
  "favorites": {
    "operations": [
      { "folder_class": "Downloaded", "op": "copy" },
      { "folder_class": "Fetched", "op": "move" },
      { "folder_class": "Others", "op": "copy" }
    ]
  },
  "clipboard": {
    "enabled": false,
    "use_whitelist": true,
    "hosts": ["wallhaven.cc", "wallpapers.net", "flickr.com", "imgur.com", "deviantart.com"]
  },
  "copyto": {
    "enabled": false,
    "folder": "default"
  },
  "sources": [
    { "enabled": true, "type": "favorites", "label": "The Favorites folder" },
    { "enabled": true, "type": "fetched", "label": "The Fetched folder" },
    { "enabled": true, "type": "folder", "path": "/usr/share/backgrounds" },
    { "enabled": true, "type": "wallhaven", "label": "Main", "query": "", "url": null }
  ],
  "wallhaven": {
    "collections": [],
    "search": {
      "q": "",
      "categories": "111",
      "purity": "100",
      "sorting": "random",
      "order": "desc",
      "atleast": "1920x1080"
    },
    "prefer": "collections_then_search",
    "throttle": {
      "max_downloads_per_hour": 360,
      "max_queue_fills_per_hour": 40
    }
  },
  "timer": {
    "respect_paused": true
  }
}
```

### `secrets.json`

```json
{
  "wallhaven_api_key": "",
  "unsplash_access_key": "",
  "reddit_client_id": "",
  "reddit_client_secret": ""
}
```

### `state.json`

```json
{
  "paused": false,
  "no_effects_on": null,
  "current": {
    "source_id": "wallhaven:abc123",
    "wallhaven_id": "abc123",
    "original_path": "/path/to/original.jpg",
    "composed_path": "/path/to/composed.jpg"
  },
  "history": ["id1", "id2"],
  "history_index": 0,
  "wallhaven": {
    "random_seed": null,
    "collection_pages": {},
    "search_page": 1
  },
  "cache_queue": ["id3", "id4"],
  "unseen_by_source": {},
  "last_change_unix": 0,
  "quote": { "text": "", "author": "" }
}
```

---

## Feature inventory (from Variety → walls phases)

### Legend

| Priority | Meaning |
|----------|---------|
| **P0** | First usable product |
| **P1** | Soon after; daily-driver completeness |
| **P2** | Parity with Variety defaults / power features |
| **P3** | Nice-to-have / low personal value |

---

### A. Wallpaper apply backends (`walls-core::apply`)

Variety `set_wallpaper` branches — implement as `DesktopBackend` enum + `auto` detection (`detect.rs` mirrors bash `detect_desktop()`).

| Backend ID | Variety DE / condition | Mechanism | Priority |
|------------|------------------------|-----------|----------|
| `enlightenment` | enlightenment, moksha | `edje_cc`, `enlightenment_remote`, `identify`, `bc` | P2 |
| `kde` | kde | D-Bus `org.kde.plasmashell` `evaluateScript`; unsupported plugin detection; `kdialog` optional | P1 |
| `gnome` | gnome, unity, budgie | `gsettings` `org.gnome.desktop.background` picture-uri + picture-uri-dark + picture-options | P1 |
| `deepin` | deepin | `com.deepin.wrap.gnome.desktop.background` | P3 |
| `xfce` | xfce | `xfconf-query` backdrop props; create per-monitor props via `xrandr` if missing | P1 |
| `lingmo` | lingmo | `qdbus` `com.lingmo.Theme.setWallpaper` | P3 |
| `lxde` | lxde | `pcmanfm --set-wallpaper` | P2 |
| `lxqt` | lxqt | `pcmanfm-qt --set-wallpaper` | P2 |
| `fluxbox` | fluxbox | `fbsetbg` + `~/.fluxbox/lastwallpaper` | P2 |
| `sway` | sway | `wlroots_set()` then `swaymsg output * bg` | P1 |
| `hyprland` | hyprland | `wlroots_set()` then `hyprctl hyprpaper` per monitor | P1 |
| `trinity` | trinity | `dcop kdesktop KBackgroundIface setWallpaper` | P3 |
| `mate` | mate | `org.mate.background` / `org.mate.desktop.background` picture-options | P2 |
| `cinnamon` | cinnamon | `org.cinnamon.desktop.background` | P2 |
| `awesome` | awesome | `awesome-client` Lua one-liner | P2 |
| `cosmic` | `XDG_CURRENT_DESKTOP=COSMIC` | RON patch `source: Path("...")` or `cosmic-ext-bg-ctl set` | **P0** (author) |
| `feh` / `nitrogen` | fallback WMs | `feh --bg-fill` / `nitrogen --set-zoom-fill` | P1 |
| `custom_script` | user override | Execute `apply.custom_script` with args `(composed, trigger, original, fill_mode)` | P1 |

**`wlroots_set` helper (shared):** try in order: `wpaperctl set`, `awww img`, `swaybg -i -m fill` (Sway kill-old workaround).

**Fill mode param ($4 in Variety):** `os | zoom | spanned | centered | scaled | stretched | wallpaper` — map to each backend where supported (GNOME family honors `picture-options`; feh/nitrogen TODO in Variety).

**`get_wallpaper` parity:** `walls current --path` reads current URI/path per backend (see `get_wallpaper` script) — needed for “revert at start” / history back.

**Lock screen (`set_lock_screen`):**

| Backend | Mechanism | Priority |
|---------|-----------|----------|
| `kde` | `kwriteconfig5` / `kscreenlockerrc` | P2 |
| `gnome` | `org.gnome.desktop.screensaver` picture-uri | P2 |
| `cosmic` | TBD (document if unsupported) | P3 |

Config: `change.change_lock_screen` runs second apply after desktop.

---

### B. Display modes (`pipeline/display_mode.rs`)

From Variety plugins — config `display.mode`:

| Mode ID | Behavior | Priority |
|---------|----------|----------|
| `os` | No pre-scale; pass fill mode to apply backend | P0 |
| `smart` | Small image → tile; ratio ≈ screen → zoom; ratio ≈ multimonitor → span; else blur-pad | P2 |
| `zoom` | ImageMagick scale `WxH^` | P1 |
| `fill-with-black` | Resize + black pad | P2 |
| `fill-with-blur` | Blurred backdrop composite | P2 |
| `gnome-*` | Delegate scaling to GNOME (`centered`, `scaled`, …) | P2 |

Implementation: build `magick` argv from templates with `%W` `%H` replaced by primary display size (`xrandr` / `wlr-randr` / COSMIC output).

---

### C. Image pipeline (`pipeline/`)

Order (same as `VarietyWindow.do_set_wp`):

1. Validate readable original.
2. `auto_rotate` (EXIF) — config `display.auto_rotate` (Variety default in code: True; shipped conf: False).
3. Random enabled **filter** (ImageMagick args from config).
4. **Display mode** pre-scale → temp file in `compose_dir`.
5. **Quote** overlay (if enabled).
6. **Clock** overlay (if enabled).
7. **Copyto** (LightDM / shared backgrounds) if enabled.
8. Prune old `wallpaper-*` temps.
9. **Apply** composed image; optional lock screen.
10. Update state/history; hooks (Wallhaven download endpoint).

| Step | Priority |
|------|----------|
| Pass-through (no effects) | P0 |
| auto_rotate | P1 |
| filters | P2 |
| display modes (non-os) | P2 |
| clock | P3 |
| quotes | P3 |
| copyto | P3 |

**Refresh levels** (for quote/clock-only refresh without re-filtering): `all | filters_and_texts | texts | clock_only` — store `post_filter_filename` in state.

---

### D. Sources (local + remote)

Variety default sources (`variety.conf` + `sources.txt`):

| Source type | Config | Priority |
|-------------|--------|----------|
| `favorites` | Directory | P1 |
| `fetched` | Directory | P1 |
| `folder` | Recursive local path | P0 |
| `image` | Single file | P1 |
| `album_by_filename` | Ordered album in folder | P2 |
| `album_by_date` | EXIF/date ordering | P3 |
| `wallhaven` | Query URL or keywords + API | **P0** |
| `wallhaven_legacy` | Old API | P3 |
| `unsplash` | OAuth/API | P3 |
| `bing` | Daily image | P2 |
| `apod` | NASA APOD | P3 |
| `reddit` | Subreddit | P3 |
| `earthview` | Google Earth | P3 |
| `artstation` | User/artist | P3 |
| `mediarss` | RSS feed | P3 |
| `flickr` | User photostream (disabled in default conf) | P3 |

**Selection across sources:**

- Weighted random with `download_preference_ratio` (0.9 default): prefer unseen downloads.
- Per-source enable flag.
- Filters: landscape, min size %, dominant color, lightness, min rating, name regex.
- Album mode: walk album in order before leaving folder.

---

### E. Wallhaven (P0 detail)

**API:** v1 — see https://wallhaven.cc/help/api

| Operation | Endpoint | Use |
|-----------|----------|-----|
| Wallpaper | `GET /w/{id}` | Metadata + download URL |
| Search | `GET /search` | Config search + TUI browse |
| Collections list | `GET /collections` | User collections |
| Collection wallpapers | `GET /collections/{user}/{id}` | Primary feed |
| Settings | `GET /settings` | Validate API key |
| On apply | client “download” tracking | Call when wallpaper set (Unsplash-style requirement) |

**Configurable source (Variety `WallhavenSource`):**

- Keywords **or** full Wallhaven URL pasted (parse query params).
- Throttling: 360 downloads/hour, 40 queue fills/hour (configurable).
- Validation: test query returns images.

**State:**

- `cache_queue` of ids ready to apply.
- `unseen_downloads` per source in state file.
- Refill when `cache_queue.len() < refetch_when_cache_below`.

**Quota:** delete oldest in `download_dir` when over `quota.size_mb`.

---

### F. CLI commands (Variety `VarietyOptionParser` mapping)

| walls command | Variety equivalent | Priority |
|---------------|-------------------|----------|
| `walls next` | `-n` / `--next` | P0 |
| `walls prev` | `-p` / `--previous` | P0 |
| `walls next --no-history` | `--fast-forward` | P1 |
| `walls apply <path>` | `--set` | P0 |
| `walls current` | `--get` / `--current` | P1 |
| `walls current --meta` | `--meta` | P2 |
| `walls pause` / `resume` / `toggle-pause` | pause/resume | P0 |
| `walls trash` | `--trash` | P2 |
| `walls favorite` | `--favorite` | P1 |
| `walls move-favorite` | `--move-to-favorites` | P2 |
| `walls fetch [paths...]` | CLI args → Fetched | P1 |
| `walls enqueue <url>` | remote URL fetch | P2 |
| `walls status [--json]` | (new) | P0 |
| `walls no-effects` | `--toggle-no-effects` | P2 |
| `walls quotes next/prev/...` | quotes_* | P3 |
| `walls tui` | N/A (GTK selector) | P1 |
| `walls config validate` | (new) | P1 |

**Not ported:** `--profile` (multi-instance), `--preferences`, `--history`, `--downloads` (GTK thumbs) — TUI replaces selector.

**IPC:** Variety uses running instance + D-Bus; walls v1 is **stateless CLI** + file lock on `state.json`. Optional v2: Unix socket for single-instance.

---

### G. TUI (ratatui)

| Screen | Capabilities | Priority |
|--------|--------------|----------|
| **Now** | Current image meta, `n`/`p`, re-apply, open path, trash, favorite | P1 |
| **History** | Scroll history, apply, filter | P1 |
| **Browse** | Wallhaven results + cache queue + local folders | P1 |
| **Search** | Live Wallhaven search (params); `:save` writes to user override file only | P2 |
| **Sources** | Toggle sources (writes `config.local.json` override optional) | P2 |
| **Filters** | Preview with filter applied | P3 |
| **Quotes** | Next/prev quote, save favorite | P3 |
| **Status** | Effective config, backends detected, cache size, paused | P0 |

**No in-TUI settings editor for home-manager fields** — Status screen links to `config.json` paths.

**Preview:** Kitty/iTerm graphics protocol optional; fallback metadata only.

**Keymap:** `j/k`, Enter, `n/p`, `1-8` tabs, `?`, `:` command line (`:next`, `:pause`, `:fetch 10`).

---

### H. Tray (`walls-tray`)

| Item | Action | Priority |
|------|--------|----------|
| Next | `walls next` | P1 |
| Previous | `walls prev` | P1 |
| Pause/Resume | `walls toggle-pause` | P1 |
| Browse | spawn terminal `walls tui` | P2 |
| Quit tray | exit tray only | P1 |

Icon: optional current wallpaper thumbnail.

---

### I. systemd + home-manager

```ini
# walls.timer — interval lives HERE (not duplicated in config.json)
[Timer]
OnBootSec=3min
OnUnitActiveSec=5min
Persistent=true

[Service]
Type=oneshot
ExecStart=%h/.local/bin/walls next
Environment=RUST_LOG=walls=info
```

`config.change.enabled` + `state.paused` → `walls next` no-ops with exit 0.

**home-manager:** `xdg.configFile`, `systemd.user.services`, `sops` secrets, `packages = [ walls walls-tray ]`.

---

### J. Quotes sources (P3)

| Source | Variety plugin |
|--------|----------------|
| Fortune | `FortuneSource` |
| Local files | `LocalFilesSource` |
| QuotationsPage | web scrape |
| Goodreads | web |
| Urban Dictionary | disabled by default |

Engine rotates on `quotes.change_interval` when `quotes.change_enabled`.

---

### K. Slideshow (P3)

Variety pan/zoom fullscreen — separate tool (`variety-slideshow`). walls: optional `walls slideshow` stub or document external tool.

---

## Apply backend implementation notes (per DE)

### COSMIC (author P0)

1. **cosmic-config:** Regex replace `source: Path("...")` in `v1/all`; use **original** path for param $3 if Variety behavior desired (Variety uses `$3` original in sed, `$1` composed for others — document: walls uses **composed** path for display, config patch target per user setting).
2. **cosmic-ext-bg-ctl:** `set <path>` — preferred if installed in NixOS.
3. **Reload:** optional `pkill -HUP cosmic-bg` if patch not live.

### KDE

Port Plasma JS from Variety; surface unsupported wallpaper plugins; detect locked widgets (Plasma 5.7+).

### GNOME family

Set `picture-uri`, `picture-uri-dark`, `picture-options` from fill mode.

### XFCE

Enumerate backdrop keys; bootstrap monitor paths (#164).

### Sway / Hyprland

Shared `wlroots` helper; Hyprland `hyprctl` JSON via `serde_json` (drop `jq` dependency).

### Enlightenment

Largest backend — code generation to `.edc`/`.edj`; async `enlightenment_remote`; cleanup old `variety_wallpaper_*` files.

### feh / nitrogen

Map fill mode: `zoom` → `--bg-fill` / `--set-zoom-fill` (Variety TODO: respect $4).

---

## Testing strategy

| Area | Method |
|------|--------|
| Config/state serde | unit + fixture JSON |
| COSMIC RON patch | fixture file in `tests/fixtures/cosmic-all.sample` |
| Wallhaven | `wiremock` |
| Apply backends | manual matrix per DE; CI only fixtures + command argv snapshots |
| Pipeline | golden magick argv strings |
| CLI | `assert_cmd` with `HOME=/tmp/...` |

---

## Milestones

| Milestone | Contents |
|-----------|----------|
| **M0** | Empty repo + this plan |
| **M1** | `walls apply`, COSMIC backend, config/state load |
| **M2** | `walls next/prev`, folder source, state history |
| **M3** | Wallhaven collection/search + cache + quota |
| **M4** | systemd timer + home-manager example |
| **M5** | Tray + TUI Now/History/Browse/Status |
| **M6** | Apply backends: GNOME, KDE, XFCE, sway, hyprland, feh |
| **M7** | Display modes + filters + favorites |
| **M8** | Remaining DEs + lock screen + quotes/clock |

---

## Phase 0 — Repository bootstrap

### Task 0.1: Workspace skeleton

```bash
cd ~/Repositories/walls
# create Cargo workspace (crates/core, crates/cli) — see prior plan for Cargo.toml stubs
cargo build
```

### Task 0.2: Example configs

Copy `config.example.json` and `secrets.example.json` from schema section above.

### Task 0.3: Commit policy

- Plan file: **review before commit** (`docs/plans/2026-06-01-walls-implementation-plan.md`).
- First code commit: `chore: initialize walls workspace` (no plan required in commit).

---

## Phase 1 — Core foundation (P0)

Detailed tasks (same structure as before):

1. `paths.rs` — XDG discovery, `expand_home`
2. `config.rs` + `secrets.rs` — full schema deserialize
3. `state.rs` — load/save, atomic write via temp rename
4. `WallsCtx::load()` — wire paths from config
5. Unit tests for all fixtures

**Verify:** `cargo test -p walls-core`

---

## Phase 2 — Apply layer (P0 COSMIC + P1 auto)

1. `apply/fill_mode.rs` — enum + GNOME mapping
2. `apply/detect.rs` — port bash `detect_desktop()`
3. `apply/cosmic.rs` — RON patch + tests
4. `apply/mod.rs` — trait `Applier`, dispatch `backend: auto|cosmic|...`
5. `apply/feh_nitrogen.rs` — fallback
6. `walls apply <path>`

**Verify:** manual on COSMIC session

---

## Phase 3 — Pipeline minimal (P0)

1. `pipeline/mod.rs` — `compose(original) -> composed_path`
2. Pass-through only first
3. Write composed to `compose_dir/wallpaper-<hash>.jpg`
4. `apply(composed, fill_mode, trigger: auto|manual|refresh)`

---

## Phase 4 — Local sources + selection (P0)

1. `sources/local.rs` — walk folder, image, favorites, fetched
2. `selection.rs` — random, avoid_recent, album traversal
3. `walls next` / `walls prev` without network
4. `walls status --json`

---

## Phase 5 — Wallhaven (P0)

1. `wallhaven/types.rs` — API models
2. `wallhaven/client.rs` — reqwest + rate limit (45/min)
3. Collection + search refill
4. Download + metadata sidecar JSON
5. Quota enforcement
6. `on_set_wallpaper` ping if required by API terms

---

## Phase 6 — CLI completeness (P1)

Implement: `favorite`, `fetch`, `current`, `toggle-pause`, `trash`, fast-forward next.

File lock: `fcntl` on `state.json` for timer vs tray races.

---

## Phase 7 — systemd + home-manager (P1)

1. `systemd/walls.{service,timer}`
2. `docs/home-manager.example.nix` — config, secrets, timer, tray unit, `cosmic-ext-bg` package optional

---

## Phase 8 — Tray (P1)

`walls-tray` binary, spawn `walls` with `current_exe` sibling.

---

## Phase 9 — TUI (P1)

`ratatui` screens: Status, Now, History, Browse (see section G).

Feature flag: `cargo build --features tui`.

---

## Phase 10 — Apply backends rollout (P1–P2)

Implement in order of community use + your machines:

1. gnome (includes unity, budgie, deepin wrap)
2. kde
3. xfce
4. sway + wlroots + hyprland
5. mate, cinnamon
6. lxde, lxqt, awesome, fluxbox
7. enlightenment, trinity, lingmo
8. `get_wallpaper` for each
9. `lock_screen` kde + gnome

Each backend PR includes:

- `impl Applier for ...`
- `impl GetWallpaper for ...` (optional trait)
- fixture or argv snapshot test
- row in `docs/apply-backends.md` manual test checklist

---

## Phase 11 — Pipeline advanced (P2)

1. `auto_rotate`
2. `filters` (random magick)
3. `display_mode` smart/zoom/blur
4. `refresh_level` in `walls next --refresh clock`

---

## Phase 12 — Extra sources (P2–P3)

Cargo features:

- `sources-bing`
- `sources-unsplash`
- `sources-reddit`
- etc.

Each: `Source` trait + config block + tests with wiremock.

---

## Phase 13 — Quotes & clock (P3)

1. `fortune` / local quotes
2. ImageMagick clock filter string with `%HOFFSET` / font resolution via `fontconfig`
3. Quote renderer (Pango optional — or defer to magick only)

---

## Dependencies (workspace)

### `walls-core`

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"          # optional human config; JSON is primary
anyhow = "1"
thiserror = "1"
tracing = "0.1"
directories = "5"
dirs = "5"
walkdir = "2"
rand = "0.8"
regex = "1"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "fs", "process"] }
fs2 = "0.4"           # file locks on state
chrono = { version = "0.4", features = ["serde"] }
image = "0.25"
tempfile = "3"

# Apply backends (feature-gated to reduce deps)
zbus = { version = "4", optional = true }
gio = { version = "0.20", optional = true }
glib = { version = "0.20", optional = true }
serde_json = "1"      # hyprland json

[features]
default = ["apply-cosmic", "apply-feh", "sources-wallhaven", "sources-local"]
apply-cosmic = []
apply-gnome = ["gio", "glib"]
apply-kde = ["zbus"]
apply-xfce = []
apply-sway = []
apply-all = ["apply-gnome", "apply-kde", "apply-xfce", "apply-sway", "..."]
sources-wallhaven = []
sources-bing = []
tui = []              # only in cli crate
```

### `walls` (cli)

```toml
clap = { version = "4", features = ["derive"] }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
ratatui = { version = "0.29", optional = true }
crossterm = { version = "0.28", optional = true }
```

### `walls-tray`

```toml
tray-icon = "0.19"
muda = "0.15"
winit = "0.30"
```

### Dev

```toml
wiremock = "0.6"
assert_cmd = "2"
predicates = "3"
```

---

## Open decisions (resolve before M1 code)

| # | Question | Recommendation |
|---|----------|----------------|
| 1 | COSMIC apply: composed vs original path in RON? | Config `apply.cosmic.use_original_path` default `false` (use composed) |
| 2 | Interval in config or only systemd? | **systemd only**; `config.change.interval_secs` deprecated/doc only |
| 3 | License | MIT |
| 4 | Binary name | `walls` + `walls-tray` |
| 5 | State lock | `fs2` exclusive lock on `state.json` during next/prev |
| 6 | ImageMagick required? | Optional; pipeline steps skip if `magick`/`convert` missing |

---

## Suggested first 15 commits

1. `chore: cargo workspace`
2. `feat(core): config, secrets, state`
3. `feat(apply): detect + cosmic`
4. `feat(cli): apply`
5. `feat(pipeline): pass-through compose`
6. `feat(sources): local folder + favorites`
7. `feat(cli): next prev status pause`
8. `feat(wallhaven): client + download`
9. `feat(cli): next with wallhaven`
10. `docs: systemd + home-manager`
11. `feat(tray): prev next pause`
12. `feat(tui): status + now + history`
13. `feat(apply): gnome + feh fallback`
14. `feat(apply): kde + xfce + sway`
15. `docs: apply backend manual test matrix`

---

## Execution handoff

1. Review this plan; adjust priorities per machine (COSMIC daily).
2. Commit plan when ready: `git add docs/plans/2026-06-01-walls-implementation-plan.md`
3. Start Phase 0–1 implementation (scaffold workspace).

**Recommended path:** M1→M5 (COSMIC + Wallhaven + timer + tray + TUI), then Phase 10 apply backends for portability.
