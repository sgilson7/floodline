# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 6 — 2026-09-01 (M13: a city finishes a wall)

**The question the last three milestones exist for is answered.** 318 cargo
tests, no warnings. Two commits: the wall, and the five things the game knew
and would not say.

Earlier the same day, before this session: **the first three-player run**, whose
account is in `DECISIONS.md` under "What three players found" and "What three
players is actually for" and never reached this file. Referee CLEAN, three peers,
eleven days each. The lockstep held; a `driver.py` fault that put every named
button twenty-four pixels out did not.

### Why nobody had ever finished a wall

Not one of the three reasons was about walls, and none had been guessed in three
milestones of handovers. `what_a_walling_city_spends_its_days_on` — a line a day
— found all three in one run:

* **Hunger and exhaustion destroyed whatever a citizen was carrying.** 710 stone
  of 720, in one day: the day a starving city's granary first has food in it,
  when every hungry hauler drops its load at once. `Citizen::pause` keeps the
  arms; `abandon` stays for orders.
* **An assigned builder at an unsupplied site worked at nothing for ever.**
  `build` returns false while materials are outstanding, `work_at` did not look,
  and `busy()` then kept `find_work` from ever running again. Five of eight
  people stood at seven segments for four days at `0% done` with six hundred
  stone in a store twenty cells away.
* **A city that employed everybody starved beside its own farm.** Hunger
  outranks the job, and could not: a citizen with nowhere to eat now fetches the
  food itself. Without it, a city of farmers is nought of eight in four days.
* And a leak: every `Assign` went through `unassign_one`, which abandoned, so
  "you four, build the wall" burned a load of stone per hauler holding one.

A fourth change — the harvest outranking construction below a day's food — was
written, measured, and **taken back out**: with the three above in place it
changes no test and no run.

### The answer

| play | before | after |
|---|---|---|
| grow | 7 | 5 |
| dike | 7 | **9** |
| flee | 10 | **12** |
| both | 8 | **14** |

`both` — a wall and the sense to get behind it — is now the best play in the
game; it was worse than running away. On seed 31 the `dike` script got eleven
segments to level two standing before the age-one flood, paid for and hauled by
the eight people feeding themselves, and finished three ages with all eight
alive against `[8, 8, 4]` with no wall. **That is the first finished wall in this
project's history.**

`whether_a_finished_wall_changes_a_flood` hands a city one free and separates the
two questions four runs ran together: **23 alive against 17**, over six paired
runs of forty-eight people. It is worth having.

### What it cost, and what the distance decides

660 stone of 720 — ninety-two per cent of everything a city will ever have —
and it went up because that wall is **twelve cells from the hearth**. On the two
seeds where the wall is eighteen and nineteen cells out, no segment was standing
when the water came; the table carries `haul` now so this is a number rather
than a guess.

The reason is not the clock. On the far seed the wall is at 22 per cent on the
impact day and then **five of the eight drown** — the five carrying stone to it,
standing in the floodplain, nineteen cells from home. A wall far from home is
built by people in the water's way, and a city that does not also evacuate pays
for it with them. `both` keeps six of eight on that seed where `dike` keeps one,
so the game already rewards the answer; the price sentence now says `24 cells
each way`, and no player has ever seen it.

### And the panel pass

The wall's price counts the hauling now (twice the building, and the unmentioned
half was the larger one) and is written in the panel, not only in a map tooltip.
A disabled button says why. A building in transit says what is inside it, and the
ladder says where the food went. A dead city can read the ground and move the
camera again. `not on that ground` says *that is rock*.

**The selection box was accused and is innocent**: macroquad's `Rect::contains`
includes all four edges, so a drag along a row of people does take the row. The
fix was written, the test passed without it, and the fix came out.

### Not done

- [x] ~~`make browser-test` after this session's `gui` changes.~~ Green, exit 0,
      including `driver_check.py`'s hover row and the three-player checks. No
      panel row moved, and `panel.py`'s literals held.
- [ ] **A finished wall meeting a flood in a *played* game.** `sim` has now seen
      one. No person has.
- [x] ~~Why a far wall cannot be built.~~ Measured: twelve cells of haul gets
      the whole wall up, eighteen or nineteen gets none of it — and the reason
      is not the clock but that the five people carrying to it drown in the
      floodplain on the impact day. The game already rewards the answer (`both`
      keeps six of eight there where `dike` keeps one); what nobody has seen is
      a *player* being told the carrying distance before they commit.
- [ ] The author playing two games in a row, laptop to desktop.
- [ ] City 1's *"a line is the wrong shape, you would need a ring"*.
- [ ] `box-select`, still unexplained: an armed tool or a moved camera, neither
      reproduced.

### Next action

**Run `make browser-test`, then play it.** The wall is worth building and a city
can build one; nobody has ever watched that happen. The run to point at it is a
two-agent run where both players are told there is a wall worth having and
neither is told how much it costs to carry.

---

## Session 5 — 2026-08-31 (M12)

**Part I, Part I-A and Parts II-IV of `PLAN-M12.md` are done.** 298 cargo
tests, no warnings. The plan is `PLAN-M12.md`; every change and what it
dragged with it is `SECOND-ORDER-M12.md`, which is new and is where changes
are listed rather than approved.

### The lobby, which was the blocker

**A second game could not be joined, and it was not what the handover
guessed.** The handshake against a finished host is fine — it answers "this
game is full". The fault was one line up: `greet` was a `bool`, so a joiner
said `Hello` to the first peer it met and to nobody else, ever. A room is not
a star, and a non-host does nothing at all with a `Hello`. It is a
`BTreeSet` now, and `waiting_since` replaces `unanswered` so churn cannot
stop `mind_the_silence` speaking.

**The gap in the tests was not "a run played to its end" either.** It is that
no test anywhere had ever put a joiner in a room with more than one peer in
it. `rejoin.py` has that room now.

### Asked for during the session

* **A farm feeds a city** — `FARM_TICKS_PER_UNIT` 32 → 11. `both` went from
  nought survivors to eight; the single-verb strategies did not move. What
  lifted is the ceiling on doing more than one thing.
* **A builder's hut** — free, a roster rather than a bench. Its builders take
  a site first and haul second, which is the opposite of an unassigned
  citizen. A granary finishes in 71 ticks with one against 143 without.
* **A cookery**, and `Good::Meal` — one food in, one meal out, and a meal is
  worth two. The snapshot grew 60 bytes and the build hash changed.
* **A people tab and progress bars** — derived entirely from state that
  already exists, so no snapshot cost at all.
* **Ground that drinks** — material soak rates and an aquifer that deletes
  water. Four constants, every one measured, and three of them exist to keep
  M5's dike balance where M5 put it.

### What the run's other findings turned out to be

**Twice, a confident finding from M11.9 was the wrong diagnosis of a real
complaint.** The silent right-click was not a footprint offset — `sim` puts
every building exactly where it is clicked — it was a right-click with nobody
chosen returning without a word. And the ground is not flat: measured over
ten seeds, the climb within walking distance of a hearth is a median of
eleven terrain units against a surge twelve deep. It is *invisible*, and `h`
now shades it.

**Growth has never worked, and the `grow` strategy has never grown.** Its
plan had no nursery and nothing ever paired anybody into a cottage, so every
strategy table ever printed has had a column called `grow` that measured a
city with two spare cottages. Four gates, of which M12.A fixed one; the
binding one is that a household is "the lowest two ids currently homed here"
and the sim houses people by itself. And `COMING_OF_AGE` is twelve days of an
eighteen-day run, which decides it: the amber line stops asking.

### M12.11 — the run, played

**Referee: CLEAN.** 17 days in 18.2 minutes on both peers, zero samples with
no tick, zero red. City 1 finished three of eight and all three ages; city 0
finished none, at two. `PLAYTEST-M12.md` is the account; `HANDOFF-M12.md` is
what to do about it.

It found five faults introduced by M12 itself, all fixed, and the worst is
worth carrying: **neither player saw a single death.** Thirteen between them,
both watching for the line, both had read the brief. The line *was* drawn —
`driver.py panel` crops from sixty pixels above the foot and M12 put the new
slots at ninety-six and a hundred and forty. The message was fixed and moved
up the screen, and the crop that is an agent's eyes was never moved to match.

And it re-opened the question three milestones have been about: **nobody has
ever finished a wall.** City 0 put 440 stone into fifteen dike segments across
two ages and not one reached `level 1 of 4`; city 1 put in ~300 and read
`being built` a full age later. M10 said the wall was unreadable and M11 made
it legible — what both players were reading was a construction site nobody was
working on.

### Not done

- [ ] **Why 440 stone of delivered sites never got built.** An unassigned
      citizen builds when there is nothing to haul, and both players had idle
      hands and full sites for two ages.
- [ ] **A finished wall meeting a flood.** Nobody has seen one, in four runs.
- [ ] The author playing two games in a row, laptop to desktop. The `cargo`
      and browser checks say the lobby fault is fixed; nobody has confirmed it
      on two machines.
- [ ] City 1's *"a line is the wrong shape, you would need a ring"*, which
      arrived bundled with the flat-ground claim and does not fall with it.

### Next action

**Read `HANDOFF-M12.md`**, then find out why the sites never got built. It is
the reason four playtests have asked whether a wall is worth building without
anybody ever owning one.

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

---

## Session 5 — 2026-08-30

**M4's mechanism is built: there is a river, it has a ford, the cities are on
its banks and the flood comes down it.** 252 cargo tests, 14 browser checks,
no warnings, `make test` in about twenty-four seconds. One commit, on top of
M3's four.

* **`map::river`** — a meandering channel from the high side of the ramp to the
  low, both mouths on opposite map edges, carved before the ground bands and
  then painted water outright. A river is water because it is a river, not
  because it is low, and measuring the alternative is what said so.
* **`Ground::Ford`** — `nav::passable`'s fourth rule. Wadeable at half speed
  and six times the pathing cost, unbuildable except by a bridge, and it closes
  when the water comes.
* **Bank-relative sites** — chosen farthest-point from the band of cells
  `SHORE_DISTANCE` from the river, on alternating banks, with the band knowing
  about rock, about being walled in, and about how high a city stands above its
  own reach of river.
* **A river-mouth surge** — `Disaster::sources` is a list of pulse times rather
  than corners, the source is the channel's upstream reach, and the sea the
  water has to climb to leave is measured at the far end.

### What M5 inherits, said plainly

**The river flood is too gentle.** The five-strategy playtest went from one
survivor to sixteen for `grow`: two seeds in three now reach age three doing
nothing defensive. `dike` scores thirteen — *worse* than doing nothing —
because a wall is 450 builder-ticks a segment and the tallest anybody finished
before the age-one flood was one segment. A game where the wall is not worth
building is exactly what M5 is for, and it now has three probes to work with:
`when_the_water_arrives`, `how_far_the_water_reaches` (re-pointed at the bank)
and `dike_pressure_on_flat_ground`.

One city in twelve still never reaches wading depth. Everything else about the
placement is guaranteed by construction.

### Blocked

Nothing.

### Next action

**Finish M4, then M5.** M4's remaining item is the one thing above: re-running
the measurements the river invalidated and deciding what to do about a flood
nobody needs to defend against. That work is M5's by the plan's own division,
so the honest next step is to start M5 with the playtest table in hand rather
than to keep tuning inside M4.

`M6` (gold, the trading hut, mules) and `M7` (levels, moving buildings) are
independent of the flood work and can be pulled forward if trade is the part
worth playing with sooner — which the river has just made much more
interesting, since the cities are now on opposite banks.

---

## Session 6 — 2026-08-30

**M5 measured, and stopped short on purpose.** 252 cargo tests, no warnings.

`dikes::which_dikes_break` walls both banks at three distances across ten
seeds and reports what the flood takes. `DIKE_STRESS_LIMIT` is now
`[15_000, 48_000, 90_000, 145_000]`, which puts an age-one flood at 71% of a
level-one wall gone and 79% of a level-two wall standing — both in the middle
of the plan's target — and worse at age three. Seven seeds in ten hit both
bands against the plan's eight; the three that miss are maps whose flood is
weak or strong, not walls behaving oddly, and two attempts at narrowing that
are written up in `DECISIONS.md` so nobody repeats them.

Two rules changed because the playtest said the wall was unbuildable:
`Kind::Dike.build_ticks()` is fifty a cell rather than a hundred and fifty, and
`DIKE_RAISE_PERCENT` makes raising a level half the work of building one. On
one seed in three a diked city now finishes three ages with all eight alive
against an undiked six — the first time that has been true here.

### Parked, deliberately

On the other two seeds building a wall still costs the city the run, because
the labour has to come from somewhere and those maps have no slack in age one.
That is a question about the **food economy**, not about dikes, and the
instrument for answering it is two people playing a run — which is M10.

**The remaining balance work is listed immediately before M10** and blocks
nothing: M6 (gold, mules), M7 (levels, moving), M8 (job icons, workers
indoors) and M9 (families) do not touch the flood, and the plan says so of M6
and M7 outright.

### Next action

**M6 — gold, the trading hut and the mule.** Two sessions. `Good::Gold` first,
in one pass, before anything depends on it; then `Kind::TradingPost`,
`Job::Trader`, and `World::mules` as their own entity rather than a citizen
wearing a hat. The river has just made this the most interesting milestone
left: the cities are on opposite banks, so a mule has to find the ford or a
bridge, and "a mule that cannot get across says so in the panel" stops being a
corner case and becomes the common one.

---

## Session 7 — 2026-08-31

**M6, M7, M8 and M9 built.** 277 cargo tests, 14 browser checks, no warnings.
M5's residual is still parked before M10, and none of these four touched it.

* **M6 — gold, the trading post, the mule.** `Good::Gold` went in on its own
  first; `covers`, `total` and the panel's cost line now walk `Good::ALL` so a
  fifth good cannot be left off a price tag. A post's traders are mules on the
  road: one trader is one mule, it carries ten wood to the nearest other city
  and comes home with gold, it drowns like a hauler, and one with nowhere to go
  says so on the map and in the panel. Gold is *minted* by the exchange rather
  than moved — nothing else makes it, so a first trade would be impossible
  otherwise.
* **M7 — levels and moving.** A level is one more citizen the building can
  hold, one sentence for every kind. A move keeps the id, the store and the
  level and arrives as a site with its materials in it, so it costs time and
  not materials. The plan's "one more hauler based there" for stores could not
  be honoured: a hauler here is based nowhere, so levels are sold only where
  hands go.
* **M8 — silhouettes and going indoors.** Six shapes over the head, because the
  colour already says whose city this is. `nav::passable` gained one exception
  — you may go inside the place you are going to — and the flow field is seeded
  at the *middle*, which is what actually made three farmers stand on one
  corner. The crowd drifts a worker in and then spreads them through the
  inside.
* **M9 — families.** Two adults sharing a fed cottage for a day are a
  household; a fed household with a nursery place and a spare bed has a child
  on a timer; a child does not work and comes of age two ages later. A
  households tab lists them and hovering one rings its people on the map.

### Three bugs these turned up in earlier work

* **A road over the ford came out with a hole in it** and nothing said so:
  `lay_road` bridged `Ground::Shallows` exactly and a ford is water too, so
  every road cell laid on it was refused and `Road::intact` quietly said the
  cities were not linked. An M4 bug, found by M8.
* **`Job::produces` was answering the wrong question** about where a worker
  stands, so a trader arriving at its post had its workplace cleared and left a
  mule belonging to nobody. `Job::stationed` is the question that was meant.
* **`two_cities_found_a_road_and_trade_for_three_days` had drifted into the
  flood.** Founding two cities across a river takes four days now, so its third
  day of trading was the impact day and the water took the road — a failure
  that read as "trade moved nothing".

### Next action

**M10 — two agents, one game, played to the end.** `HANDOFF-M10.md` is the
onboarding document for whoever picks it up: where the game stands, what M5
left for it to answer, what already exists to build on
(`packaging/browser/game_two_tabs.py`), the traps, and the one decision to make
first — a run is thirty-six minutes of wall clock and the clock is fixed at ten
ticks a second, so either it is played at that speed or a test-only multiplier
gets written down.

The single next action inside it: two browser *contexts* (not two pages in one)
into one room on the deployed build, green and committed, before anything
decides what to build.

---

## Session 8 — 2026-08-31

**M10.1, M10.2 and M10.3 done, and the setup found three things nobody had
played long enough to see.** 278 cargo tests, 17 browser checks, no warnings.
`HANDOFF-M10.md`'s ten-milestone plan is now broken into eight steps in
`PLAN-M10.md`, ordered so nothing that can fail cheaply is allowed to fail
during the thirty-six minutes.

### The two decisions the handoff asked for

* **Real speed, no clock multiplier.** It closes itself: the done-condition
  names the deployed build and a multiplier must never reach a shipped build,
  so both cannot hold. It would also compress the variable under test, since a
  day is two minutes of *thinking* time.
* **Two browsers, not two contexts.** `DROP_AFTER_TICKS` is 300 ticks counted
  in the waiting peer's own ticks — thirty seconds of wall clock — and
  `MOST_PER_FRAME` is 8, so a page must render at least 1.25 frames a second.
  Chromium throttles backgrounded pages and two pages in one browser cannot
  both be in front, so a run would have been decided by which tab Chromium
  thought was visible.

### What the setup found

* **`peers at` said nothing in a browser.** It reported one number — the peer's
  own tick, which the row above it already said — because a page has one
  `Lockstep` and `Session::ticks` had nothing else to give. It was only ever
  true of the native build, where every peer's lockstep is in-process.
  `Lockstep::peer_ticks` fixes it from `seen_at`, with no wire change; two
  browsers now show `peers at [74, 71]`.
* **The panel's variable row still moved three fixed rows.** `8ead1b8` moved
  the level/move row and its comment says it "sits below everything fixed" —
  but the hover line, the selection row and the trade button were all still
  under it and all still shifted by forty-eight pixels whenever a building was
  picked. It is genuinely last now, and `panel_rows.py` is the check that says
  so — verified to fail on the old layout.
* **Only the host can notice a desync, and the joiner freezes without being
  told.** Both written up in `DECISIONS.md` and deliberately not fixed: the
  second is a shipped failure path that has never fired, and changing it on
  evidence from reading rather than playing is the kind of change this project
  has twice regretted. M10.8 answers them with a run behind them.

### Built

* `two_agents.py` — two browsers, one room, green against localhost and the
  deployed build. Checks something `game_two_tabs.py` does not: that both tabs
  are still *ticking* three seconds later.
* `table.py` — the one copy of the lobby dance, and the launcher that leaves
  two browsers standing with a debugging port each.
* `panel.py` — the one copy of the panel's running totals. `play.py` and
  `assign.py` keep their own literals on purpose.
* `driver.py` — an agent's hands and eyes over CDP, one command at a time,
  because an agent's turns are separate processes.
* `driver_check.py`, `panel_rows.py` — both in `make browser-test`.

### Blocked

Nothing.

### M10.4, and what the soak said

**The harness holds, and the clock is exact.** Three day-turns 121 and 120
seconds apart against a nominal 120, no stalls, nothing red, two browsers, the
deployed build.

**And a soak cannot be ten minutes.** Nobody feeds anybody, so both cities
starve on day four and an unattended game is over in about eight. That is a
constraint on M10.6 as much as on the soak: both agents have to be feeding
their people inside the first four days or the run ends on its own.

Two things would have made the referee useless, and running it caught both. It
reported twenty-six failures for a game that had finished normally, because an
ended game and a stopped page are identical at the tick row. And `WaitingOn` is
drawn in WARNING in the same row a desync uses ALARM, so "is it reddish" cries
desync every few seconds — which is how often lockstep waits.

### M10.5 — two agents played an age, and M5's question is answered

Twelve minutes, two browsers, the deployed build, neither agent able to see the
other's screen or read `crates/sim`. The full account is `PLAYTEST-M10.md`.

**The harness holds.** Seven day-turns 124, 119, 120, 119, 118 and 121 seconds
apart against a nominal 120; no stalls, nothing red, fifteen minutes with two
agents clicking and a flood in the middle. Thirty-six minutes is not in doubt.

**M5 parked the food economy for a run to answer.** City 0 built the wall and
came out of age one with three souls of eight, only two of whom drowned: *"the
dike cost me a day and a half of my only three free labourers, and that labour
is the same labour that feeds the city."* The wall is not too expensive in
stone — it spent 220 of 648 and never noticed. **It is paid for in food,
invisibly.** City 1 never saw the water at all: it sat fifteen cells up the
bank, the flood stopped seven cells short, and it starved anyway with its farm
staffed three-of-three. Neither had any way to know which of those two
situations it was in before choosing.

**Three findings were ours, not the game's**, and all three would have wasted
the run. The worst: `driver.py panel` cropped the panel and nothing else, so
the red line under the map that says *why* a click was refused was never in the
picture either agent was told to prefer. Both reported clicks that "silently
did nothing"; none of them was silent.

### And the one game fix pulled forward

Both cities died of the same blindness, so the food line was fixed before the
run rather than after it. The panel names the clock now — `1 food left, and 8
mouths eat 96 a day - under a day` — and a working building says what is
standing on it — `farm: 3 of 3 working, 1 food waiting`. No balance constant
moved; twelve a head a day is arithmetic on the eating model the game already
had.

### M10.6 and M10.7 — the run, played to the end

**"The map stood."** Three ages, eighteen days, 35.4 minutes on the deployed
build; city 0 with two souls of eight, city 1 with three, both standing.
**Neither client ever showed a desync banner** — 359 samples, zero alarm
pixels, and `peers at [21564, 21561]` at the final tick. The clock held to
119.8 seconds a day against a nominal 120. M10's done-condition is met.

The brief was left unchanged on purpose: the lesson the rehearsal pair died for
had been put into the *game* rather than into their instructions, so the run
tested the fix on players who did not already know. Neither city starved this
time, and city 1 quoted the new line back as the sentence that taught it the
system.

**The wall, answered twice more.** City 0 ran a clean experiment without being
asked: age 1 no wall, no deaths; age 2 a 43-cell dike costing its whole stone
and four days of hauling, six deaths and the wall broke; age 3 no wall, one
`m move` of the granary to high ground, no deaths through the worst flood.
*"Elevation beat masonry by a mile."* City 1, on the far bank, lost people only
to wall-building — once to the famine it caused and once to standing in the
floodplain hauling for it.

**But the wall is not underpowered — it is unreadable.** Neither player could
ask how high a cell is or where the water reached last time, and both chose
their wall lines by squinting at map colours. And a *raise* that works looks
exactly like nothing happening, so city 1 played a whole run believing the
interaction was broken. Nobody has ever tested a level-three wall in a played
game.

`PLAYTEST-M10.md` is the account, and it is an artifact.

### Next action

**Read `HANDOFF-M11.md`.** M11.1 through M11.9 are done — the clock halved, the
panel stopped drawing over its own instruments, the ground got a height and a
high-water mark, the wall got a strain readout, and the M11.9 run played three
ages with two agents and no desync.

That run found that **no city in this game could ever grow**, and that the one
probe pointed at growth was the reason nobody knew: `how_a_city_grows` sets
`c.food = NEED_FULL` every tick, so it had only ever tested a city that cannot
get hungry. `CHILD_FOOD` was `FED_ENOUGH`, which is the level a citizen *stops
eating* at, so a household never got past 99 of the 1 200 consecutive ticks it
needed. Fixed, with a test that plays rather than feeds.

Thirteen further findings are ranked in the handover: five confirmed and small,
four reported but **not reproduced**, four design questions.

**But the first priority is not one of them.** Playing laptop-to-desktop, the
author got through one game and has not been able to get two machines into a
lobby since: the joiner sits for ever on *"found the host, asking for a
city..."*. That means it has met a peer and sent its `Hello` and never received
a `Welcome` — a handshake failure, not a transport one. `mind_the_silence`
should be saying so after a few seconds and never does, and `peer_left` resets
`unanswered` to nought on every departure, which would keep the timer from ever
firing in a room with churn.

Nothing tests it: `rejoin.py` is lobby-only and no check anywhere plays a run to
its end and then puts two peers into a lobby again. The `cargo` reproduction
needs no browser — run a loopback game to `finished`, then try to join it.
