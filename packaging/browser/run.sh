#!/usr/bin/env bash
# Serve dist/web/ and run every browser check against it.
#
# The venv is built once and kept; Chromium is a couple of hundred megabytes
# and downloading it per run would make this the thing nobody runs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VENV="$ROOT/.venv-test"
PORT=${PORT:-8123}

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }

if [ ! -x "$VENV/bin/python" ]; then
  say "Making $VENV (once)"
  python3 -m venv "$VENV"
  "$VENV/bin/pip" -q install playwright pillow
  "$VENV/bin/playwright" install chromium
fi

[ -f "$ROOT/dist/web/echo.html" ] || { echo "run make web first" >&2; exit 1; }

say "Serving dist/web on :$PORT"
( cd "$ROOT/dist/web" && exec python3 -m http.server "$PORT" >/dev/null 2>&1 ) &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT
sleep 1

fail=0
for script in "$ROOT"/packaging/browser/echo_*.py; do
  say "$(basename "$script")"
  "$VENV/bin/python" "$script" "http://localhost:$PORT/echo.html" || fail=1
done

# The letterbox has been wrong at a device pixel ratio of two before and was
# invisible at one. Both, every time.
for dpr in 1 2; do
  say "game.py at device pixel ratio $dpr"
  "$VENV/bin/python" "$ROOT/packaging/browser/game.py" \
    "http://localhost:$PORT/index.html" "$dpr" || fail=1
done

# The mouse reaching the simulation, which is the one thing cargo cannot say.
say "play.py"
"$VENV/bin/python" "$ROOT/packaging/browser/play.py" \
  "http://localhost:$PORT/index.html" || fail=1

# The whole stack: two tabs of the real game, both ways into a room.
for mode in room code; do
  say "game_two_tabs.py by $mode"
  "$VENV/bin/python" "$ROOT/packaging/browser/game_two_tabs.py" \
    "http://localhost:$PORT/index.html" 1 "$mode" || fail=1
done

exit $fail
