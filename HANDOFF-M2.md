# FLOODLINE — handing over mid-plan

Written at 97% of a context window, at the end of the session that fixed the
multiplayer bugs and started the ten-milestone plan. `HANDOFF.md` is still the
guide to the *codebase*; this is the guide to **what is half-finished and what
comes next**. Read `CLAUDE.md`, then `HANDOFF.md`, then this.

---

## Where it is

**Live and playable at <https://sgilson7.github.io/floodline/>.** Two people
have finished a run together and survived the first flood — the first time
anybody has played it.

x, no warnings. `make test` ~17s,
`make browser-test` ~6 minutes.

The plan is an artifact, and it is the plan of record:
**<https://claude.ai/code/artifact/f51d6368-d9a0-48f2-9213-c41f1eba59b1>**

| | | |
|---|---|---|
| M1 | Forester costs stone | **done** |
| M2 | A camera over the map | **done** |
| M3 | Dikes: 3×1, drawn, breakable | ← **you are here** |
| M4 | The river and the wave | |
| M5 | Tuning: which dikes break | |
| M6 | Gold, the trading hut, mules | |
| M7 | Levels, and moving buildings | |
| M8 | Job icons, and workers going indoors | |
| M9 | Families, children, a nursery, a households tab | |
| M10 | Two agents, one game, played to the end | |

Nothing is half-written. Every commit is green and pushed; `main` and the
deployed build agree.

---

## What this session changed, in one paragraph each

**Four multiplayer bugs, all reproduced before being touched.** A lobby that
was left kept its Trystero room for ever (`WebPeer` had no `Drop`), so joiners
connected to a game that no longer existed and hung on "looking for the host".
A seat was never given back, so a two-seat game accepted exactly one joiner for
the host's whole life. Any peer leaving read as "the host left the game". And
the lobby could not tell any of those apart, because a joiner's `connected()`
was always 1.

**The gesture that starved every village.** Choosing a whole city and
right-clicking a farm asked three slots to take eight, which the rules refuse
*whole*, so nobody farmed and the city died on day four — two days before the
flood it never saw. `World::will_take` / `will_house` answer "how many would
you take"; the mouse sends what fits and says "3 of 8".

**A clock.** The sim advanced one tick per rendered frame — measured at 24/s
against a design rate of 10 — so a day was 20 seconds and `DROP_AFTER_TICKS`
meant five seconds instead of thirty. Now exactly 10.0.

**Wood and stone have a source** (forester's hut, quarry), **the Copy button
copies** (it must happen inside the click's own event, not the frame after),
**citizens take up room** (`crowd.rs`), and **the panel teaches** (one line
that always names the next thing to do).

All of it is in `DECISIONS.md`, most recent last. Read the last ten entries
before changing anything they touch.

---

## Starting M3: dikes

The plan artifact has the full write-up. The short version and the traps:

1. **Orientation.** `Kind::size()` is a constant per kind. A 3×1 dike needs
   `Building::facing`, and `Command::Place` has to carry it. That ripples
   through `footprint`, `fits_on_map`, `ground_suits`, `neighbours_suit`,
   `occupy`, `can_place` and `place`. Wire format changes — fine, the build
   hash gates it.
2. **`Command::DikeLine { from, to }`**, beside `Command::Road` and not reusing
   it: a road takes the cheapest path, a wall is a straight line. GUI is a
   click-drag with a ghost of the run and a running cost.
3. **`Building::stress: u32`.** Each tick, for the three cells on the wet side,
   `depth × speed` — both already kept by the water automaton — summed, scaled,
   accumulated. Past a per-level threshold the dike is rubble. Stress bleeds
   off while the water is away. Dikes leave `batter_buildings` and use this
   instead, so there is one model and not two.
4. **Do not pick the thresholds.** They are M5's job, measured against the
   river, to hit "many level ones break, most level twos hold".

Every existing dike test assumes 1×1 and places cell by cell. They all get
rewritten; expect the diff to look larger than the feature.

---

## Things that will bite you, that are new since `HANDOFF.md`

* **Two coordinate conversions now, and both live in `screen.rs`.**
  `Viewport` is the letterbox; `MapView` is the camera. `draw::map_rect` and
  `draw::cell_at` are **gone** — everything asks `MapView`. Do not add a third
  place that multiplies by `CELL`.
* **`Viewport::camera` passes GL the letterbox's *top* margin** where GL wants
  the distance from the bottom. It is correct only because the letterbox is
  centred and the two numbers are equal. `MapView::camera` does the
  subtraction properly. If the letterbox ever stops being centred, fix the
  older one.
* **`packaging/browser/view.py`** is the one copy of that arithmetic on the
  test side. Every script that clicks the map imports it. When the camera or
  the panel moves, that file and the panel y-coordinates in `play.py` /
  `assign.py` move with it — **the panel has shifted four times this week and
  each time it silently broke two browser checks.** Symptom: a check clicks a
  gap and reports something unrelated as failing.
* **`nav::passable` is two lines and four systems read it** — pathing, the
  crowd, the flood, road-laying. M4's ford and M8's "workers may stand in their
  own workplace" both change it. Change all four call sites together.
* **A fixed-point vector shorter than about 1/16 of a cell normalises to
  zero.** `with_len` divides by a length computed in the same 1/256ths, and
  `Fx(1) * Fx(1)` is 0. This cost an afternoon in `crowd.rs`; the fallback
  there is a whole-cell unit vector.
* **Anything that has to happen "when the player clicks" in the browser must
  happen in the click's own event.** macroquad reads a click in the *next*
  animation frame, by which time the browser's user gesture has expired.
  `fl_arm_copy` is the pattern: Rust arms the plugin each frame with the text
  and the button's rectangle, and the canvas's own listener does the work.
* **`navigator.clipboard.writeText` rejects with a promise**, which
  `try`/`catch` cannot see. That is how the Copy button did nothing, silently,
  for a whole session.

---

## How to work here

* `make test` at every commit, no warnings, `make browser-test` before pushing
  anything that touches `gui` or `web/`.
* **Measure numbers, do not pick them.** Every balance constant that is a
  judgement has its measurement written into its doc comment
  (`SHORE_DISTANCE`, `STARTING_STONE`, `FOREST_TICKS_PER_UNIT`). Add a probe,
  run it, paste the table. Three constants were wrong in ways no test caught
  until they were measured.
* **When a test fails, find out whether the test or the code is wrong before
  changing either.** Roughly half the failures in these sessions were the test
  encoding an old world; saying so in the comment is worth more than the fix.
* **Reproduce before fixing.** Every bug this session was reproduced from
  scratch first — the ghost room, the seat, the assignment — and each
  reproduction became a test.
* DECISIONS.md gets a paragraph for anything somebody else would have written
  differently.

---

## What has never been tested

* **A run has never been played to age 3 by two people.** One flood, once. M10
  exists to fix that and is deliberately last.
* **Two browsers on two networks have never talked**, except through this one
  machine. The one case that needs TURN — both ends behind strict NATs — is by
  definition untested. The user hit a strict-NAT error on their own LAN once;
  the message now distinguishes "the relays did not answer" (a pasted code
  helps) from "we found each other and could not connect" (it does not, and
  TURN is the only fix). **If they report it again, the sentence they saw is
  the whole diagnosis.**
* More than three peers has only been tested on `Loopback` and in `echo.html`.

---

## The single next action

Start M3. Begin with `Building::facing` and get `make test` green again before
writing a line of the pressure model — the orientation edit is wide and
mechanical, and it is much easier to review on its own.
