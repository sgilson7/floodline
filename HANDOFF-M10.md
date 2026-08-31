# FLOODLINE — handing over at M10

`HANDOFF.md` is the guide to the *codebase* and is still true of it.
`HANDOFF-M2.md` is the guide to the plan and is now out of date only in its
table. This is the guide to **where the ten-milestone plan actually stands and
how to set up the one thing left**. Read `CLAUDE.md`, then `HANDOFF.md`, then
this.

---

## Where it is

**Live at <https://sgilson7.github.io/floodline/>**, and `main` and the
deployed build agree.

277 cargo tests, 14 browser checks, no warnings. `make test` ~30s,
`make browser-test` ~6 minutes.

The plan is an artifact and is still the plan of record:
**<https://claude.ai/code/artifact/f51d6368-d9a0-48f2-9213-c41f1eba59b1>**

This document is also an artifact, if a link is easier to hand on than a file:
**<https://claude.ai/code/artifact/44221090-95f3-42cf-a451-efeb3b07e106>**

| | | |
|---|---|---|
| M1 | Forester costs stone | **done** |
| M2 | A camera over the map | **done** |
| M3 | Dikes: 3×1, drawn, breakable | **done** |
| M4 | The river and the wave | **done** |
| M5 | Tuning: which dikes break | **measured; one item parked, see below** |
| M6 | Gold, the trading hut, mules | **done** |
| M7 | Levels, and moving buildings | **done** |
| M8 | Job icons, and workers going indoors | **done** |
| M9 | Families, children, a nursery, a households tab | **done** |
| M10 | Two agents, one game, played to the end | ← **you are here** |

Nothing is half-written. Every commit is green and pushed.

---

## The game you are about to play

It is a different game from the one `HANDOFF.md` describes, and the differences
are the point of M10.

* **There is a river.** A meandering channel is carved from the high side of
  the ramp to the low, both mouths on opposite map edges. It is water along its
  whole length, it is impassable, and it divides the map.
* **Cities are on its banks**, alternating sides, 10–18 cells from the water and
  within `SITE_HEADROOM` of the bed. So there is a river *between* the players,
  and a bridge, a road and a trade have a reason to exist for the first time.
* **There is exactly one ford** per map: water you can wade at half speed and
  six times the pathing cost of open ground, unbuildable except by a bridge.
  **It closes on the impact day** — the crossing you have been relying on goes
  under with everything else.
* **The flood comes down the channel**, held at the upstream mouth for
  `SURGE_TICKS`, and spills over the banks. Age three sends two pulses about
  half a day apart.
* **Dikes are drawn**, not placed: press `7` and drag. Three-cell segments, a
  ghost and a running cost under the cursor. They accumulate stress from the
  water leaning on them and break when it passes a threshold their level sets.
* **There is gold**, a trading post, and mules on the road. Gold buys levels; a
  level is one more citizen the building can hold.
* **There are families.** Two adults sharing a fed cottage for a day are a
  household; a fed household with a nursery place and a spare bed has a child;
  a child works two ages later. No nursery, no children.

The build menu is now `1` cottage, `2` farm, `3` granary, `4` forester,
`5` quarry, `6` stockpile, `7` dike (drag), `8` trading post, `9` nursery, plus
`r` road and `p` point. Click one of your own buildings to select it, then the
level button or `m` to move it. The panel has two tabs: **build** and
**households**.

---

## What M5 left for you, and why it is M10's question

M5's break probe hit its target — an age-one flood takes 71% of a level-one
wall and leaves 79% of a level-two one standing, and both are worse by age
three, on 7 seeds in 10. The numbers and the table are in
`balance::DIKE_STRESS_LIMIT`.

**What it could not settle is whether a wall is worth building.** At the old
price it plainly was not: every `dike` run in `playtest.rs` died before the
water arrived, because a wall long enough to shield a city is about forty cells
and half the city stood on the bank for two days a long walk from the granary.
Cutting `Kind::Dike.build_ticks()` to fifty a cell and making a raise half the
work of a build fixed that far enough that on one seed in three a diked city
now finishes three ages with all eight alive against an undiked six — the first
time that has ever been true here.

On the other two seeds building the wall still costs the city the run. **The
cause is measured and it is not a dike problem: it is the food economy.** The
labour has to come from somewhere and those maps have no slack in the first
age. How much a farm yields and how many hands a city of eight can spare are
numbers nobody has ever watched a person spend, which is exactly what M10 is
for. Do not tune them from a probe before the run; read the run.

---

## M10: two agents, one game, played to the end

The plan's deliverables, unchanged:

| | |
|---|---|
| Two independent clients | Separate browser contexts, separate agents, one room code passed between them the way two people would |
| A full run | Three ages, both cities, both floods survived or lost on their merits |
| Checksums agree throughout | The panel shows both peers' ticks; a desync at any point is the headline finding |
| A written account | What each city did, what the flood did to it, and what was actually fun — design step 7, finally answered by something that played |
| Everything since M1 exercised | Camera, dikes, river, gold, mules, families: all of it in one game, by two hands |

**Done when** two agents finish a run on the deployed build and neither client
ever showed a desync banner.

### What already exists to build on

`packaging/browser/game_two_tabs.py` does the whole lobby dance already and is
the file to start from. It:

* launches Chromium, makes one context, opens two pages,
* picks a random room name and types it into both,
* clicks Host on one and Join on the other,
* waits on `window.FLOODLINE_RTC.debug()?.links.size > 0` on both,
* clicks Start, dismisses the modal first-run card on both,
* and checks both tabs left the lobby into a drawn map.

Its lobby coordinates are literals copied from `crates/gui/src/lobby.rs`, kept
that way on purpose: if the lobby moves, the check should notice.

`packaging/browser/view.py` is the one copy of the letterbox-and-camera
arithmetic. **Every script that clicks the map imports it.** Do not write a
second one.

`packaging/browser/play.py` and `assign.py` are worked examples of driving the
game with a mouse and reading the answer off the screen.

### What the client tells you, and it is all you get

An agent playing this sees what a player sees. There is no hook that reads the
world out of the page — deliberately, because "it sees what its own client
shows and decides for itself" is the point. What is on the panel:

* `age N of 3    day N of 6`, the omen line, and the treasury
  (`food / wood / stone / gold`).
* One line per city with its population.
* The tutorial line, which always names the next thing to do and is the fastest
  way for an agent to orient itself.
* The status line: `playing`, `waiting on city N`, or
  **`DESYNC with city N at tick T`** in red.
* At the bottom: `tick N`, `peers at [N, N]`, `build <hash>  seed N`.

`peers at` is the two peers' tick counts side by side. Watch it: if they part,
or the desync banner appears, **stop and write it down** — that is the headline
finding and it outranks the rest of the run.

At the end the score screen draws over everything with "The map stood." or
"The last city fell."

### The one number that shapes the whole exercise

**A run is 36 minutes of wall clock.** Three ages × six days × 1200 ticks is
21 600 ticks, and `crates/gui/src/main.rs`'s `Clock` runs the simulation at a
fixed ten ticks a second on purpose — a day is two minutes and nothing about
that is negotiable from outside the binary. `MOST_PER_FRAME` caps catch-up at
eight ticks a frame, so a backgrounded tab does not sprint.

That leaves a decision, and it is the first one to make and write down:

1. **Play it at real speed.** Thirty-six minutes of two agents taking turns,
   which is honest and is what a person would experience. Poll every few
   seconds; most ticks want no decision at all.
2. **Add a test-only speed multiplier.** `TICKS_PER_SECOND` is a `sim` constant
   and everything counted in ticks scales with it consistently, so multiplying
   the *clock* — not the constant — in `gui` is the small version:
   `Clock::ticks_due` returning `n × due`. It has to be the same on both peers
   or the slower one is permanently the one everybody waits for, and it must
   never reach a shipped build. If you do this, `DECISIONS.md` gets a paragraph
   saying it is a test affordance and how it is kept out of the deployed page.

Option 2 is tempting and option 1 is what the milestone is asking for. Pick
deliberately rather than by accident.

### Setting it up, in the order that will work

1. **Two contexts, not two pages in one.** `game_two_tabs.py` uses one context
   with two pages, which is fine for a transport check and wrong here: two
   agents should not share a clipboard, a localStorage or a permission grant.
   `browser.new_context()` twice.
2. **The room-code path, not the pasted code.** It is two clicks and a typed
   string on each side. The pasted-code path exists for players behind strict
   NATs and has nothing to teach a playtest.
3. **The same build on both.** The room name carries the wasm's sha256 (design
   §9.4). A local build and the deployed build *cannot* join each other and the
   reason will not be obvious at the time — the tab just never finds the game.
   Test against one or the other, not both.
4. **Dismiss the first-run card on both tabs.** It is modal and covers the map.
5. **Then play.** One agent per context. Each screenshots its own page, reads
   the panel, decides, clicks. Neither may read the other's page — that is the
   whole difference between this and the two-tab check.

### What to write down as you go

The account is a deliverable, not a by-product. Keep it as you play, not after:

* What each city built, in what order, and why.
* When the water arrived, how deep it got, and who it took.
* Whether the wall was worth building — the M5 question, answered by a person's
  hands rather than a probe.
* Anything that was confusing, unreadable at the fit, or silently did nothing.
  Design step 7 is "playtest the flood until it is fun", and the fastest way to
  fail it is to record only what the numbers say.

---

## Things that will bite you

The ones from `HANDOFF.md` and `HANDOFF-M2.md` all still hold. These are new
since M2 and every one of them cost real time.

* **The panel has now shifted five times, and each time it silently broke two
  browser checks.** The symptom is a check clicking a gap and reporting
  something unrelated as failing. `play.py` and `assign.py` carry panel
  y-coordinates as literals; `assign.py` also crops the hover row by hand. The
  variable row (the level/move buttons, which appear only when a building is
  selected) now sits **below everything fixed** precisely so it cannot move
  anything — keep it there.
* **`nav::passable` has four rules now and four readers.** Rock and shallows
  are closed, a dry ford is open, and **you may walk inside the building you are
  walking to** (`nav::passable_into`). Pathing, the crowd, the flood and
  road-laying all read it. Change all four together.
* **Arriving is *at* a building, not *in* it.** Making a worker arrive only once
  it was on the footprint reads exactly like what M8 asked for and starves
  cities: a farmer that has not arrived is still `Walking`, and a city
  permanently on its way somewhere eats and sleeps at the wrong times. What
  puts a worker inside its farm is the crowd's inward drift.
* **A flow field for a building is seeded at its *middle*.** Seeding every cell
  makes the field flat across the footprint, which is what made three farmers
  stand on one corner for eight months.
* **Gold is minted by the exchange, not moved.** Nothing else makes it, so if
  trade only moved it the first trade could never happen. Do not "fix" this.
* **Gold is not hauled** (`Good::hauled`), and the barter dialog will not offer
  it. Barter is still design §6's standing daily exchange walked by people.
* **A dike segment is priced per cell.** `Kind::Dike.cost()` and
  `build_ticks()` both scale with `DIKE_LENGTH`. Making a segment three cells
  long without that cut the price of a wall to a third overnight and no
  assertion caught it — the five-strategy playtest did.
* **`Job::produces` and `Job::stationed` are different questions.** A trader
  makes nothing and still stands at its post holding a slot. Getting this wrong
  left a mule on the road belonging to nobody.
* **`bear_a_child` is the only thing that adds a citizen, and it appends.** Ids
  are indices into `World::citizens` in half a dozen places and the crowd, the
  flood and every roster iterate it. Appending is safe; reordering or reusing an
  id is not.
* **`Building::slots_for`, `beds` and `places` are on the *building*, not the
  kind**, because they read the level. Anything that asks `Kind` how many people
  fit is asking the wrong object.
* **A road over the ford used to come out with a hole in it** and nothing said
  so, because `lay_road` bridged `Ground::Shallows` exactly. It bridges anything
  `watery` now. If you add a third kind of water, come back here.

---

## The instruments

Eight probes, all `#[ignore]`, all measurements rather than assertions. Run them
with `--ignored --nocapture`:

| probe | what it answers |
|---|---|
| `map::probe::where_the_cities_sit` | spacing, distance to the bank, rock in reach, headroom over the bed |
| `map::probe::what_the_river_costs` | how much map the channel takes and how much of it the bands would have made water on their own |
| `map::probe::sweep_noise_amplitude` | the terrain sweep, unchanged since phase 1 |
| `dikes::dike_pressure_on_flat_ground` | what a surge does to a wall, level by level, on flat ground |
| `dikes::which_dikes_break` | **M5's instrument**: fraction broken by level, age and distance, over ten seeds |
| `playtest::how_far_the_water_reaches` | depth by distance from the bank, both surge heights |
| `playtest::when_the_water_arrives` | when the wave reaches each hearth, its peak, and when it drains |
| `families::how_a_city_grows` | population over three ages, fed and unfed |

`playtest::three_full_runs_of_each_strategy` is the five-strategy table and is
the closest thing to M10 that a machine can do. It is not a substitute: it plays
one city against no opponent with a script that knows what it is doing.

`cargo test -p sim --release --test profile -- --ignored --nocapture` reports
0.75 ms a tick at 500 citizens with the flood running, against a 20 ms budget.

---

## How to work here

Unchanged, and all of it earned:

* `make test` at every commit, no warnings. `make browser-test` before pushing
  anything that touches `gui` or `web/`.
* **Measure numbers, do not pick them.** Every balance constant that is a
  judgement carries its measurement in its doc comment. Add a probe, run it,
  paste the table.
* **When a test fails, find out whether the test or the code is wrong before
  changing either.** Roughly half the failures across these sessions were a test
  encoding an old world, and saying so in the comment is worth more than the fix.
* **Reproduce before fixing.**
* `DECISIONS.md` gets a paragraph for anything somebody else would have written
  differently. It is long and it is in date order; the last dozen entries are
  about M3 to M9.
* One milestone at a time, and `PROGRESS.md` updated at session end with phase,
  done/not done, decisions, blockers, and the single next action.

---

## What has never been tested

* **Nobody has played a full run.** Two tabs on one machine have reached the
  same world and a person has clicked around for a few minutes; a run to age
  three has never been finished by anybody, agent or human. That is M10.
* **Two browsers on two networks have never talked**, except through this one
  machine. The case that needs TURN — both ends behind strict NATs — is by
  definition untested and is the only thing here that can cost money to fix.
* **More than three peers has only been tested on `Loopback` and in
  `echo.html`.**
* **Nothing since M3 has been played at all**, only tested and measured. The
  dike drag, the river, the ford, the mules, the levels, the moving, the
  silhouettes and the families have every one of them been exercised by a test
  and none of them by a person deciding what to do next. Expect to find things.

---

## The single next action

Copy `packaging/browser/game_two_tabs.py` to something like
`packaging/browser/two_agents.py`, change it to two contexts, get both tabs into
one room on the deployed build, and stop there. Get *that* green and committed
before writing a line of anything that decides what to build — the lobby is the
part with the known traps, and the playing is the part that is supposed to be
interesting.

Then decide, and write down, whether the run happens at real speed or with a
clock multiplier. Everything after that is playing the game.
