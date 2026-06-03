# walls M2–M5 Execution Plan

**Goal:** Reach a daily-driver `walls` on COSMIC: `next`/`prev` from local folders and Wallhaven, systemd timer rotation, tray prev/next/pause, and a minimal TUI — building on the existing M1 scaffold.

**Approach:** Extend `walls-core` with `sources`, `selection`, and `wallhaven` modules; wire `walls next`/`prev` through `WallsCtx::advance()`. Use a **systemd user timer** (not a daemon) calling `walls next`. Add `walls-tray` as a thin subprocess spawner. TUI last, behind `--features tui`. Defer multi-DE apply backends to M6 (see `2026-06-01-walls-implementation-plan.md`).

**Key Files:**
- `crates/core/src/sources/local.rs` (new)
- `crates/core/src/sources/mod.rs` (new)
- `crates/core/src/selection.rs` (new)
- `crates/core/src/wallhaven/{mod,client,types}.rs` (new)
- `crates/core/src/ctx.rs` (modify — `advance_next`, `advance_prev`)
- `crates/cli/src/main.rs` (modify — real `next`/`prev`)
- `crates/cli/src/commands/` (new — split commands)
- `systemd/walls.{service,timer}` (new)
- `docs/home-manager.example.nix` (new)
- `crates/tray/` or `crates/cli/src/bin/tray.rs` (new, M5)

**Tech Notes:**
- Dev shell: `nix develop` (see `flake.nix`). Run tests with `nix develop -c cargo test`.
- Config: `~/.config/walls/config.json` — copy from `config.example.json`.
- State: `~/.local/state/walls/state.json` — tool-managed.
- `walls next` must respect `state.paused` and `config.change.enabled`.
- Wallhaven: 45 req/min; API key in `secrets.json`.
- Nix flake needs git-tracked sources; `git add` new files before `nix build`.

**Done (M1 — do not re-implement):**
- `WallsCtx`, config/state load, COSMIC apply, `walls apply|status|pause|resume`, 4 tests, `flake.nix` devShell.

---

## Milestone map

| Milestone | User-visible outcome |
|-----------|----------------------|
| **M2** | `walls next` / `walls prev` cycle images from local folders |
| **M3** | `walls next` pulls from Wallhaven cache + collections/search |
| **M4** | systemd timer rotates wallpapers; home-manager example |
| **M5** | Tray + TUI (Status, Now, History, Browse) |

---

# M2 — Local sources + next/prev

### Task 1: Add `sources` module stub

**Files:**
- Create: `crates/core/src/sources/mod.rs`
- Create: `crates/core/src/sources/local.rs`
- Modify: `crates/core/src/lib.rs`

```rust
// crates/core/src/lib.rs — add:
pub mod sources;
pub mod selection;
```

```rust
// crates/core/src/sources/mod.rs
mod local;

pub use local::{list_images, SourceImage};

use crate::config::SourceEntry;

pub fn enabled_sources(entries: &[SourceEntry]) -> Vec<&SourceEntry> {
    entries.iter().filter(|s| s.enabled).collect()
}
```

- [ ] `nix develop -c cargo build` succeeds
- [ ] Commit: `feat(core): sources module stub`

---

### Task 2: Failing test — list folder images

**Files:**
- Create: `crates/core/tests/local_sources.rs`

```rust
use std::fs;
use walls_core::config::SourceEntry;
use walls_core::sources::list_images;

#[test]
fn lists_images_in_folder() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.jpg"), b"x").unwrap();
    fs::write(dir.path().join("b.png"), b"x").unwrap();
    fs::write(dir.path().join("skip.txt"), b"x").unwrap();

    let src = SourceEntry {
        enabled: true,
        source_type: "folder".into(),
        label: None,
        path: Some(dir.path().display().to_string()),
        query: None,
        url: None,
    };
    let images = list_images(&src).unwrap();
    assert_eq!(images.len(), 2);
}
```

- [ ] Run: `nix develop -c cargo test local_sources`
- [ ] Expected: **FAIL** — `list_images` not found or unimplemented
- [ ] Commit: `test: folder image listing`

---

### Task 3: Implement `list_images` for folder/favorites/fetched

**Files:**
- Modify: `crates/core/src/sources/local.rs`

```rust
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::SourceEntry;
use crate::paths::expand_home;

const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "bmp", "gif"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImage {
    pub path: PathBuf,
    pub source_id: String,
}

pub fn list_images(entry: &SourceEntry) -> anyhow::Result<Vec<SourceImage>> {
    let path = resolve_path(entry)?;
    if !path.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for dent in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
        if !dent.file_type().is_file() {
            continue;
        }
        let p = dent.path();
        if is_image(p) {
            out.push(SourceImage {
                path: p.to_path_buf(),
                source_id: p.file_name().unwrap().to_string_lossy().into_owned(),
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn resolve_path(entry: &SourceEntry) -> anyhow::Result<PathBuf> {
    match entry.source_type.as_str() {
        "folder" | "image" => {
            let p = entry.path.as_ref().ok_or_else(|| anyhow::anyhow!("missing path"))?;
            Ok(expand_home(p))
        }
        "favorites" | "fetched" => {
            anyhow::bail!("favorites/fetched need WallsPaths — use list_images_with_paths")
        }
        other => anyhow::bail!("unsupported source type: {other}"),
    }
}

pub fn list_images_with_paths(
    entry: &SourceEntry,
    favorites: &Path,
    fetched: &Path,
) -> anyhow::Result<Vec<SourceImage>> {
    let path = match entry.source_type.as_str() {
        "favorites" => favorites.to_path_buf(),
        "fetched" => fetched.to_path_buf(),
        _ => return list_images(entry),
    };
    let mut e = entry.clone();
    e.source_type = "folder".into();
    e.path = Some(path.display().to_string());
    list_images(&e)
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXT.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}
```

Add to `crates/core/Cargo.toml`:

```toml
walkdir = "2"
```

- [ ] `nix develop -c cargo test local_sources` — **PASS**
- [ ] Commit: `feat(sources): list local folder images`

---

### Task 4: Failing test — selection picks unseen image

**Files:**
- Create: `crates/core/src/selection.rs`
- Create: `crates/core/tests/selection.rs`

```rust
// crates/core/tests/selection.rs
use walls_core::selection::{pick_next, PickInput};

#[test]
fn avoids_recent_ids() {
    let candidates = vec!["a".into(), "b".into(), "c".into()];
    let recent = vec!["a".into(), "b".into()];
    let pick = pick_next(&PickInput {
        candidates: &candidates,
        recent: &recent,
        avoid_recent: 10,
    })
    .unwrap();
    assert_eq!(pick, "c");
}
```

```rust
// crates/core/src/selection.rs
pub struct PickInput<'a> {
    pub candidates: &'a [String],
    pub recent: &'a [String],
    pub avoid_recent: usize,
}

pub fn pick_next(input: &PickInput) -> anyhow::Result<String> {
    let recent_set: std::collections::HashSet<_> =
        input.recent.iter().take(input.avoid_recent).collect();
    let pool: Vec<_> = input
        .candidates
        .iter()
        .filter(|c| !recent_set.contains(*c))
        .cloned()
        .collect();
    let pool = if pool.is_empty() { input.candidates.to_vec() } else { pool };
    use rand::seq::SliceRandom;
    pool.choose(&mut rand::thread_rng())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no candidates"))
}
```

Add `rand = "0.8"` already present.

- [ ] Test passes
- [ ] Commit: `feat(core): selection pick_next`

---

### Task 5: `WallsCtx::collect_candidates` + `advance_next`

**Files:**
- Modify: `crates/core/src/ctx.rs`

Add methods (implement fully):

```rust
impl WallsCtx {
    pub fn collect_local_candidates(&self) -> anyhow::Result<Vec<std::path::PathBuf>> {
        use crate::sources::{enabled_sources, list_images_with_paths};
        let mut paths = Vec::new();
        for src in enabled_sources(&self.config.sources) {
            if !matches!(
                src.source_type.as_str(),
                "folder" | "favorites" | "fetched" | "image"
            ) {
                continue;
            }
            for img in list_images_with_paths(
                src,
                &self.paths.favorites_dir,
                &self.paths.fetched_dir,
            )? {
                paths.push(img.path);
            }
        }
        Ok(paths)
    }

    pub fn advance_next(&mut self) -> anyhow::Result<Option<std::path::PathBuf>> {
        if self.state.paused || !self.config.change.enabled {
            tracing::info!("skipped: paused or change disabled");
            return Ok(None);
        }
        let paths = self.collect_local_candidates()?;
        let ids: Vec<String> = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let id = crate::selection::pick_next(&crate::selection::PickInput {
            candidates: &ids,
            recent: &self.state.history,
            avoid_recent: self.config.selection.avoid_recent,
        })?;
        let path = paths
            .into_iter()
            .find(|p| p.display().to_string() == id)
            .ok_or_else(|| anyhow::anyhow!("picked path vanished"))?;
        self.apply_file(&path, crate::apply::ApplyTrigger::Auto)?;
        Ok(Some(path))
    }

    pub fn advance_prev(&mut self) -> anyhow::Result<Option<std::path::PathBuf>> {
        if self.state.history.len() < 2 {
            return Ok(None);
        }
        self.state.history_index = (self.state.history_index + 1).min(self.state.history.len() - 1);
        let id = self.state.history[self.state.history_index].clone();
        let path = std::path::PathBuf::from(&id);
        if path.exists() {
            self.apply_file(&path, crate::apply::ApplyTrigger::Manual)?;
            return Ok(Some(path));
        }
        Ok(None)
    }
}
```

**Note:** History stores display paths for M2; switch to stable ids in M3.

- [ ] `nix develop -c cargo build`
- [ ] Commit: `feat(ctx): advance_next and advance_prev (local)`

---

### Task 6: Wire CLI `next` and `prev`

**Files:**
- Modify: `crates/cli/src/main.rs`

Replace stubs:

```rust
Some(Command::Next) => {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_next()? {
        Some(p) => println!("{}", p.display()),
        None => println!("no change"),
    }
}
Some(Command::Prev) => {
    let mut ctx = WallsCtx::load()?;
    match ctx.advance_prev()? {
        Some(p) => println!("{}", p.display()),
        None => println!("no previous"),
    }
}
```

- [ ] Manual: `walls next` twice with `config.json` pointing at `/usr/share/backgrounds` (or test dir)
- [ ] Commit: `feat(cli): next and prev commands`

---

### Task 7: Integration test — next updates state

**Files:**
- Create: `crates/core/tests/advance_next.rs`

Use temp config dir via env var **or** pass paths into test-only constructor. Simplest approach: add `WallsCtx::load_from(paths_override)` for tests.

```rust
#[test]
fn advance_next_writes_state() {
    // temp dir with one jpg + minimal config.json
    // WallsCtx::load_from(tmp) -> advance_next -> assert state.current.is_some()
}
```

- [ ] Commit: `test: advance_next updates state`

---

# M3 — Wallhaven

### Task 8: Add reqwest + wallhaven types

**Files:**
- Modify: `crates/core/Cargo.toml` — add `reqwest` with `rustls-tls`, `json`
- Create: `crates/core/src/wallhaven/types.rs`
- Create: `crates/core/src/wallhaven/mod.rs`

```rust
// wallhaven/types.rs (minimal)
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Wallpaper {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub data: Vec<Wallpaper>,
    pub meta: SearchMeta,
}

#[derive(Debug, Deserialize)]
pub struct SearchMeta {
    pub current_page: u32,
    pub last_page: u32,
}
```

- [ ] Commit: `feat(wallhaven): API types`

---

### Task 9: Failing test — parse search fixture

**Files:**
- Create: `crates/core/tests/fixtures/wallhaven-search.json` (minimal 1-item JSON from API docs)
- Create: `crates/core/tests/wallhaven_parse.rs`

- [ ] Test fails until deserializer wired
- [ ] Commit: `test: wallhaven search JSON parse`

---

### Task 10: `WallhavenClient::search` with wiremock

**Files:**
- Create: `crates/core/src/wallhaven/client.rs`
- Add dev-dep `wiremock = "0.6"`

Implement `search(&self, params, page) -> Result<SearchResponse>` with header `X-API-Key`.

- [ ] `nix develop -c cargo test wallhaven`
- [ ] Commit: `feat(wallhaven): HTTP client`

---

### Task 11: Download to cache

**Files:**
- Extend `wallhaven/client.rs`

```rust
pub async fn download_to_cache(
    &self,
    wp: &Wallpaper,
    cache_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let ext = wp.path.rsplit('.').next().unwrap_or("jpg");
    let dest = cache_dir.join(format!("wallhaven-{}.{}", wp.id, ext));
    if dest.exists() {
        return Ok(dest);
    }
    let bytes = self.http.get(&wp.path).send().await?.bytes().await?;
    tokio::fs::write(&dest, &bytes).await?;
    Ok(dest)
}
```

Add `tokio` with `fs` to core.

- [ ] Commit: `feat(wallhaven): download to cache`

---

### Task 12: Refill `cache_queue` in state

**Files:**
- Create: `crates/core/src/wallhaven/refill.rs`
- Modify: `crates/core/src/ctx.rs`

```rust
pub async fn refill_wallhaven_cache(&mut self) -> anyhow::Result<()> {
    if self.state.cache_queue.len() >= self.config.selection.refetch_when_cache_below {
        return Ok(());
    }
    let client = WallhavenClient::new(&self.secrets.wallhaven_api_key)?;
    // collections first per config.wallhaven.prefer
    // push ids onto state.cache_queue, dedupe
    self.save_state()?;
    Ok(())
}
```

- [ ] Commit: `feat(wallhaven): cache queue refill`

---

### Task 13: `advance_next` prefers cache_queue then local

**Files:**
- Modify: `crates/core/src/ctx.rs`

Logic:

1. If `cache_queue` non-empty → pop id → download if missing → apply.
2. Else `refill_wallhaven_cache().await?` if `internet_enabled`.
3. Else fall back to M2 local `collect_local_candidates`.

Make `advance_next` async; CLI already uses `#[tokio::main]`.

- [ ] Manual test with real API key in `~/.config/walls/secrets.json`
- [ ] Commit: `feat(ctx): next uses wallhaven cache`

---

# M4 — systemd + home-manager

### Task 14: Add systemd units

**Files:**
- Create: `systemd/walls.service`
- Create: `systemd/walls.timer`

```ini
# systemd/walls.service
[Unit]
Description=walls — rotate wallpaper

[Service]
Type=oneshot
ExecStart=%h/.local/bin/walls next
```

```ini
# systemd/walls.timer
[Unit]
Description=walls rotation timer

[Timer]
OnBootSec=3min
OnUnitActiveSec=5min
Persistent=true

[Install]
WantedBy=timers.target
```

- [ ] Commit: `docs: systemd user units`

---

### Task 15: home-manager example

**Files:**
- Create: `docs/home-manager.example.nix`

```nix
{ pkgs, ... }:
{
  home.packages = [ pkgs.walls ];

  xdg.configFile."walls/config.json".source = ./walls-config.json;
  xdg.configFile."walls/secrets.json".source = ./walls-secrets.json; # sops

  systemd.user.services.walls = {
    Unit.Description = "walls rotate";
    Service.ExecStart = "${pkgs.walls}/bin/walls next";
  };
  systemd.user.timers.walls = {
    Unit.Description = "walls timer";
    Timer.OnBootSec = "3min";
    Timer.OnUnitActiveSec = "30min";
    Install.WantedBy = [ "timers.target" ];
  };
}
```

- [ ] Commit: `docs: home-manager example`

---

### Task 16: Document timer + pause in README

**Files:**
- Modify: `README.md`

Add section: timer interval lives in **home-manager only**; `walls pause` skips `next`.

- [ ] Commit: `docs: README systemd and pause`

---

# M5 — Tray + TUI

### Task 17: `walls-tray` binary stub

**Files:**
- Modify: `Cargo.toml` workspace members
- Create: `crates/tray/Cargo.toml`, `crates/tray/src/main.rs`

```rust
// spawns: walls prev | walls next | walls pause toggle
let walls = std::env::var("WALLS_BIN").unwrap_or_else(|_| "walls".into());
std::process::Command::new(&walls).arg("next").status()?;
```

Add tray deps: `tray-icon`, `muda`, `winit` in flake devShell.

- [ ] Commit: `feat(tray): prev/next menu stub`

---

### Task 18: Enable TUI feature + app shell

**Files:**
- Create: `crates/cli/src/tui/mod.rs`, `app.rs`
- Modify: `crates/cli/Cargo.toml` — `ratatui`, `crossterm` optional
- Modify: `flake.nix` devShell — add them when hacking TUI

`walls` with no subcommand + TTY → run TUI; else require subcommand.

- [ ] Commit: `feat(tui): app shell and tabs`

---

### Task 19: TUI Status + Now screens

Wire `n`/`p` to `ctx.advance_next()` / `advance_prev()`.

- [ ] Commit: `feat(tui): status and now screens`

---

### Task 20: TUI History + Browse

Browse shows `cache_queue` + last search results stored in state.

- [ ] Commit: `feat(tui): history and browse`

---

# M6+ (separate plan)

Multi-DE apply backends (GNOME, KDE, XFCE, sway, …), pipeline filters/clock/quotes, extra downloaders — see **`docs/plans/2026-06-01-walls-implementation-plan.md`** sections Phase 10–13.

---

## Verification checklist (after M5)

```bash
cd ~/Repositories/walls
nix develop -c cargo test
nix develop -c cargo clippy -- -D warnings
walls apply ~/test.jpg
walls next
walls prev
walls status --json
systemctl --user start walls.timer
```

---

## Suggested commit sequence (M2–M5)

1. `feat(core): sources module stub`
2. `test: folder image listing`
3. `feat(sources): list local folder images`
4. `feat(core): selection pick_next`
5. `feat(ctx): advance_next and advance_prev (local)`
6. `feat(cli): next and prev commands`
7. `feat(wallhaven): API types + client`
8. `feat(wallhaven): download and cache refill`
9. `feat(ctx): next uses wallhaven cache`
10. `docs: systemd and home-manager`
11. `feat(tray): prev/next menu`
12. `feat(tui): status, now, history, browse`

---

## Execution handoff

Plan saved to `docs/plans/2026-06-01-walls-m2-m5-execution-plan.md`.

**Reference (full feature inventory):** `docs/plans/2026-06-01-walls-implementation-plan.md`

Do you want to:

1. **Execute now** — implement task-by-task in this session (starting at Task 1), or
2. **Subagent-driven** — one fresh subagent per task (recommended for M3+ async Wallhaven work)?
