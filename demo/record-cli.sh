#!/usr/bin/env bash
# Record demo/demo-cli.cast and demo/demo.gif (requires asciinema + agg).
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v asciinema >/dev/null || ! command -v agg >/dev/null; then
	echo "Need asciinema and agg, e.g.: nix-shell -p asciinema asciinema-agg --run '$0'"
	exit 1
fi

export WALLS_BIN="${WALLS_BIN:-$(cd .. && pwd)/target/release/walls}"
chmod +x demo-cli.sh

asciinema rec demo-cli.cast \
	--command './demo-cli.sh' \
	--cols 80 \
	--rows 24 \
	--overwrite \
	--title 'walls CLI'

agg demo-cli.cast demo.gif --font-size 14 --speed 1.1
echo "Wrote demo-cli.cast and demo.gif ($(wc -c <demo.gif) bytes)"
