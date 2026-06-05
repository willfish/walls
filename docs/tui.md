# TUI Design And Verification

`walls tui` is a keyboard-first Ratatui surface for repeated wallpaper management. Keep it quiet, dense, and predictable.

## Architecture

The TUI follows a model/update/view shape:

```text
terminal event -> UiAction -> update App -> UpdateEffect -> render App
```

- `App` in `crates/cli/src/tui/app.rs` is the model. It owns the loaded `WallsCtx`, selected tab, cursor, input mode, command/search text, search results, local candidates, status message, and colour mode.
- `UiAction` in `crates/cli/src/tui/mod.rs` is the input boundary. Key events are translated to actions before any state changes happen.
- `update` applies actions to the model and returns an `UpdateEffect`. Effects make follow-up work explicit, currently `Reload` or `Quit`.
- Render helpers in `crates/cli/src/tui/mod.rs` project the model into widgets. They should not read files, call APIs, mutate state, or inspect real time.
- Optional image rendering lives in `crates/cli/src/tui/preview.rs` and must stay a progressive enhancement. Metadata and controls must remain useful without graphics support.

## Styles

Use semantic styles from `crates/cli/src/tui/style.rs`.

- `Theme::chrome_block` for top/footer chrome.
- `Theme::content_block` for tab bodies and preview panes.
- `Theme::selected` for selected rows and tabs.
- `Theme::status` for neutral, success, warning, and error states.
- `Theme::key_hint` for compact keyboard affordances.

Do not add raw Ratatui colours in render code unless the semantic style module cannot express the state yet. Extend the style module first.

`WALLS_TUI_COLOR=never` disables colour. Important states must remain legible through labels and modifiers, not colour alone.

## Layout Contracts

Supported terminal size classes are encoded in `terminal_size`:

- `Tiny`: below `10x6`; render nothing rather than corrupting the screen.
- `Narrow`: below `50` columns or `12` rows; use one content column and compact footer keys.
- `Standard`: normal one-column operation with full key hints.
- `Wide`: at least `100x18`; the `Now` tab can split metadata and preview/fallback panes.

Long paths and status text may be clipped by the terminal, but they must not push persistent controls off screen. `q` must remain visible in narrow normal/search modes.

## Preview Capability

Preview capability is decided in two stages:

1. Environment hints decide whether to probe image protocols at all.
2. `ratatui-image` queries the terminal and chooses the concrete protocol.

Ghostty and Kitty are treated as Kitty-graphics candidates. iTerm is treated as an iTerm2 candidate. Unknown terminals and explicit preview-disable settings show metadata-only fallback text.

Use `WALLS_TUI_PREVIEW=0`, `false`, `no`, `off`, `never`, or `metadata` for metadata-only mode.

## Verification

Before closing TUI work, run the checks in [`tui-verification.md`](tui-verification.md). Layout, colour, key handling, and preview changes require both automated tests and PTY visual/behavioural verification.
