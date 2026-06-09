# Demo recordings

| Script | Output | Use |
|--------|--------|-----|
| `record-cli.sh` | `demo.gif`, `demo-cli.cast` | README — CLI apply/status/pause (asciinema + agg) |
| `capture-tui-showcase.sh` | `showcase-capture/*.png`, `showcase-capture/demo-tui.gif` | README candidate — real COSMIC desktop, tray, transparent Ghostty TUI, wallpaper switch |
| `render-tui-gif.sh` | `demo-tui.gif` | Inspection fallback — deterministic TUI-first showcase generated from real Ratatui output |
| `record-tui.sh` | `demo-tui.gif` | Manual PR/release demos — guided TUI screen capture (gpu-screen-recorder + ffmpeg) |
| `demo-cli.sh` | — | Asciinema driver (typed commands) |

Capture the README TUI showcase from a real COSMIC desktop:

```bash
cargo build -p walls -p walls-tray
./demo/capture-tui-showcase.sh
```

Run it from the clean capture workspace/window. On Will's COSMIC setup,
workspace 3 is reserved for this. The script creates an isolated demo config,
starts a dedicated `walls-tray`, opens `walls tui` in a transparent Ghostty
window, switches a generated COSMIC wallpaper, and writes verification notes to
`demo/showcase-capture/verification.txt`.

Review the PNG frames and GIF before copying any artifact into the README. The
recording is not acceptable if it shows unrelated apps, misses the tray icon,
does not visibly switch wallpaper, or captures a non-TUI terminal instead of the
Ghostty TUI.

Generate the deterministic TUI fallback:

```bash
cargo build -p walls
./demo/render-tui-gif.sh
```

The renderer drives `walls tui` in a PTY with an isolated demo config, captures
the alternate-screen states, and renders them to a GIF. It avoids recording the
current desktop, so it is useful for debugging rendering but does not prove tray
or wallpaper behaviour.

Regenerate the CLI demo:

```bash
nix-shell -p asciinema asciinema-agg --run './demo/record-cli.sh'
```

Record a live TUI desktop/window demo from an interactive graphical session:

```bash
nix-shell -p gpu-screen-recorder ffmpeg --run './demo/record-tui.sh'
```

Use this only when you need a real portal/window recording. Start it from an
empty workspace/window before launching `walls tui`; on Will's COSMIC setup,
workspace 3 is reserved as the clean capture canvas.

Headless TUI check (PTY, no pixels):

```bash
./scripts/validate-tui-pty.sh
```

Requires `target/release/walls` or `target/debug/walls`.
