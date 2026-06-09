# TUI Design And Verification

`walls` (no args) or `walls tui` starts the keyboard-first Ratatui surface for repeated wallpaper management. Keep it quiet, dense, and predictable.

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

## Design Language

The TUI is a daily-driver wallpaper control surface. It should feel like a compact operational dashboard: dense, calm, predictable, and fast to scan. It is not a marketing screen, a splash page, or a decorative demo surface.

Keep the first screen useful:

- Show current wallpaper state, source health, queue/cache state, and available actions before ornamental content.
- Prefer persistent footer/status affordances over instructional prose inside the tab body.
- Keep tab bodies compact enough that narrow terminals still expose the focused block and the quit path.
- Use decoration only when it improves parsing, such as borders, separators, selected-row markers, or status labels.

Use this hierarchy:

- Chrome: top-level tabs, footer keys, and persistent frame elements.
- Tab body: operational content for the active tab.
- Focus: the selected row, active edit field, or command/search input target.
- Secondary metadata: paths, counts, hints, provider details, and fallback text.
- Validation/status: errors, warnings, success, and neutral progress or result messages.

Shape language:

- Borders frame chrome and tab bodies; avoid nested decorative boxes.
- Separators group dense edit forms and lists when spacing alone is too expensive.
- Markers such as `▸` identify focus; do not rely on colour alone.
- Unicode is appropriate for compact operational symbols (`▸`, `✓`, `✗`, separators) when the plain text remains understandable in PTY output and tests.
- Align repeated labels and values so list scanning does not depend on colour.

## Style Tokens

Use semantic styles from `crates/cli/src/tui/style.rs`.

- `Theme::chrome_block` frames persistent chrome such as top tabs and footers.
- `Theme::content_block` frames the active tab body, preview panes, and focused tool surfaces.
- `Theme::normal` is ordinary readable body content.
- `Theme::muted` is secondary metadata, unavailable text, separators, and low-priority hints.
- `Theme::accent` highlights titles and primary labels that establish hierarchy.
- `Theme::heading` is a colour-neutral strong label for enabled names and compact section text.
- `Theme::active_state` and `Theme::inactive_state` are for enabled/off availability state, not operation outcomes.
- `Theme::boolean_true` and `Theme::boolean_false` are for config boolean values; `false` must not imply an error.
- `Theme::unavailable` is for unavailable but actionable states such as locked fields or missing capabilities.
- `Theme::selected` is for selected list rows, tabs, and command targets.
- `Theme::edit_focus_row`, `Theme::edit_focus_label`, and `Theme::edit_focus_value` are only for the active edit-form row.
- `Theme::border` is the default border treatment for blocks.
- `Theme::key_hint` is for compact keyboard affordances in chrome or status areas.
- `Theme::status` is for neutral, success, warning, and error states.

Do not add raw Ratatui colours in render code unless the semantic style module cannot express the state yet. Extend `Theme` first, then update this section so future render code has a named purpose to reuse. One-off `Style::default()` usage is acceptable only for unstyled normal text or inside the style module itself.

`WALLS_TUI_COLOR=never` disables colour. Important states must remain legible through labels, markers, and modifiers, not colour alone. Any new token must define a no-colour representation before it is used.

Colour is allowed to speed up scanning, but it must not be the only carrier of meaning:

- Focus must also use a marker, reverse video, or both (`>`, `▸`, selected row styling).
- Empty, disabled, unavailable, missing configuration, warning, error, and loading rows must include a text label such as `[empty]`, `[missing]`, or `[warning]`.
- Validation errors must keep the `!!` cue and include the affected config path plus recovery hint where available.
- Boolean/config state must use words such as `on`, `off`, `true`, `false`, or `unavailable`, not colour alone.
- Muted and disabled text should remain readable on dark and light terminal themes. Prefer the terminal foreground plus `DIM` over fixed low-contrast foreground colours for body text.
- Decorative borders and separators may use quieter styling because they do not carry critical state by themselves.

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

Wide layouts render previews for the current Now wallpaper and for the selected History/Browse item. Preview rendering uses normalized thumbnail files under the walls cache directory before converting the image into the terminal-specific protocol; terminal protocol output is still produced by the TUI because it depends on the active terminal and render area.

Use `WALLS_TUI_PREVIEW=0`, `false`, `no`, `off`, `never`, or `metadata` for metadata-only mode.

## Library Semantics

History is the applied-wallpaper timeline. Browse is the candidate surface for
queued provider items, local source results, and other selectable wallpaper
rows. A Browse row for a queued Wallhaven or Unsplash item can show a preview
when its provider cache file already exists; otherwise the row remains usable as
metadata.

`download_dir` and `cache_dir` are provider-managed storage. `fetched_dir` is
the user-imported local library populated by `walls fetch` and source entries of
type `fetched`; provider reset actions must not remove fetched files or fetched
history entries.

## Verification

Before closing TUI work, run the checks in [`tui-verification.md`](tui-verification.md). Layout, colour, key handling, and preview changes require both automated tests and PTY visual/behavioural verification.

## Config editing (drill-down, non-modal)
`e` on a Config block or (after `Enter` to subnav on the Sources list) on a provider item enters focused edit: main content replaced by the form for the item (stable layout, no overlay/popup). Arrow up/down move fields, type edits text fields (live `val|buffer`), left/right or Space cycle choice fields, `Enter` commits and saves the current field, and `Esc` returns to the list. Run `walls config validate` for a full config check.

Wide (tui-preview): split list-context | form (like Now preview split). Narrow: full form.

Nested providers: "Sources" block shows the configured sources vec, subnav j/k or arrow up/down pick, Home/End jump, PageUp/PageDown page, `a` adds a Wallhaven query source and opens it for editing, `x` removes the selected configured source, and `e` edits the chosen SourceEntry (type-aware fields like path/query/url/image_path/api_key, designed per schema). Extended provider examples live in `config.sources.example.json` instead of the first-run config. Reuses atomic save + Reload + existing validation.

### Config tab field coverage matrix

Use this matrix when expanding the Config tab. It inventories the persisted
`config.json` and `secrets.json` fields, then classifies whether the TUI edits
the field directly, shows it as read-only context, leaves it to manual file edit,
or deliberately defers it to a later focused slice.

| Field path | Coverage | Notes |
| --- | --- | --- |
| `config.$schema` | Manual | Schema hint for external editors; keep out of the TUI edit flow. |
| `config.change.enabled` | Editable | Rotation block boolean. |
| `config.change.on_start` | Editable | Rotation block boolean. |
| `config.change.interval_secs` | Editable | Rotation block numeric text field. |
| `config.change.internet_enabled` | Editable | Rotation block boolean. |
| `config.change.safe_mode` | Editable | Rotation block boolean. |
| `config.change.change_lock_screen` | Editable | Rotation block boolean. |
| `config.change.download_preference_ratio` | Editable | Rotation block numeric text field. |
| `config.paths.cache_dir` | Read-only | Library block context; manual path edits avoid moving user data unexpectedly. |
| `config.paths.download_dir` | Read-only | Library block context; manual path edits avoid moving user data unexpectedly. |
| `config.paths.favorites_dir` | Read-only | Library block context; manual path edits avoid moving user data unexpectedly. |
| `config.paths.fetched_dir` | Read-only | Library block context; manual path edits avoid moving user data unexpectedly. |
| `config.paths.compose_dir` | Read-only | Display/apply context; manual path edits avoid moving generated wallpaper files unexpectedly. |
| `config.quota.enabled` | Editable | Library edit form boolean. |
| `config.quota.size_mb` | Editable | Library edit form numeric text field with inline validation. |
| `config.apply.backend` | Editable | Apply/display edit form choice field with inline validation. |
| `config.apply.cosmic.method` | Editable | Apply/display edit form choice field. |
| `config.apply.cosmic.config_path` | Editable | Apply/display edit form text field with inline validation when the COSMIC backend uses it. |
| `config.apply.cosmic.use_original_path` | Editable | Apply/display edit form boolean. |
| `config.apply.cosmic.entry.rotation_frequency` | Manual | COSMIC-specific low-level patch field. |
| `config.apply.cosmic.entry.filter_by_theme` | Manual | COSMIC-specific low-level patch field. |
| `config.apply.custom_script` | Editable | Apply/display edit form text field with existing executable validation for the custom-script backend. |
| `config.display.mode` | Editable | Apply/display edit form choice field. |
| `config.display.auto_rotate` | Editable | Apply/display edit form boolean. |
| `config.display.imagemagick_command` | Editable | Apply/display edit form text field. |
| `config.display.target_width` | Editable | Apply/display edit form numeric text field; width and height validate as a pair. |
| `config.display.target_height` | Editable | Apply/display edit form numeric text field; width and height validate as a pair. |
| `config.display.filters.enabled` | Editable | Apply/display edit form boolean. |
| `config.display.filters.command` | Editable | Apply/display edit form text field. |
| `config.display.filters.filters[].name` | Manual | Multi-row ImageMagick filter editing remains file-based. |
| `config.display.filters.filters[].args` | Manual | Multi-row ImageMagick filter editing remains file-based. |
| `config.selection.use_landscape_enabled` | Editable | Library edit form boolean. |
| `config.selection.avoid_recent` | Editable | Library edit form numeric text field. |
| `config.selection.refetch_when_cache_below` | Editable | Library edit form numeric text field. |
| `config.selection.strategy` | Editable | Config block cycle action persists random/sequential. |
| `config.tray.accent` | Editable | Rotation edit form choice field. |
| `config.tray.autostart.desktops.*` | Editable | Rotation edit form toggles the current desktop entry when supported. |
| `config.tui.key_profile` | Editable | TUI edit form choice field for Emacs/Vim key profiles. |
| `config.sources[].enabled` | Editable | Source edit form boolean. |
| `config.sources[].type` | Editable | Source edit form choice field for source kinds. |
| `config.sources[].label` | Editable | Source edit form text field when the source kind persists labels. |
| `config.sources[].path` | Editable | Folder/image source text field. |
| `config.sources[].query` | Editable | Reddit, Unsplash, Weighting, Wallhaven, and Pixabay text field. |
| `config.sources[].url` | Editable | JSON, Media RSS, Attribution, Unsplash, and Immich text field. |
| `config.sources[].collection` | Editable | Unsplash text field. |
| `config.sources[].user` | Editable | Unsplash text field. |
| `config.sources[].topic` | Editable | Unsplash text field. |
| `config.sources[].orientation` | Editable | Unsplash choice field. |
| `config.sources[].api_key` | Editable | Pixabay/Immich inline source key; secrets-backed providers stay in `secrets.json`. |
| `config.sources[].image_path` | Editable | JSON source text field. |
| `config.sources[].title_path` | Deferred | Legacy schema field; source normalization does not persist it from TUI edits. |
| `config.sources[].sort` | Editable | Reddit choice field. |
| `config.sources[].time` | Editable | Reddit choice field when the selected sort uses time. |
| `config.sources[].categories` | Editable | Wallhaven source category booleans write the bit string. |
| `config.sources[].purity` | Editable | Wallhaven source purity booleans write the bit string; NSFW depends on API key presence. |
| `config.sources[].sorting` | Editable | Wallhaven sorting choice field. |
| `config.sources[].order` | Editable | Wallhaven order choice field. |
| `config.sources[].ratios` | Editable | Wallhaven aspect ratio choice field. |
| `config.sources[].atleast` | Editable | Wallhaven minimum resolution choice field. |
| `config.sources[].prefer` | Editable | Wallhaven collection/search preference choice field. |
| `config.sources[].collections` | Manual | Wallhaven collection list is persisted on each Wallhaven source. |
| `config.sources[].collections[].username` | Manual | Wallhaven collection username. |
| `config.sources[].collections[].id` | Manual | Wallhaven collection id. |
| `config.sources[].collections[].label` | Manual | Optional Wallhaven collection label. |
| `config.sources[].source` | Deferred | Attribution metadata schema field; source normalization does not persist it from TUI edits yet. |
| `config.sources[].author` | Deferred | Attribution metadata schema field; source normalization does not persist it from TUI edits yet. |
| `secrets.$schema` | Manual | Schema hint for external editors. |
| `secrets.wallhaven_api_key` | Manual | TUI shows presence/hints only; edit `secrets.json` directly. |
| `secrets.unsplash_access_key` | Manual | TUI shows presence/hints only; edit `secrets.json` directly. |
| `secrets.reddit_client_id` | Manual | TUI shows presence/hints only; edit `secrets.json` directly. |
| `secrets.reddit_client_secret` | Manual | TUI shows presence/hints only; edit `secrets.json` directly. |

Normal mode navigation:

- Left/right switch top-level tabs and leave Config Sources subnav.
- Number keys `1` through `6` jump directly to visible tabs.
- `?` opens in-app key help in normal mode; Esc or `q` closes help without quitting.
- `/` enters Search input from any top-level tab; `/`, `:`, and `q` remain normal typed characters while command, search, or config edit input is active.
- `j/k` and arrow up/down move within the active list.
- Home/End jump to the first/last row for History, Browse, Search, Config blocks, and Config Sources subnav.
- PageUp/PageDown jump five rows and clamp at list boundaries.
- Destructive actions are two-step: `d` requests trash for the current wallpaper, then `d` confirms or Esc cancels; Shift+X requests provider storage reset, then Shift+X confirms or Esc cancels. Provider reset clears the queue, deletes files under `cache_dir` and `download_dir`, prunes provider-backed History/current state, and leaves `fetched_dir` alone. Other keys are ignored while a destructive confirmation is pending.
