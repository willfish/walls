#!/usr/bin/env bash
# Record demo/demo-tui.gif for the Ratatui interface.
#
# This needs an interactive graphical session because gpu-screen-recorder uses
# xdg-desktop-portal to select the terminal/window to capture.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEMO_DIR="$ROOT/demo"
WALLS="${WALLS_BIN:-$ROOT/target/release/walls}"
[[ -x $WALLS ]] || WALLS="$ROOT/target/debug/walls"

if [[ ! -x $WALLS ]]; then
	echo "Build walls first, e.g.: cargo build -p walls --release"
	exit 1
fi

for bin in gpu-screen-recorder ffmpeg; do
	if ! command -v "$bin" >/dev/null; then
		echo "Need $bin, e.g.: nix-shell -p gpu-screen-recorder ffmpeg --run '$0'"
		exit 1
	fi
done

TMP="$(mktemp -d)"
VIDEO_FILE="${VIDEO_FILE:-$TMP/walls-tui.mp4}"
GIF_FILE="${GIF_FILE:-$DEMO_DIR/demo-tui.gif}"
CONFIG_HOME="$TMP/config"
STATE_HOME="$TMP/state"

cleanup() {
	if [[ ${RECORDER_PID:-} ]]; then
		kill -INT "$RECORDER_PID" 2>/dev/null || true
		wait "$RECORDER_PID" 2>/dev/null || true
	fi
	rm -rf "$TMP"
}
trap cleanup EXIT

mkdir -p "$CONFIG_HOME/walls" "$STATE_HOME/walls" "$TMP/images" "$TMP/cache"
echo '{}' >"$CONFIG_HOME/walls/secrets.json"
printf 'demo-a\n' >"$TMP/images/aurora.jpg"
printf 'demo-b\n' >"$TMP/images/city.jpg"

NOOP="$TMP/noop.sh"
cat >"$NOOP" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$NOOP"

cat >"$CONFIG_HOME/walls/config.json" <<EOF
{
  "change": { "enabled": true, "internet_enabled": false },
  "paths": {
    "cache_dir": "$TMP/cache",
    "download_dir": "$TMP/downloaded",
    "favorites_dir": "$TMP/favorites",
    "fetched_dir": "$TMP/fetched",
    "compose_dir": "$TMP/wallpaper"
  },
  "apply": { "backend": "custom-script", "custom_script": "$NOOP" },
  "display": { "mode": "os" },
  "sources": [{ "enabled": true, "type": "folder", "path": "$TMP/images" }]
}
EOF

cat <<EOF
walls TUI demo recorder

Output: $GIF_FILE
Binary: $WALLS tui

Suggested capture:
  1. Select this terminal/window in the portal picker.
  2. When the TUI opens, press:
     3  Browse
     j/k move
     1  Status
     n  next wallpaper
     q  quit

EOF

read -r -p "Press Enter to open the portal picker and start recording..."
gpu-screen-recorder -w portal -f 24 -o "$VIDEO_FILE" >/dev/null 2>&1 &
RECORDER_PID=$!

read -r -p "After selecting the terminal/window, press Enter to launch walls tui..."

if ! kill -0 "$RECORDER_PID" 2>/dev/null; then
	echo "Recording failed to start"
	exit 1
fi

XDG_CONFIG_HOME="$CONFIG_HOME" \
	XDG_STATE_HOME="$STATE_HOME" \
	RUST_BACKTRACE=0 \
	"$WALLS" tui || true

kill -INT "$RECORDER_PID" 2>/dev/null || true
wait "$RECORDER_PID" 2>/dev/null || true
RECORDER_PID=""

ffmpeg -y -i "$VIDEO_FILE" \
	-vf "fps=12,scale=800:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer" \
	-loop 0 "$GIF_FILE"

echo "Wrote $GIF_FILE ($(wc -c <"$GIF_FILE") bytes)"
