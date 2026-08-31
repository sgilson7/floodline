# FLOODLINE — handing over after the M11 playtest

This document is also an artifact, if a link is easier to hand on than a file:
**<https://claude.ai/code/artifact/954c3d02-e9ac-43eb-9d71-661db385ce5f>**

`HANDOFF.md` is the guide to the *codebase* and is still true of it.
`HANDOFF-M10.md` is the guide to how the two-agent playtest works and is still
true of that. **This is the guide to what the M11 run found and what to do
about it.** Read `CLAUDE.md`, then `HANDOFF.md`, then this.

---

## Where it is

**Live at <https://sgilson7.github.io/floodline/>**, and `main`, the deployed
build and this document all agree at commit `998b50d`.

286 cargo tests, 17 browser checks, no warnings. `make test` ~30s,
`make browser-test` ~10 minutes.

| | |
|---|---|
| The plan M11 executed | `PLAN-M11.md` |
| The account of the run | **`PLAYTEST-M11.md`** — read this before anything else here |
| The previous run's account | `PLAYTEST-M10.md` |
| Every decision, in date order | `DECISIONS.md` — the last dozen entries are M11 |

M11.1 through M11.9 are **done**. This document is what M11.9 turned up.

## What M11 was

M10 played the game to the end and produced one sentence that shaped
everything since:

> **The wall is not underpowered. It is unreadable.**

M11 was nine milestones of making the world legible — the clock halved, the
panel stopped drawing over its own instruments, the ground got a height and a
high-water mark, the wall got a strain readout, people got sent by who is free,
deaths got causes, goods in transit stopped looking like theft, and a city
stopped being the colour of water. **Not one of them changed a balance
constant.**

Then M11.9 played it again, with two agents, and asked two narrower questions:
*can a player grow a city above eight*, and *did any of that change a
decision*.

## What the run found, in one line

**No city in FLOODLINE could ever grow, and the probe pointed at growth was the
reason nobody knew.** That one is fixed — see `DECISIONS.md`, "No city could
grow" — but the lesson generalises and is the most important thing in this
document:

> `how_a_city_grows` sets `c.food = NEED_FULL` every tick, so it had only ever
> tested a city that cannot get hungry. **A probe that arranges the condition it
> is measuring can only confirm itself.** This repo's rule is "measure, do not
> pick"; the corollary is that the measurement has to be taken on the thing
> that ships.

Before adding a probe, ask what it is *arranging*. Before trusting one, ask the
same.

---

## The work, ranked

Three groups, in the order they are worth doing. The account has the players'
own words for all of it.

### A — Confirmed, small, and certain

Nothing here needs reproducing; both accounts agree and the cause is known.
Together these are about a session's work.

1. **`back to hauling` leaves people reading `idle`.** The households roster
   added in M11.5 labels a citizen with `job == None` as idle, and an
   unassigned citizen *is* a hauler — that is what hauling is. Both players hit
   it and one re-pressed the button for two days believing it had done nothing.
   `crates/gui/src/input.rs::households`.
2. **Deaths share one message slot with refusals, so they are lost.** M11.6
   added "2 drowned, 1 starved" and it worked — but `self.say` has one slot,
   deaths arrive a few per frame, and a click-refusal in the same frame
   overwrites them. City 0 lost eight people and read "1 drowned" throughout;
   city 1 never saw a death message at all. Either accumulate deaths over a
   window and say the total, or give the flood its own line. This is the
   readout that most needs fixing.
3. **Water depth belongs in the panel during a flood.** `water: wading` versus
   `out of your depth` is the difference between ignore-it and evacuate-now,
   and it exists only under the cursor. The warning "8 of your people are in
   the water" does not say *how deep*. Both players asked for this, and city 0:
   *"the only thing on the whole screen that distinguishes survivable water
   from lethal water. It changed exactly one decision and by then everyone was
   dead."*
4. **Two refusals lie.** A cottage that is not built yet says **"no beds left
   there"**, which is what a *full* cottage says. A nursery says **"no room
   left there"** when the truth is that a nursery is not a workplace at all.
   Both sent a player looking for the wrong problem. Compare the dike's
   "it is not built yet", which is right.
5. **The amber line's priorities are wrong late in a run.** It spent age 2
   telling both players to build a trading post — while one had a stalled
   household, no children and a flood two days out. The ordering in
   `tutorial::next_thing` is "what kills you soonest" and stops thinking after
   food.

### B — Reported by a player, *not reproduced*. Reproduce before touching

The repo's rule is reproduce-before-fixing and none of these has been. The
first is the most valuable thing in this document after the growth bug.

6. **Right-clicking the cell you built on does not always staff the building.**
   City 0 placed a forester with `click-cell 75 97`; `right-click-cell 75 97`
   did nothing at all — no refusal, no message, the people went idle — and
   `right-click-cell 76 98` worked instantly. Same with a farm placed at
   (74,102), staffable only at (75,103). It cost two game-days with a forester
   standing empty while the amber line said "nobody is cutting wood".
   **A silent failure on the game's most common gesture.** Start by checking
   what `World::place` does with the clicked cell versus what `occupancy` and
   `building_at` then contain — if a footprint is offset from the click, the
   ghost is drawn in one place and the building lands in another.
7. **A wall vanished with no announcement**, although M11.4 added exactly that
   message. City 1 watched almost its whole dike disappear between ages and
   never saw `mind_the_wall` fire. Two candidates: the segments went to
   `Rubble` while the player's attention was elsewhere and the notice was
   overwritten (see fault 2), or they were removed by something that is not
   `Rubble`.
8. **A segment that read `level 1 of 4` two minutes earlier refused a raise
   with "it is not built yet".** Either it was damaged in between, or a raise
   had already succeeded and returned it to a site — which is the M11.4
   behaviour and would mean the message is right and the *previous* raise was
   never noticed.
9. **City 1 got no score screen** — its window returned to the lobby at age 3
   day 6. `main.rs` returns to the lobby on Escape or Enter once the run is
   over, and an agent presses keys, so this may be benign. Worth ten minutes to
   rule out.

### C — Design questions the run raised. Decide, do not patch

These are not bugs. Each wants a paragraph in `DECISIONS.md` and possibly a
probe, and two of them may be answers rather than problems.

10. **Growing may be pointless in a three-age run even now it works.**
    `COMING_OF_AGE` is two ages, so only a child born in age *one* ever works —
    and age one is the age with no wood to spare for cottages and a day-four
    starvation clock. City 0's reasoning, and it is good. Either the payoff has
    to arrive sooner, or growth is a fourth-age feature the MVP should stop
    asking for. **A run is the instrument, not a probe.**
11. **The ground near a city is flat, so height has nothing to say.** Every
    cell either player could reach read 16 to 25. City 1: *"my whole reachable
    world is 16–18. On terrain with relief this would be the best verb in the
    game. Here it only told me I had no options."* And design §3.2's "get
    uphill" is an order neither player could obey. That is a finding about
    `map::terrain` and `SITE_HEADROOM`, not about the readout — `where_the_cities_sit`
    is the probe to point at it.
12. **The high-water mark is invisible exactly when it is wanted.** Two causes
    and both are real: it is a faint blue tint drawn under live water, so on a
    blue map it cannot be told from the flood; and **the water does not drain
    between ages** — city 1 spent age 3 days 1 and 2 reading "all quiet" while
    its farm still read `wading`, and lost two souls to standing water on
    nominally quiet days. Fixing the colour without fixing the drainage leaves
    the mark useless. Fixing the drainage changes the flood model, so measure
    `how_far_the_water_reaches` and `three_full_runs_of_each_strategy` either
    side of it.
13. **A wall is a line and the water arrives from every direction.** City 1:
    *"you would need a ring, roughly three times the cost."* Combined with 11,
    this is the shape of the wall problem M5 and M10 both circled: on flat
    ground with water on every side, no affordable wall exists. This may be the
    real answer to "is the wall worth building", and it is a map question as
    much as a dike one.

---

## Things that will bite you

The lists in `HANDOFF.md` and `HANDOFF-M10.md` all still hold. These are new
since M10 and every one cost real time this session.

* **The panel has now moved seven times.** `packaging/browser/panel.py` is the
  one copy of its running totals and anything new imports it. `play.py` and
  `assign.py` keep their own literals *on purpose*, as tripwires — when they
  fail after a layout change, that is them working. `two_agents.py` had copied
  the tick row instead of importing it and silently reported that neither peer
  was ticking; do not add a fourth copy.
* **Nothing may be drawn below `input::VARIABLE_FLOOR`.** The panel is full:
  before M11.2 the tools ended ten pixels above the foot, and a trade offer was
  being drawn over `tick`, `peers at` and `build`/`seed` — which is how M10.6's
  referee reported 116 stalls that never happened. `panel_rows.py` guards it.
  If you need vertical space, it has to come from somewhere; M11.2 bought 38
  pixels from the tools and 23 from the foot and that was all there was.
* **`TICKS_PER_SECOND` is a wall-clock knob and only stays one while
  `SURGE_TICKS` and `DROWN_TICKS` are pinned in ticks.** They read like
  seconds and are balance. `ages::the_clock_can_change_without_changing_the_game`
  asserts both values *and* their ratio.
* **A `Welcome` snapshot is 102 419 bytes against design §8's 150 KB.** There
  is about 47 KB of headroom for all future world state. The high-water mark
  cost 2 KB by being one bit a cell; a `u16` a cell would have cost 32.
  `wire::a_snapshot_is_a_sendable_size` prints the number.
* **Do not add a per-tick sweep of the map.** The first high-water mark had a
  pass of its own — sixteen thousand cells every tick, dry or not — and made
  the determinism test take longer than the rest of the suite. It is folded
  into the sweep `Water::step` already makes.
* **Do not reach a game state by simulating to it in a debug build.** Six days
  of two worlds takes over ten minutes. `World::tick` and `age_start_tick` are
  public: put the world where you need it. Both the `day 7 of 6` test and the
  high-water determinism test do this and run in under a second.
* **`nothing_the_game_draws_is_outside_ascii` is real.** macroquad's font draws
  a hollow box for anything else, and it caught an em dash on its way into a
  drawn string this session.
* **Player 0 is yellow and player 1 magenta**, not blue. Three browser checks
  find a player's own city by colour and all three had to move. Blue is seat
  six now, because `palette::water` is a mid-blue and a blue city vanished into
  the flood.
* **A raise looks like nothing happening unless you know.** Raising a dike adds
  a level and returns the segment to a *site*, so the row goes from
  `level 1 of 4` to `level 2 of 4, being built`. That is the raise working.

## The instruments

* `make test` — 286 tests, hermetic, no window.
* `make browser-test` — 17 checks in a real browser. Run before pushing
  anything touching `gui` or `web/`.
* **The probes are all `#[ignore]`**; run with `--ignored --nocapture`. The two
  that matter most here: `playtest::three_full_runs_of_each_strategy` is the
  five-strategy table and the thing to check either side of any balance change,
  and `families::what_a_fed_household_actually_manages` is the one that found
  the growth bug — it plays rather than feeds, and prints the number that
  matters if anybody moves `CHILD_FOOD` again.
* **The two-agent harness works and is the most valuable instrument here.**
  `table.py` seats two browsers with a debugging port each and stays up;
  `driver.py <port> <verb>` is one agent's hands; `referee.py <seconds> <dir>`
  watches both panels, issues no input, and says CLEAN or not.
  `AGENT-BRIEF.md` is everything an agent is told and **must be updated with
  anything you change that a player can see** — a brief describing the old game
  measures nothing. `HANDOFF-M10.md` has the full method.

## How to work here

Unchanged, and all of it earned. `make test` at every commit, no warnings. A
`sim` change ships with its test in the same commit; a `gui` change ships with
its browser check. **Reproduce before fixing** — group B above is exactly the
list that rule exists for. When a test fails, find out whether the test or the
code is wrong before changing either; roughly half the failures across these
sessions were a test encoding an old world. `DECISIONS.md` gets a paragraph for
anything somebody else would have written differently.

And the one this run added: **when you write a probe, write down what it
arranges.** `how_a_city_grows` was right about everything except the thing it
existed to check.

## The single next action

**Reproduce fault 6** — the silent right-click that does not staff a building.
It is the game's most common gesture, it fails without a word, and it cost a
player two game-days in a run that only lasts eighteen minutes. Write the
browser check that places a forester and staffs it at the clicked cell, watch
it fail, and only then look at `place`, `occupancy` and `building_at`.

Then group A in order. It is a session's work and it is all certain.
