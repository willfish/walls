#!/usr/bin/env bash
# Smoke-run walls tui in a PTY (same idea as crates/cli/tests/tui_smoke.rs).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WALLS="${WALLS_BIN:-$ROOT/target/release/walls}"
[[ -x $WALLS ]] || WALLS="$ROOT/target/debug/walls"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CONFIG_HOME="$TMP/config"
STATE_HOME="$TMP/state"
mkdir -p "$CONFIG_HOME/walls" "$STATE_HOME/walls" "$TMP/images" "$TMP/cache"
echo '{}' >"$CONFIG_HOME/walls/secrets.json"
echo 'x' >"$TMP/images/a.jpg"

NOOP="$TMP/noop.sh"
printf '%s\n' '#!/bin/sh' 'exit 0' >"$NOOP"
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

export XDG_CONFIG_HOME="$CONFIG_HOME"
export XDG_STATE_HOME="$STATE_HOME"
export WALLS_BIN="$WALLS"
export RUST_BACKTRACE=0

python3 <<'PY'
import os, pty, select, sys, time

walls = os.environ["WALLS_BIN"]
pid, fd = pty.fork()
if pid == 0:
    os.execvp(walls, [walls, "tui"])
    raise SystemExit(1)

time.sleep(0.5)
os.write(fd, b"3")
time.sleep(0.3)
os.write(fd, b"q")

deadline = time.time() + 5
out = b""
while time.time() < deadline:
    r, _, _ = select.select([fd], [], [], 0.2)
    if r:
        try:
            out += os.read(fd, 4096)
        except OSError:
            break
    pid_done, status = os.waitpid(pid, os.WNOHANG)
    if pid_done == pid:
        exit_code = os.waitstatus_to_exitcode(status)
        if exit_code != 0:
            sys.stderr.write(out.decode(errors="replace"))
            raise SystemExit(f"walls tui exited {exit_code}")
        break
else:
    os.kill(pid, 9)
    raise SystemExit("timeout waiting for walls tui")

text = out.decode(errors="replace")
# Alternate-screen TUI may emit mostly ANSI; success = clean exit + some output.
if len(out) < 32:
    sys.stderr.write(repr(text))
    raise SystemExit("PTY captured almost no output")
print("ok: walls tui ran in PTY and exited cleanly")
print(f"capture_bytes={len(out)}")
if "Browse" in text or "walls" in text:
    print("ok: readable TUI chrome in capture")
else:
    print("note: capture is mostly ANSI (alternate screen); use gpu-screen-recorder for GIF")
PY
