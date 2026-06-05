#!/usr/bin/env bash
# Render fixed PTY frames for the TUI and assert key visual/behavioural states.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WALLS="${WALLS_BIN:-$ROOT/target/debug/walls}"

if [[ ! -x $WALLS ]]; then
	cargo build -p walls --features tui-preview
fi

python3 <<'PY'
import fcntl
import json
import os
import pty
import re
import select
import shutil
import struct
import tempfile
import termios
import time
from pathlib import Path

walls = Path(os.environ.get("WALLS_BIN", "target/debug/walls"))


def setup():
    tmp = Path(tempfile.mkdtemp())
    config_home = tmp / "config"
    state_home = tmp / "state"
    image_dir = tmp / "images"
    for path in [config_home / "walls", state_home / "walls", image_dir, tmp / "cache"]:
        path.mkdir(parents=True, exist_ok=True)
    (image_dir / "a.jpg").write_bytes(b"x")
    noop = tmp / "noop.sh"
    noop.write_text("#!/bin/sh\nexit 0\n")
    noop.chmod(0o755)
    config = {
        "change": {"enabled": True, "internet_enabled": False},
        "paths": {
            "cache_dir": str(tmp / "cache"),
            "download_dir": str(tmp / "downloaded"),
            "favorites_dir": str(tmp / "favorites"),
            "fetched_dir": str(tmp / "fetched"),
            "compose_dir": str(tmp / "wallpaper"),
        },
        "apply": {"backend": "custom-script", "custom_script": str(noop)},
        "display": {"mode": "os"},
        "sources": [{"enabled": True, "type": "folder", "path": str(image_dir)}],
    }
    (config_home / "walls" / "config.json").write_text(json.dumps(config))
    secrets = config_home / "walls" / "secrets.json"
    secrets.write_text("{}")
    secrets.chmod(0o600)
    return tmp, config_home, state_home


def screen(data, cols, rows):
    grid = [[" "] * cols for _ in range(rows)]
    x = y = 0
    text = data.decode("utf-8", "ignore")
    i = 0
    while i < len(text):
        char = text[i]
        if char == "\x1b":
            i += 1
            if i < len(text) and text[i] == "[":
                i += 1
                start = i
                while i < len(text) and not ("@" <= text[i] <= "~"):
                    i += 1
                params = text[start:i].replace("?", "")
                final = text[i] if i < len(text) else ""
                i += 1
                nums = [
                    int(value) if value else 0
                    for value in re.split(r"[;:]", params)
                    if value == "" or value.isdigit()
                ]
                if final in "Hf":
                    y = max(0, min(rows - 1, (nums[0] if len(nums) > 0 and nums[0] else 1) - 1))
                    x = max(0, min(cols - 1, (nums[1] if len(nums) > 1 and nums[1] else 1) - 1))
                elif final == "J":
                    grid = [[" "] * cols for _ in range(rows)]
                    x = y = 0
                elif final == "K":
                    for cx in range(x, cols):
                        grid[y][cx] = " "
                elif final == "C":
                    x = min(cols - 1, x + (nums[0] if nums else 1))
                elif final == "D":
                    x = max(0, x - (nums[0] if nums else 1))
                elif final == "B":
                    y = min(rows - 1, y + (nums[0] if nums else 1))
                elif final == "A":
                    y = max(0, y - (nums[0] if nums else 1))
                continue
            continue
        if char == "\r":
            x = 0
        elif char == "\n":
            y = min(rows - 1, y + 1)
            x = 0
        elif char == "\b":
            x = max(0, x - 1)
        elif char >= " ":
            grid[y][x] = char
            x = min(cols - 1, x + 1)
        i += 1
    return "\n".join("".join(row).rstrip() for row in grid)


def capture(name, cols, rows, keys=b"", extra_env=None):
    tmp, config_home, state_home = setup()
    pid, fd = pty.fork()
    if pid == 0:
        env = {
            "XDG_CONFIG_HOME": str(config_home),
            "XDG_STATE_HOME": str(state_home),
            "RUST_BACKTRACE": "0",
            "TERM": "xterm-256color",
            "WALLS_TUI_PREVIEW": "0",
        }
        if extra_env:
            env.update(extra_env)
        os.environ.update(env)
        os.execv(str(walls), [str(walls), "tui"])

    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    out = b""
    for _ in range(14):
        time.sleep(0.05)
        while select.select([fd], [], [], 0)[0]:
            out += os.read(fd, 65536)
    if keys:
        os.write(fd, keys)
        for _ in range(12):
            time.sleep(0.05)
            while select.select([fd], [], [], 0)[0]:
                out += os.read(fd, 65536)

    frame = screen(out, cols, rows)
    os.write(fd, b"q")
    deadline = time.time() + 3
    while time.time() < deadline:
        if select.select([fd], [], [], 0.05)[0]:
            try:
                os.read(fd, 65536)
            except OSError:
                break
        done, _ = os.waitpid(pid, os.WNOHANG)
        if done == pid:
            break
    else:
        os.kill(pid, 9)
        raise SystemExit(f"{name}: timeout waiting for quit")
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"--- {name} {cols}x{rows} ---")
    print(frame)
    return frame


standard = capture("standard-status", 80, 24)
assert "normal ready" in standard, standard
assert "space pause" in standard and "q quit" in standard, standard
assert "local candidates: 1 paths" in standard, standard

narrow_search = capture("narrow-search", 42, 10, b"5")
assert "Search" in narrow_search and "query:" in narrow_search, narrow_search
assert "i edit | Enter | j/k | : | q" in narrow_search, narrow_search

no_colour = capture("no-colour-status", 80, 24, extra_env={"WALLS_TUI_COLOR": "never"})
assert "normal ready" in no_colour and "q quit" in no_colour, no_colour

wide_now = capture("wide-now-preview-disabled", 120, 32, b"2")
assert "Now" in wide_now and "(no current wallpaper)" in wide_now, wide_now

print("ok: TUI visual verification frames passed")
PY
