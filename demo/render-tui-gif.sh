#!/usr/bin/env bash
# Render a deterministic TUI-first README GIF without a graphical recorder.
#
# This drives the real `walls tui` in a PTY with an isolated demo config, captures
# alternate-screen frames, renders them to SVG, and converts them to demo-tui.gif.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-$ROOT/demo/demo-tui.gif}"
WALLS="${WALLS_BIN:-$ROOT/target/release/walls}"
[[ -x $WALLS ]] || WALLS="$ROOT/target/debug/walls"

if [[ ! -x $WALLS ]]; then
	echo "Build walls first, e.g.: cargo build -p walls --release"
	exit 1
fi

for bin in python3 magick; do
	if ! command -v "$bin" >/dev/null; then
		echo "Need $bin"
		exit 1
	fi
done

OUT="$OUT" WALLS="$WALLS" python3 <<'PY'
import html
import json
import os
import pty
import re
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path

walls = Path(os.environ["WALLS"])
out = Path(os.environ["OUT"])
cols = 96
rows = 28


def setup_demo_root():
    tmp = Path(os.environ.get("WALLS_DEMO_ROOT", "/tmp/walls-tui-demo"))
    shutil.rmtree(tmp, ignore_errors=True)
    config_home = tmp / "config"
    state_home = tmp / "state"
    image_dir = tmp / "demo-wallpapers"
    for path in [
        config_home / "walls",
        state_home / "walls",
        image_dir,
        tmp / "cache",
        tmp / "downloaded",
        tmp / "favorites",
        tmp / "fetched",
        tmp / "wallpaper",
    ]:
        path.mkdir(parents=True, exist_ok=True)

    # Tiny placeholder files are enough for the TUI list/state paths. The custom
    # apply script keeps this demo independent of a real desktop wallpaper backend.
    (image_dir / "aurora-field.jpg").write_bytes(b"demo aurora")
    (image_dir / "city-lights.jpg").write_bytes(b"demo city")
    (image_dir / "forest-path.jpg").write_bytes(b"demo forest")

    noop = tmp / "apply-noop.sh"
    noop.write_text("#!/bin/sh\nprintf '%s\\n' \"$1\" >>\"$APPLIED_LOG\"\n")
    noop.chmod(0o755)

    config = {
        "change": {"enabled": True, "internet_enabled": False, "interval_secs": 300},
        "paths": {
            "cache_dir": str(tmp / "cache"),
            "download_dir": str(tmp / "downloaded"),
            "favorites_dir": str(tmp / "favorites"),
            "fetched_dir": str(tmp / "fetched"),
            "compose_dir": str(tmp / "wallpaper"),
        },
        "apply": {"backend": "custom-script", "custom_script": str(noop)},
        "display": {"mode": "os"},
        "sources": [
            {
                "enabled": True,
                "type": "folder",
                "label": "Demo wallpapers",
                "path": str(image_dir),
            }
        ],
    }
    (config_home / "walls" / "config.json").write_text(json.dumps(config))
    secrets = config_home / "walls" / "secrets.json"
    secrets.write_text("{}")
    secrets.chmod(0o600)
    return tmp, config_home, state_home


def apply_csi(grid, cursor, params, final):
    x, y = cursor
    nums = [
        int(value) if value else 0
        for value in re.split(r"[;:]", params.replace("?", ""))
        if value == "" or value.isdigit()
    ]
    if final in "Hf":
        y = max(0, min(rows - 1, (nums[0] if len(nums) > 0 and nums[0] else 1) - 1))
        x = max(0, min(cols - 1, (nums[1] if len(nums) > 1 and nums[1] else 1) - 1))
    elif final == "J":
        for row in grid:
            row[:] = [" "] * cols
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
    return x, y


def screen(data):
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
                params = text[start:i]
                final = text[i] if i < len(text) else ""
                i += 1
                x, y = apply_csi(grid, (x, y), params, final)
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
    return ["".join(row).rstrip() for row in grid]


def read_available(fd, data):
    while select.select([fd], [], [], 0)[0]:
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        data += chunk
    return data


def wait_and_frame(fd, data, delay):
    deadline = time.time() + delay
    while time.time() < deadline:
        time.sleep(0.04)
        data = read_available(fd, data)
    return data, screen(data)


def svg_for(lines):
    char_w = 9
    char_h = 18
    pad_x = 24
    pad_y = 26
    width = cols * char_w + pad_x * 2
    height = rows * char_h + pad_y * 2
    body = []
    for index, line in enumerate(lines):
        escaped = html.escape(line)
        body.append(
            f'<text x="{pad_x}" y="{pad_y + (index + 1) * char_h}" '
            f'xml:space="preserve">{escaped}</text>'
        )
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <rect width="100%" height="100%" rx="10" fill="#101418"/>
  <rect x="10" y="10" width="{width - 20}" height="{height - 20}" rx="8" fill="#151b22" stroke="#5b8cff" stroke-width="2"/>
  <style>
    text {{
      fill: #e5e7eb;
      font-family: "JetBrains Mono", "Fira Code", "DejaVu Sans Mono", monospace;
      font-size: 15px;
    }}
  </style>
  {''.join(body)}
</svg>
'''


def write_frame(frame_dir, index, lines):
    svg = frame_dir / f"frame-{index:02d}.svg"
    png = frame_dir / f"frame-{index:02d}.png"
    svg.write_text(svg_for(lines))
    subprocess.run(["magick", str(svg), str(png)], check=True)
    return png


tmp, config_home, state_home = setup_demo_root()
frame_dir = Path(tempfile.mkdtemp(prefix="walls-tui-frames-"))
try:
    pid, fd = pty.fork()
    if pid == 0:
        os.environ.update(
            {
                "XDG_CONFIG_HOME": str(config_home),
                "XDG_STATE_HOME": str(state_home),
                "APPLIED_LOG": str(tmp / "applied.log"),
                "RUST_BACKTRACE": "0",
                "TERM": "xterm-256color",
                "WALLS_TRAY": "0",
                "WALLS_TUI_PREVIEW": "0",
                "WALLS_TUI_INTRO": "1",
            }
        )
        os.execv(str(walls), [str(walls), "tui"])

    fcntl_ioctl = __import__("fcntl").ioctl
    fcntl_ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    data = b""
    frames = []

    data, frame = wait_and_frame(fd, data, 0.08)
    frames.append(frame)
    data, frame = wait_and_frame(fd, data, 0.35)
    frames.append(frame)

    for keys, delay in [
        (b"4", 0.25),  # Browse
        (b"5", 0.25),  # Search
        (b"2", 0.25),  # Now
        (b"n", 0.55),  # Apply next wallpaper
        (b"?", 0.25),  # Key help
    ]:
        os.write(fd, keys)
        data, frame = wait_and_frame(fd, data, delay)
        frames.append(frame)

    os.write(fd, b"\x1bq")
    deadline = time.time() + 1
    exited = False
    while time.time() < deadline:
        data = read_available(fd, data)
        done, _ = os.waitpid(pid, os.WNOHANG)
        if done == pid:
            exited = True
            break
        time.sleep(0.05)
    if not exited:
        os.write(fd, b"\x03")
        time.sleep(0.2)
        done, _ = os.waitpid(pid, os.WNOHANG)
        if done != pid:
            os.kill(pid, 9)
            os.waitpid(pid, 0)

    pngs = []
    for i, frame in enumerate(frames):
        pngs.append(write_frame(frame_dir, i, frame))

    out.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["magick", "-delay", "95", "-loop", "0", *map(str, pngs), str(out)],
        check=True,
    )
    print(f"Wrote {out} ({out.stat().st_size} bytes)")
finally:
    shutil.rmtree(tmp, ignore_errors=True)
    shutil.rmtree(frame_dir, ignore_errors=True)
PY
