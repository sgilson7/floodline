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

# The camera, and the thing it could break: does a click still land on the cell
# the cursor is over? At both device pixel ratios, because the coordinate code
# has been wrong twice and both times it was invisible at one.
for dpr in 1 2; do
  say "camera.py at device pixel ratio $dpr"
  "$VENV/bin/python" "$ROOT/packaging/browser/camera.py" \
    "http://localhost:$PORT/index.html" "$dpr" || fail=1
done

# The mouse reaching the simulation, which is the one thing cargo cannot say.
say "play.py"
"$VENV/bin/python" "$ROOT/packaging/browser/play.py" \
  "http://localhost:$PORT/index.html" || fail=1

# Putting people to work: the gesture that used to do nothing at all.
say "assign.py"
"$VENV/bin/python" "$ROOT/packaging/browser/assign.py" \
  "http://localhost:$PORT/index.html" || fail=1

# The panel has moved under a written-down coordinate five times. This is the
# one that asks the picture rather than reasoning about the running total.
say "panel_rows.py"
"$VENV/bin/python" "$ROOT/packaging/browser/panel_rows.py" \
  "http://localhost:$PORT/index.html" || fail=1

# The Copy button, with clipboard writing denied - the case the fallback is for.
say "clipboard.py"
"$VENV/bin/python" "$ROOT/packaging/browser/clipboard.py" \
  "http://localhost:$PORT/index.html" || fail=1

# A room that can be joined more than once, and a lobby that lets go of it.
say "rejoin.py"
"$VENV/bin/python" "$ROOT/packaging/browser/rejoin.py" \
  "http://localhost:$PORT/index.html" || fail=1

# The whole stack: two tabs of the real game, both ways into a room.
for mode in room code; do
  say "game_two_tabs.py by $mode"
  "$VENV/bin/python" "$ROOT/packaging/browser/game_two_tabs.py" \
    "http://localhost:$PORT/index.html" 1 "$mode" || fail=1
done

# The same world reached by two browsers that share nothing - the ground a
# playtest is run on, and the one that has to hold for a whole run.
say "two_agents.py"
"$VENV/bin/python" "$ROOT/packaging/browser/two_agents.py" \
  "http://localhost:$PORT/index.html" || fail=1

# Three cities in one game. The lobby's `+` goes to six and the lockstep plays
# six; two is simply the only size anybody had ever seated, so a third player
# was untestable rather than impossible. A third one also makes the room stop
# being a star - the two joiners are connected to each other - which is where
# `send_turns` was found broadcasting a joiner's turn to everybody.
say "three_players.py"
"$VENV/bin/python" "$ROOT/packaging/browser/three_players.py" \
  "http://localhost:$PORT/index.html" || fail=1

# An agent's hands: every verb it has, doing something visible. A verb that is
# missing or wrong is otherwise found halfway through a run that cannot be
# repeated cheaply.
say "driver_check.py"
"$VENV/bin/python" "$ROOT/packaging/browser/driver_check.py" \
  "http://localhost:$PORT/index.html" || fail=1

exit $fail
