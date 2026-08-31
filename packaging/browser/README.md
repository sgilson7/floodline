# Checking the browser build

`make test` is hermetic, needs no window and takes twelve seconds, and that is
worth keeping — so none of this is part of it. What is here is the other half:
the four things about phase 4 that only a real browser can answer, in a form
that can be run again rather than described.

    make browser-test

builds `dist/web/`, serves it, and runs every `*.py` here against it. The first
run makes a virtualenv and downloads Chromium, which takes a couple of minutes;
after that it is about a minute. `.venv-test/` is gitignored.

| script | what it answers |
|---|---|
| `echo_code.py` | the pasted-code path: one invitation, one reply, bytes on both channels, a closed tab seen as `Left`, and a fresh invitation waiting for the next joiner |
| `echo_room.py` | the Trystero path with three tabs: two joiners reach the host, and each joiner sees exactly *one* peer even though Trystero introduced it to the other joiner |
| `echo_more.py` | the BitTorrent strategy, and a tab on a different build failing to find the game (design 9.4's room-name prefix) |
| `game.py` | the real game loads, letterboxes where it says it will, and keeps drawing once started |
| `clipboard.py` | the Copy button on the pasted-code screen actually copies, with clipboard writing denied |
| `rejoin.py` | a seat a joiner left is given to the next one, an abandoned lobby leaves its room, and hosting a second game does not close the one it just opened |
| `assign.py` | choosing a whole city and right-clicking a farm puts three people on it and says so, instead of being refused whole |
| `camera.py` | zoom and pan, nothing drawn over the panel, and a click still lands on the cell the cursor is over |
| `view.py` | not a check: the one copy of the letterbox-and-camera arithmetic every other script imports |
| `panel.py` | not a check: the one copy of the *panel's* running totals, for anything written for M10. `play.py` and `assign.py` keep their own literals on purpose |
| `panel_rows.py` | whether choosing a building moves the rows below it — it did, five times, and the symptom is a click landing in a gap |
| `play.py` | the mouse reaching the simulation: choosing citizens, ordering them about, a cottage, a road, a ping, a trade, a wall drawn with a drag, and a refusal that says so |
| `game_two_tabs.py` | the whole stack: two tabs reach the lobby, join by room code or by pasted code, and both leave the lobby into the same world |
| `two_agents.py` | the same world reached by two *browsers* that share no clipboard, storage or renderer — M10's ground, and the one setup that has to hold for thirty-six minutes |

The three `echo_*` scripts drive `web/echo.html`, which drives `window.FLOODLINE_RTC`
directly — no wasm, no `sim`, no lockstep. That separation is design 9.6's and
it earns its keep: if `echo_room.py` passes and `game.py` does not, the fault is
above the transport, and the converse.

Two things to keep doing, because both of the worst bugs of phase 3 were found
this way and neither was visible any other way:

* **read the console.** Every script forwards `pageerror` and `console.error`.
  An uncaught error on load is not cosmetic; it is the thing that will be
  mistaken for the cause of whatever goes wrong next.
* **run at a device pixel ratio of 2 as well as 1.** `game.py` takes the ratio
  as its second argument and `make browser-test` runs it at both. A letterbox
  bug is invisible from a desktop and unmissable on a laptop.

Against the deployed build instead of a local one:

    ./.venv-test/bin/python packaging/browser/echo_room.py \
        https://sgilson7.github.io/floodline/echo.html

Two tabs of a *local* build and the *deployed* build cannot join each other,
and that is the guard working: the room name carries the wasm's sha256, CI's
rustc is not this machine's, and design 8 says mismatched builds do not play
together. Test two tabs on the same build.
