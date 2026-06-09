#!/usr/bin/env bash
# Capture a real COSMIC/Ghostty TUI showcase from a clean desktop workspace.
#
# Run this from the empty capture workspace (workspace 3 on Will's COSMIC setup).
# The script launches an isolated walls config, starts a dedicated tray process,
# opens the real TUI in a transparent Ghostty window, captures desktop
# screenshots, and restores the original COSMIC wallpaper config on exit.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEMO_DIR="$ROOT/demo"
OUT_DIR="${OUT_DIR:-$DEMO_DIR/showcase-capture}"
WALLS="${WALLS_BIN:-$ROOT/target/release/walls}"
WALLS_TRAY="${WALLS_TRAY_BIN:-$ROOT/target/release/walls-tray}"
[[ -x $WALLS ]] || WALLS="$ROOT/target/debug/walls"
[[ -x $WALLS_TRAY ]] || WALLS_TRAY="$ROOT/target/debug/walls-tray"

COSMIC_BG_CONFIG="${COSMIC_BG_CONFIG:-$HOME/.config/cosmic/com.system76.CosmicBackground/v1/all}"
WORKSPACE_HINT="${WORKSPACE_HINT:-workspace 3}"
AUTO="${AUTO:-0}"
COMMIT_ARTIFACTS="${COMMIT_ARTIFACTS:-0}"

for bin in cosmic-screenshot ghostty magick python3 rg; do
	if ! command -v "$bin" >/dev/null; then
		echo "Need $bin"
		exit 1
	fi
done

if [[ ! -x $WALLS || ! -x $WALLS_TRAY ]]; then
	echo "Build walls and walls-tray first, e.g.: cargo build -p walls -p walls-tray"
	exit 1
fi

if [[ ! -f $COSMIC_BG_CONFIG ]]; then
	echo "COSMIC background config not found: $COSMIC_BG_CONFIG"
	exit 1
fi

TMP="$(mktemp -d)"
CONFIG_HOME="$TMP/config"
STATE_HOME="$TMP/state"
IMAGE_DIR="$TMP/demo-wallpapers"
BACKUP="$TMP/cosmic-background.ron"
VERIFY="$OUT_DIR/verification.txt"
TRAY_PID=""
GHOSTTY_PID=""

cleanup() {
	if [[ -n $GHOSTTY_PID ]]; then
		kill "$GHOSTTY_PID" 2>/dev/null || true
	fi
	if [[ -n $TRAY_PID ]]; then
		kill "$TRAY_PID" 2>/dev/null || true
	fi
	if [[ -f $BACKUP ]]; then
		cp "$BACKUP" "$COSMIC_BG_CONFIG"
	fi
	rm -rf "$TMP"
}
trap cleanup EXIT

mkdir -p "$CONFIG_HOME/walls" "$STATE_HOME/walls" "$IMAGE_DIR" \
	"$TMP/cache" "$TMP/downloaded" "$TMP/favorites" "$TMP/fetched" "$TMP/wallpaper" "$OUT_DIR"
cp "$COSMIC_BG_CONFIG" "$BACKUP"
: >"$VERIFY"

DEMO_FONT="${DEMO_FONT:-DejaVu-Sans-Bold}"

magick -size 2880x1920 gradient:'#08203a-#7b1e3c' \
	-font "$DEMO_FONT" \
	-fill '#f7d36b' -gravity northwest -pointsize 160 -annotate +180+180 'walls demo' \
	"$IMAGE_DIR/aurora-field.jpg"
magick -size 2880x1920 gradient:'#111827-#3b82f6' \
	-font "$DEMO_FONT" \
	-fill '#e5e7eb' -gravity southeast -pointsize 140 -annotate +180+180 'next wallpaper' \
	"$IMAGE_DIR/city-lights.jpg"
magick -size 2880x1920 gradient:'#052e16-#65a30d' \
	-font "$DEMO_FONT" \
	-fill '#ecfccb' -gravity center -pointsize 150 -annotate +0+0 'browse source' \
	"$IMAGE_DIR/forest-path.jpg"

cat >"$CONFIG_HOME/walls/secrets.json" <<'JSON'
{}
JSON
chmod 0600 "$CONFIG_HOME/walls/secrets.json"

cat >"$CONFIG_HOME/walls/config.json" <<JSON
{
  "change": {
    "enabled": true,
    "on_start": false,
    "interval_secs": 300,
    "internet_enabled": false,
    "safe_mode": false,
    "change_lock_screen": false,
    "download_preference_ratio": 0.9
  },
  "paths": {
    "cache_dir": "$TMP/cache",
    "download_dir": "$TMP/downloaded",
    "favorites_dir": "$TMP/favorites",
    "fetched_dir": "$TMP/fetched",
    "compose_dir": "$TMP/wallpaper"
  },
  "quota": { "enabled": true, "size_mb": 1000 },
  "apply": {
    "backend": "cosmic",
    "cosmic": {
      "method": "cosmic-config",
      "config_path": "$COSMIC_BG_CONFIG",
      "use_original_path": true,
      "entry": {
        "rotation_frequency": 0,
        "filter_by_theme": false
      }
    },
    "custom_script": null
  },
  "display": { "mode": "os" },
  "wallhaven": {
    "enabled": false,
    "collections": [],
    "search": {
      "q": "space",
      "categories": "111",
      "purity": "100",
      "sorting": "random",
      "order": "desc",
      "atleast": "1920x1080"
    },
    "prefer": "collections_then_search"
  },
  "sources": [
    {
      "enabled": true,
      "type": "folder",
      "label": "Showcase wallpapers",
      "path": "$IMAGE_DIR"
    }
  ]
}
JSON

current_wallpaper_source() {
	rg --only-matching 'source: Path\("[^"]+"\)' "$COSMIC_BG_CONFIG" | head -1 || true
}

shot() {
	local name="$1"
	local before
	before="$(date +%s)"
	cosmic-screenshot --interactive=false --modal=false --notify=false --save-dir "$OUT_DIR" >/dev/null
	local latest
	latest="$(
		find "$OUT_DIR" -maxdepth 1 -type f -name 'Screenshot_*.png' -printf '%T@ %p\n' |
			sort -nr |
			head -1 |
			cut -d' ' -f2-
	)"
	if [[ -z $latest || ! -f $latest ]]; then
		echo "No screenshot was created for $name"
		exit 1
	fi
	cp "$latest" "$OUT_DIR/$name.png"
	printf '%s captured %s after %s\n' "$name" "$OUT_DIR/$name.png" "$before" >>"$VERIFY"
	echo "Captured $OUT_DIR/$name.png"
}

cat <<EOF
walls TUI showcase capture

Output directory: $OUT_DIR

Before continuing:
  1. Switch to clean $WORKSPACE_HINT.
  2. Close unrelated windows and notifications on that workspace.
  3. Run this script from that workspace.

This script temporarily patches:
  $COSMIC_BG_CONFIG

The original file is restored when the script exits.
EOF

if [[ $AUTO != 1 ]]; then
	read -r -p "Press Enter only when the clean capture workspace is ready..."
fi

XDG_CONFIG_HOME="$CONFIG_HOME" XDG_STATE_HOME="$STATE_HOME" "$WALLS" apply "$IMAGE_DIR/aurora-field.jpg" >/dev/null
INITIAL_SOURCE="$(current_wallpaper_source)"
printf 'initial_wallpaper=%s\n' "$INITIAL_SOURCE" >>"$VERIFY"

env \
	XDG_CONFIG_HOME="$CONFIG_HOME" \
	XDG_STATE_HOME="$STATE_HOME" \
	WALLS_TRAY=1 \
	"$WALLS_TRAY" >"$OUT_DIR/walls-showcase-tray.log" 2>&1 &
TRAY_PID=$!
sleep 1

if busctl --user list | rg -q "org\\.kde\\.StatusNotifierItem-$TRAY_PID-"; then
	printf 'tray_status_notifier=registered pid=%s\n' "$TRAY_PID" >>"$VERIFY"
else
	printf 'tray_status_notifier=not-found pid=%s\n' "$TRAY_PID" >>"$VERIFY"
fi

TUI_RUNNER="$TMP/run-tui.sh"
cat >"$TUI_RUNNER" <<EOF
#!/usr/bin/env bash
export XDG_CONFIG_HOME="$CONFIG_HOME"
export XDG_STATE_HOME="$STATE_HOME"
export WALLS_TUI_INTRO=1
export WALLS_TUI_PREVIEW=0
export WALLS_TRAY=1
exec "$WALLS" tui
EOF
chmod +x "$TUI_RUNNER"

ghostty \
	--gtk-single-instance=false \
	--background-opacity=0.72 \
	--window-decoration=false \
	--confirm-close-surface=false \
	--font-size=15 \
	--theme='Catppuccin Mocha' \
	-e "$TUI_RUNNER" >"$OUT_DIR/walls-showcase-ghostty.log" 2>&1 &
GHOSTTY_PID=$!

cat <<'EOF'

Ghostty should now show the real TUI over the COSMIC desktop.

Capture sequence:
  1. Leave the TUI on its landing/intro state, then press Enter here.
  2. In the TUI press 4 for Browse, then press Enter here.
  3. In the TUI press 2, then n to switch wallpaper, then press Enter here.
  4. In the TUI press ? for key help, then press Enter here.
  5. Review the PNG/GIF before committing any artifact.

EOF

read -r -p "Frame 1: press Enter when the TUI landing is visible..."
shot "01-tui-landing"
read -r -p "Frame 2: press 4 in the TUI for Browse, then Enter here..."
shot "02-browse"
read -r -p "Frame 3: press 2 then n in the TUI to switch wallpaper, then Enter here..."
SWITCHED_SOURCE="$(current_wallpaper_source)"
printf 'switched_wallpaper=%s\n' "$SWITCHED_SOURCE" >>"$VERIFY"
shot "03-wallpaper-switched"
read -r -p "Frame 4: press ? in the TUI for help, then Enter here..."
shot "04-help"

wait "$GHOSTTY_PID" 2>/dev/null || true
GHOSTTY_PID=""

if [[ -n $INITIAL_SOURCE && -n $SWITCHED_SOURCE && $INITIAL_SOURCE == "$SWITCHED_SOURCE" ]]; then
	echo "Wallpaper source did not change during capture"
	exit 1
fi

magick -delay 120 -loop 0 \
	"$OUT_DIR/01-tui-landing.png" \
	"$OUT_DIR/02-browse.png" \
	"$OUT_DIR/03-wallpaper-switched.png" \
	"$OUT_DIR/04-help.png" \
	-resize 1200x \
	"$OUT_DIR/demo-tui.gif"

if [[ $COMMIT_ARTIFACTS == 1 ]]; then
	cp "$OUT_DIR/03-wallpaper-switched.png" "$DEMO_DIR/demo-tui-cosmic.png"
	printf 'committed_artifact=%s\n' "$DEMO_DIR/demo-tui-cosmic.png" >>"$VERIFY"
fi
printf 'review_required=true\n' >>"$VERIFY"

echo "Wrote $OUT_DIR/demo-tui.gif"
if [[ $COMMIT_ARTIFACTS == 1 ]]; then
	echo "Wrote $DEMO_DIR/demo-tui-cosmic.png"
fi
echo "Review the PNG frames and GIF before committing any artifact."
