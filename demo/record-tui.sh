#!/usr/bin/env bash
# Record walls tui for README (asciinema cannot render ratatui alternate screen).
#
# On COSMIC/GNOME/KDE, use gpu-screen-recorder via xdg-desktop-portal:
#   nix-shell -p gpu-screen-recorder ffmpeg --run ./demo/record-tui.sh
#
# Or record manually: open a terminal, run walls tui, press 3 (Browse), q (quit).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "walls TUI uses the terminal alternate screen."
echo "Binary: ${WALLS_BIN:-$ROOT/target/release/walls} tui"
echo "Validate headless: $ROOT/scripts/validate-tui-pty.sh"
echo
echo "Suggested manual capture (~15s):"
echo "  1. walls tui"
echo "  2. Keys: 3 (Browse) → j/k → 1 (Status) → n (next) → q (quit)"
echo "  3. gpu-screen-recorder -w portal -f 24 -o /tmp/walls-tui.mp4"
echo "  4. ffmpeg -i /tmp/walls-tui.mp4 -vf 'fps=12,scale=800:-1:flags=lanczos' -loop 0 demo-tui.gif"
