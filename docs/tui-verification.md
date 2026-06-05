# TUI Verification

Use this checklist for TUI-facing changes before opening a PR or closing a TUI issue.

## Automated Checks

Run:

```sh
cargo fmt --all
cargo test -p walls tui:: --features tui-preview
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
scripts/validate-tui-pty.sh
scripts/verify-tui-visual.sh
```

## Manual/Visual Checks

- Standard terminal, dark background: `walls tui`
- Light-background terminal or theme: `WALLS_TUI_COLOR=never walls tui`
- Narrow terminal around `42x10`: confirm mode/status and `q` remain visible.
- Wide terminal around `120x32`: confirm the `Now` tab keeps metadata and preview/fallback regions stable.
- Preview disabled: `WALLS_TUI_PREVIEW=0 walls tui`
- Unknown terminal fallback: run with a normal `xterm-256color`-style environment and confirm no raw image escape sequences are shown.
- Ghostty: run in Ghostty with preview enabled. It should probe the Kitty-compatible graphics path when available, or show `preview unavailable: ...; showing metadata` without corrupting the UI.

## Environment Knobs

- `WALLS_TUI_COLOR=never` disables colour and keeps state visible through text and modifiers.
- `WALLS_TUI_PREVIEW=0`, `false`, `no`, `off`, `never`, or `metadata` forces metadata-only preview mode.

Record the commands and terminal sizes used in the PR when changing layout, colour, key handling, or preview behaviour.
