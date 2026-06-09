# Demo recordings

| Script | Output | Use |
|--------|--------|-----|
| `record-cli.sh` | `demo.gif`, `demo-cli.cast` | README — CLI apply/status/pause (asciinema + agg) |
| `render-tui-gif.sh` | `demo-tui.gif` | README — deterministic TUI-first showcase generated from real Ratatui output |
| `record-tui.sh` | `demo-tui.gif` | Manual PR/release demos — guided TUI screen capture (gpu-screen-recorder + ffmpeg) |
| `demo-cli.sh` | — | Asciinema driver (typed commands) |

Regenerate the README TUI demo:

```bash
cargo build -p walls
./demo/render-tui-gif.sh
```

The renderer drives `walls tui` in a PTY with an isolated demo config, captures
the alternate-screen states, and renders them to a GIF. It avoids recording the
current desktop, so the checked-in artifact starts from a blank, reproducible
canvas.

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
