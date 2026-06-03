#!/usr/bin/env bash
# Driver for README asciinema demo (CLI workflow; TUI uses alternate screen — see record-tui.sh).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WALLS="${WALLS_BIN:-$ROOT/target/release/walls}"
[[ -x $WALLS ]] || WALLS="$ROOT/target/debug/walls"

TMP="$(mktemp -d)"
export XDG_CONFIG_HOME="$TMP/config"
export XDG_STATE_HOME="$TMP/state"
mkdir -p "$XDG_CONFIG_HOME/walls" "$XDG_STATE_HOME/walls" "$TMP/images"
echo '{}' >"$XDG_CONFIG_HOME/walls/secrets.json"
printf 'demo\n' >"$TMP/images/demo.jpg"

NOOP="$TMP/noop.sh"
printf '%s\n' '#!/bin/sh' 'exit 0' >"$NOOP"
chmod +x "$NOOP"

cat >"$XDG_CONFIG_HOME/walls/config.json" <<EOF
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

type_cmd() {
	local cmd="$1"
	for ((i = 0; i < ${#cmd}; i++)); do
		printf '%s' "${cmd:i:1}"
		sleep 0.035
	done
	sleep 0.25
	printf '\n'
}

pause() { sleep "${1:-1.2}"; }

clear
pause 0.4

type_cmd "$WALLS apply $TMP/images/demo.jpg"
"$WALLS" apply "$TMP/images/demo.jpg"
pause 1

type_cmd "$WALLS status --json"
"$WALLS" status --json | head -c 200
printf '...\n'
pause 1

type_cmd "$WALLS toggle-pause"
"$WALLS" toggle-pause
pause 0.8

type_cmd "$WALLS current"
"$WALLS" current || true
pause 1

echo
echo "# walls tui — run in your terminal (alternate screen; not captured here)"
echo "$WALLS tui"
pause 1.2
