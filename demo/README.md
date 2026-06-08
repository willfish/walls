# Demo recordings

| Script | Output | Use |
|--------|--------|-----|
| `record-cli.sh` | `demo.gif`, `demo-cli.cast` | README — CLI apply/status/pause (asciinema + agg) |
| `record-tui.sh` | `demo-tui.gif` | README/PRs — guided TUI screen capture (gpu-screen-recorder + ffmpeg) |
| `demo-cli.sh` | — | Asciinema driver (typed commands) |

Regenerate the CLI demo:

```bash
nix-shell -p asciinema asciinema-agg --run './demo/record-cli.sh'
```

Record the TUI demo from an interactive graphical session:

```bash
nix-shell -p gpu-screen-recorder ffmpeg --run './demo/record-tui.sh'
```

Headless TUI check (PTY, no pixels):

```bash
./scripts/validate-tui-pty.sh
```

Requires `target/release/walls` or `target/debug/walls`.
