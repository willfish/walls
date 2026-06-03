# Demo recordings

| Script | Output | Use |
|--------|--------|-----|
| `record-cli.sh` | `demo.gif`, `demo-cli.cast` | README — CLI apply/status/pause (asciinema + agg) |
| `record-tui.sh` | (manual) | Instructions for TUI screen capture |
| `demo-cli.sh` | — | Asciinema driver (typed commands) |

Headless TUI check (PTY, no pixels):

```bash
./scripts/validate-tui-pty.sh
```

Requires `target/release/walls` or `target/debug/walls`.
