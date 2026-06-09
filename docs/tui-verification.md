# TUI Verification

Use this checklist for TUI-facing changes before opening a PR or closing a TUI issue.

## Automated Checks

Run:

```sh
cargo fmt --all
cargo test -p walls tui::
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
scripts/validate-tui-pty.sh
scripts/verify-tui-visual.sh
```

## Manual/Visual Checks

- Standard terminal, dark background: `walls` (or `walls tui`)
- Light-background terminal or theme: `WALLS_TUI_COLOR=never walls`
- No-colour journeys: confirm Config source warnings/missing credentials, Search empty state, edit validation errors, and footer status remain understandable through text labels, markers, and modifiers.
- Config edit (drill-down): Config tab, j/k to Sources, Enter subnav, j/k pick provider, `e` -> form replaces main content. In edit mode, arrow up/down move fields, text keys type into text fields, Space or left/right cycle boolean/choice fields, Enter commits/saves, and Esc cancels/back. Test narrow 50x12, WALLS_TUI_COLOR=never (legible), no stale cells.
- Normal navigation: left/right switch tabs; `?` opens key help and Esc/`q` closes it; `/` opens Search input; Home/End and PageUp/PageDown move History, Browse, Search results, Config blocks, and Config Sources subnav without affecting command/search/edit text entry.
- Destructive confirmations: with a current wallpaper, press `d` and confirm the prompt names the target; unrelated keys do not perform unrelated actions, Esc cancels, and a second `d` confirms. Shift+X nuke still requires a second Shift+X and Esc cancels.
- Narrow terminal around `42x10`: confirm mode/status and `q` remain visible.
- Wide terminal around `120x32`: confirm the `Now` tab keeps metadata and preview/fallback regions stable.
- Preview disabled: `WALLS_TUI_PREVIEW=0 walls`
- Unknown terminal fallback: run with a normal `xterm-256color`-style environment and confirm no raw image escape sequences are shown.
- Ghostty: run in Ghostty with preview enabled. It should probe the Kitty-compatible graphics path when available, or show `preview unavailable: ...; showing metadata` without corrupting the UI.

## Environment Knobs

- `WALLS_TUI_COLOR=never` disables colour and keeps state visible through text, focus markers, and modifiers.
- `WALLS_TUI_PREVIEW=0`, `false`, `no`, `off`, `never`, or `metadata` forces metadata-only preview mode.

Record the commands and terminal sizes used in the PR when changing layout, colour, key handling, or preview behaviour.
