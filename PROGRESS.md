# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 2 — 2026-08-30

**Phases 4, 5 and 6 done. The MVP is playable end to end.** 212 tests and
seven browser checks, no warnings, `make test` in about twelve seconds.

Live at **https://sgilson7.github.io/floodline/**. Two people on two machines
open the link, one hosts and shares a room code or a pasted invitation, the
other joins, and both play the same world — with nothing running anywhere
except GitHub Pages.

### Checklist

- [x] **Phase 0** — workspace, Makefile, `package-web.sh`, Pages workflow
- [x] **Phase 1** — `sim`: land, citizens, buildings, roads, trade, ages, score
- [x] **Phase 2** — water, the surge, bodies in the flood, building damage
- [x] **Phase 3** — `net::Peer`, `Loopback`, the wire format, the star lockstep
- [x] **Phase 4** — `quad_rtc.js`, vendored Trystero, the pasted-code path,
      `net-web`, `echo.html`. Both paths verified in a real browser, on the
      deployed build.
- [~] **Phase 5** — the lobby, selection, the build menu, the road tool, the
      trade dialog, the score screen and design step 7's playtest are all
      built. Its done-condition is "two people on two machines play a full run
      to age 3 on the Pages build", and that has been attempted once and
      failed. Not done until somebody finishes a run.
- [x] **Phase 6** — failure messages that say what to do, the relay fallback,
      the build-hash guard end to end, the README.

### What was decided

Seven entries added to `DECISIONS.md` (this said ten, and was wrong); the ones that would change what somebody
else wrote are indexed under "Things that will bite you" in `HANDOFF.md`. The
four that matter most:

* **The handshake was written down before the plugin** (as the plan asks) and
  three of its guesses were wrong in ways only a browser could say. The one it
  got right in advance — replacing design §9.2's "a joiner accepts the first
  peer it meets" with a role byte — would otherwise have wired two joiners
  together and only with three players and only sometimes.
* **Playing a full run found four bugs no test had.** Nothing in a city ever
  built anything unless a player knew to assign builders, so an unattended city
  starved on day four with the materials on the floor. "Get uphill" — design
  §3.2's one order that matters in a flood — was undone a tick after they
  arrived. Every citizen starts inside its own Hearth and could not be ordered
  out of it.
* **The map decided the game.** Hearth sites on a ring around the map centre
  sat anywhere from 65 to 148 cells from the water, and an age-one flood stops
  at about 115. They now sit on a line at a fixed distance from the corner the
  water comes out of. The spacing guarantee fell from 40 cells to 17 and
  five- and six-player maps are cramped; that cost is written down.
* **A dike cost twenty times what a city could pay.** A wall that changes the
  outcome is about thirty-four cells; at forty stone a level that is 2 720
  against a purse of 120. Ten a level and 720 to start buys one good wall in a
  run, and choosing where to put it is the decision.

### Blocked

Nothing.

### Not answered, and needing a person

Design step 7 says "playtest the flood until it is fun", and nobody has played
FLOODLINE with their hands. What was measured is that the decisions now have
different outcomes — idling starves, growing survives two ages bloodied, a dike
keeps everybody through ages one and two — which is the part a test can settle.
Three specific questions are left, all in `DECISIONS.md` under "Design step 7":

1. **Age three kills everybody on two of three seeds** whatever is done.
2. **Nothing produces stone**, so a player gets exactly one wall in a run. That
   is a clean decision or a straitjacket, and only playing will say which.
3. **A run is thirty-six minutes.** Design §11 already suspects that is long.

---

## Session 3 — 2026-08-30

**Playing it broke it, which is what playing it is for.** Two people could not
get into the same room, and a single-player village starved before the flood.
Four bugs, all reproduced from scratch before being touched, all now tested:
220 cargo tests and eleven browser checks.

* **A lobby that was left kept its room.** `WebPeer` had no `Drop`, so leaving
  the lobby dropped the game and left the browser in the Trystero room for
  ever, answering strangers on behalf of nothing.
* **A seat was never given back.** A two-seat game accepted exactly one joiner
  for the host's whole life.
* **Any peer leaving read as "the host left the game".**
* **Choosing the whole city and right-clicking the farm did nothing at all** —
  three slots, eight people, refused whole — so nobody farmed and the city
  starved on day four with the farm standing empty.

The last one is why "the water did not render": the run ended two days before
the flood. `the_opening_a_player_would_play_reaches_the_flood` now plays that
opening in `sim` and asserts the city lives to see water.

### Still open, in the order they will be hit

1. **Nothing produces wood or stone.** 200 wood is about five buildings and
   that is the whole run; a Forester's hut and a Quarry are in design section
   3.3 and deferred by the plan. This is the first question a player asks.
2. **No fixed timestep.** The sim advances one tick per rendered frame —
   measured at 24 ticks a second against a design rate of 10, and ~60 on a
   normal display. A day is 20 to 50 seconds instead of two minutes, and the
   drop timeout counts frames rather than seconds.
3. **Design step 7 still needs a person.** Nobody has finished a run.

### Next action

See `HANDOFF-M2.md`. Two people have now played together and survived the first
flood, and a ten-milestone plan is agreed and under way — M1 and M2 are done,
M3 (dikes) is next. The plan of record is an artifact:
<https://claude.ai/code/artifact/f51d6368-d9a0-48f2-9213-c41f1eba59b1>

---

## Session 4 — 2026-08-30

**M3 is done: dikes are three cells wide, drawn as a line, and they break.**
250 cargo tests, no warnings, `make test` in about seventeen seconds. Three
commits, each green on its own, in the order the handover asked for.

* **`Building::facing`.** A wide mechanical edit on its own, before a line of
  the pressure model. `Kind::size` takes a `Facing`, which ripples through
  `footprint`, `fits_on_map`, `ground_suits`, `neighbours_suit`, `can_place`
  and `place`; `Command::Place` carries one for every kind so there is one
  `place` on the wire. `Building::site` normalises it away for a kind that
  does not turn, so a cottage cannot checksum a distinction the game does not
  make.
* **`Command::DikeLine { from, to }`**, beside `Command::Road` and not reusing
  it. The run snaps to the longer axis and to a whole number of segments; a
  segment the ground refuses is skipped rather than failing the line. The tool
  is a click-drag with a ghost and a running cost, both drawn from
  `plan_dike_line` — the same function that lays the wall.
* **`Building::stress`.** Each tick the water's push on the wet side is added,
  time takes some away, past a per-level limit the segment is rubble, and a
  strained dike darkens toward it. Dikes leave `batter_buildings`.

### What was decided

Three entries in `DECISIONS.md`. Two matter to anyone else:

* **The plan's pressure formula was wrong and the measurement said so.**
  `depth * speed` is zero exactly where a wall earns its keep, because water a
  dike has stopped is water that has stopped moving — fifty-one sixteenths
  piled against a wall moving at a speed of two. It is
  `depth * (STILL_PUSH + speed)` now: depth loads the wall, flow makes the
  front worse than the pool.
* **A segment is priced per cell.** Leaving `Kind::Dike.cost()` at ten stone
  would have made a wall a third of the price `STARTING_STONE` was measured
  against. No assertion caught it; the five-strategy playtest did.

### Measured, and left alone deliberately

Drawing straight costs a barrier something. `playtest.rs` used to wall a
diagonal one cell at a time, which against a four-neighbour automaton is a
perfect seal for one cell of stone per cell of front; straight segments cannot
draw a diagonal, so it draws a staircase and half of every staircase runs along
the flow rather than across it. Same seeds, same stone: survivors for the
`dike` strategy went from 2 to 1 and for `both` from 4 to 1. **This is not
tuned here on purpose.** M4 replaces the corner flood with a river, where a
wall along a bank *is* a straight line, and M5 re-derives every one of these
numbers against it.

`DIKE_STRESS_LIMIT` is provisional for the same reason and says so in its doc
comment: measured against sustained flow on flat ground, enough to make "a
level one breaks, a level two holds" true and testable, and M5's to replace.

### Blocked

Nothing.

### Next action

**M4 — the river, and the wave.** Two sessions, the biggest change in the plan,
and the one that invalidates the most measurement. Start with `map::river`:
a deterministic meandering channel carved before the ground bands are computed,
so the river counts as the shallows it is. `SHORE_DISTANCE` stops meaning
"from the low corner" and starts meaning "from the river bank". The ford is a
fourth passability rule in a system that has had one, and `nav::passable` is
read by pathing, the crowd, the flood and road-laying — change all four
together.
