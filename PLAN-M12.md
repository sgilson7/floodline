# FLOODLINE — the plan for M12: a game you can start twice

M11 made the world legible, played it again with two agents, and wrote
`PLAYTEST-M11.md` and `HANDOFF-M11.md`. This is what those ask for, as
milestones.

This document is also an artifact, if a link is easier to hand on than a file:
**<https://claude.ai/code/artifact/bd011be2-a71a-4e0f-adc9-2b7927a52839>**

M11.1 through M11.9 are done and closed. Thirteen findings are ranked in the
handover — five confirmed, four reported but not reproduced, four design
questions — and above all of them sits one thing the playtest did not find:
**two machines can no longer get into a lobby together after a game has been
played.** That orders this plan.

---

## The one sentence this plan comes from

> A game two people cannot start twice is not a game.

M10's sentence was "the wall is not underpowered, it is unreadable", and M11
spent nine milestones answering it without touching a balance constant. This
plan has no such single theme, because the run that produced it found three
different kinds of thing: a blocker in the lobby, a handful of readouts that
lie, and a set of questions about the map that the game has been avoiding
since M5.

So the ordering principle is:

1. what stops the game being played at all,
2. what fails silently, because a silent failure teaches a player the wrong
   model of the game,
3. what cost people their lives,
4. what cost them time,
5. what has to be decided rather than patched.

## The lesson M11 paid for, which governs everything below

> **A probe that arranges the condition it is measuring can only confirm
> itself.**

`families::how_a_city_grows` set `c.food = NEED_FULL` every tick, so for three
playtests it reported healthy growth in a game where no city could grow. Every
measurement in this plan states, in its doc comment, **what it arranges**. Any
milestone below that adds a probe and does not write that sentence is not done.

## What is already done, so nobody redoes it

* **Growth.** `CHILD_FOOD` is `NEED_FULL / 10`, households settle in an
  ordinary fed city, and `a_household_in_a_fed_city_settles_without_being_force_fed`
  plays rather than feeds. Fed cities reach 10 and 12; unfed ones stay at eight
  or die.
* **All nine M11 legibility milestones.** The clock, the panel floor, ground
  height, wall strain, sending some of the people, deaths with causes, goods in
  transit, the persistent refusal, the fit. Two of these — goods in transit and
  the persistent refusal — both players named unprompted as the best of them.
  Do not undo either.
* **The two-agent harness.** `table.py`, `driver.py`, `referee.py`. It works,
  it found the growth bug, and it is the most valuable instrument here.

## What is *not* here

* **No new features.** Every milestone below is a fault, a measurement or a
  decision.
* **No balance constant moves without a table either side.** Three milestones
  (M12.8, M12.9, M12.10) may move one. Each says so, and each pastes
  `three_full_runs_of_each_strategy` into its doc comment before and after.
* **Nothing new drawn below `input::VARIABLE_FLOOR`** without first saying
  where the pixels came from. M11.2 bought 38 from the tools and 23 from the
  foot, and that was all there was.

---

# Part I — the lobby

Nothing in Part II or later is worth starting before this is done.

## M12.1 — Reproduce it

First, because the house rule is reproduce-before-fixing and because the
reproduction says which of the three suspects it is. **No fix in this
milestone.**

What is known: `lobby.rs::joining_screen` shows *"found the host, asking for a
city..."* when `connected() > 0 && !roster_empty()` and not `welcomed()`. So
the joiner has a live peer and a roster, has sent its `Hello`, and never
received a `Welcome`. That is a handshake failure, not a transport one. The
room name carries the build hash and the typed code, so the same code typed
twice on the same build is the *same room* — which is what a person playing
against themselves would do every time.

**Deliverables**

* `crates/net/tests/lockstep.rs::a_second_game_can_be_joined` — the loopback
  star, a host and a joiner, the world put where it needs to be with
  `World::tick` and `age_start_tick` rather than simulated to it, run to
  finished; then a fresh `Lockstep::join(build)` against the same host, driven
  for enough frames to settle. Assert on what the joiner ends up *knowing*:
  either welcomed, or refused with a reason it can read. Sitting silent is the
  failure.
* If that passes, the fault is in `net-web` or the room, and the reproduction
  moves to the browser:
  `packaging/browser/rejoin.py::a_room_that_held_a_finished_game` — three tabs,
  which `rejoin.py` already has the machinery for. Host, join, start, drive to
  the score screen, then put a third tab into the same room code.
* A paragraph in `DECISIONS.md` naming **what the correct behaviour is**,
  chosen before the fix is written. Three candidates:
  1. a finished host returns to its lobby and hosts again in the same room;
  2. a finished host *leaves* the room, so a joiner finds nothing and is told
     so;
  3. the room name carries a per-game nonce, so a finished game's room is
     never the room a new game is in.
* Which of the three ranked suspects it actually was, in writing. Rule out the
  build hash first — it is one glance at both lobbies.

**Done when** a check in `cargo` or in the browser fails on today's build for
the reason the author saw, and the cause is named.

**If it will not reproduce** after a session, say so in `DECISIONS.md` with
what was tried, and go to M12.2 regardless — M12.2 is worth having whatever
the cause.

## M12.2 — A joiner is never left in silence

Independent of the root cause, and worth doing on its own: it converts a
silent hang into a sentence a player can act on. **This is the best clue in the
handover and it is also a bug in its own right.**

The fault, exactly: `mind_the_silence` counts only while `host_peer.is_some()`
and fires at `unanswered == SILENCE_FRAMES` (500). But `peer_left` sets
`unanswered = 0` every time the greeted peer goes away. In a Trystero room with
any churn — a ghost joining and leaving, a stale tab reconnecting — the count
never reaches 500. The joiner greets, waits, resets, greets again, and is never
told anything.

**Deliverables**

* A counter that never resets: frames since the joiner first met anybody while
  unwelcomed. `peer_left` may reset `unanswered`; it may not reset this one.
* `mind_the_silence` speaks off that counter, and speaks **even when
  `host_peer` is `None`** — a joiner that met somebody and lost them is
  precisely the case reported.
* `crates/net/tests/lockstep.rs::a_joiner_that_meets_a_ghost_is_told_so` —
  greet, peer leaves, another arrives and leaves, repeated past
  `SILENCE_FRAMES`; assert `trouble()` is set.
* The message covers the new case. It currently blames a host that left its
  lobby; it must also fit "there is somebody in this room, but nobody hosting".
* A browser check: join a room code nobody is hosting, read the line off the
  panel. At 500 frames this is about eight seconds — cheap.
* One sentence in `DECISIONS.md` on whether frames should now be seconds, given
  `Clock` exists. "No, and here is why" is an acceptable answer.

**Done when** a joiner that will never be welcomed says so within about ten
seconds, in `cargo` and in a browser, however much the room churns.

## M12.3 — Fix it, and guard it for good

**Deliverables**

* The fix M12.1 named, wherever it lands — `net`, `net-web`, `web/quad_rtc.js`
  or `gui`.
* M12.1's failing check passes.
* `rejoin.py` keeps the end-to-end case permanently: **a run played to its end,
  then two peers into a lobby again.** This check has never existed anywhere,
  in `cargo` or in a browser, and that gap is exactly why the fault survived
  M10 and M11.
* If the answer is "a finished host returns to its lobby", then `main.rs` drops
  the session when the run *ends*, not only on Escape or Enter at the score
  screen — which also disposes of group B fault 9.
* `DECISIONS.md` paragraph.

**Done when** `make browser-test` is 18 or more checks and green, and the
author has played two games back to back, laptop to desktop, without changing
the room code or restarting a browser.

---

# Part II — what fails silently, and what killed people

The handover calls this "about a session's work" and it is all certain except
the first, which is reported and must be reproduced.

## M12.4 — The silent right-click

Group B fault 6, and the first thing to chase after the lobby: **a silent
failure on the game's most common gesture.** A forester placed at (75,97) would
not staff at (75,97) — no refusal, no message, the people went idle — and
staffed instantly at (76,98). Two game-days lost while the amber line said
"nobody is cutting wood".

The suspect, from the code: `Building::footprint`'s origin is the *top-left*
cell. If `place` stores the clicked cell as the origin while the ghost is drawn
centred on the cursor, the building lands up and left of where the player
aimed, and `building_at(clicked)` finds nothing.

**Deliverables**

* Reproduction in `cargo` first:
  `crates/sim/tests/commands.rs::a_building_is_where_it_was_clicked` — for
  every `Kind` and both facings, place at `(x, y)` and assert
  `building_at(x, y)` is that building.
* If `sim` is innocent, the reproduction moves to `gui`:
  `packaging/browser/assign.py` gains "place at a cell, then right-click that
  same cell".
* The fix.
* **Every right-click either changes the world or says why not.** The staffing
  path in `input.rs` returns without a word when `building_at` is `None`. That
  is the deeper fault, and it stands even after the offset is fixed: the
  gesture that stalled a player for two days did so by being indistinguishable
  from a gesture that worked.
* A browser check that the refusal reaches the panel.

**Done when** a building placed at a cell can be staffed at that cell in a real
browser, and no right-click anywhere in the game is answered with silence.

## M12.5 — Deaths, and how deep the water is

Group A faults 2 and 3. Both cost lives in the run; the first is the readout
the handover says most needs fixing.

City 0 lost eight people and read "1 drowned" throughout. City 1 never saw a
death message at all, because its own click-refusal overwrote it in the same
frame. `input.rs::say` writes to one slot, `self.notice`, and deaths arrive
several to a frame.

And: *"the only thing on the whole screen that distinguishes survivable water
from lethal water. It changed exactly one decision and by then everyone was
dead."* — `water: wading` versus `out of your depth` exists only under the
cursor. The panel's warning "8 of your people are in the water" does not say
how deep.

**Deliverables**

* **Deaths get their own line.** A second slot that a refusal cannot
  overwrite, accumulating over a window of a few seconds and saying the total:
  "3 drowned, 1 starved". The count over a run must add up to the dead.
* **Where the pixels came from**, in writing, before anything is drawn.
  `panel_rows.py` gains a case with a toll, a refusal and a trade offer
  pending at once, and it guards `VARIABLE_FLOOR` as it does now.
* **Depth in the panel during a flood.** The in-the-water warning carries the
  depth of the deepest of them, in the same words the hover readout uses.
* `packaging/browser/panel.py` — the one copy of the running totals — gains
  the new row. Do not add a fourth copy; `play.py` and `assign.py` keep their
  literals as tripwires on purpose.
* A browser check that a death count is still readable after a refusal in the
  same frame.
* `AGENT-BRIEF.md` updated. A brief describing the old panel measures nothing.

**Done when** a city that loses eight people reads eight, and a player in a
flood can tell survivable water from lethal water without moving the cursor.

## M12.6 — Small honesties, second pass

Group A faults 1, 4 and 5. Cheap, certain, and between them a large part of
the run's confusion.

**Deliverables**

* **`back to hauling` stops leaving people reading `idle`.**
  `input.rs::households` labels `job == None` as idle, and an unassigned
  citizen *is* a hauler — that is what hauling is. Both players hit it and one
  re-pressed the button for two days believing it had done nothing. The label
  follows the game: unassigned reads "hauling". If a genuinely idle state
  exists, it gets its own name.
* **Two refusals that lie**, both in `sim`'s command rules and both shipping
  with a test in `crates/sim/tests/commands.rs`:
  * a cottage that is not built yet says **"it is not built yet"**, as the dike
    correctly does — not "no beds left there", which is what a *full* cottage
    says;
  * a nursery says what a nursery is, not "no room left there", which sent a
    player looking for a capacity problem in a building that is not a workplace
    at all.
* **`tutorial::next_thing` stops thinking after food.** It spent age 2 telling
  both players to build a trading post — one of them with a stalled household,
  no children and a flood two days out. Two more rungs on the ladder: a
  household that has been settling for days with nothing else wrong, and a
  flood within two days. A trading post is never the next thing while either
  is true.
* A browser check per visible change, and `AGENT-BRIEF.md` updated.

**Done when** no line in the panel says something the game does not mean, and
the amber line's advice in age 2 survives being read out loud.

---

# Part III — what was reported and never reproduced

## M12.7 — Reproduce or dismiss, time-boxed to one session

Group B faults 7, 8 and 9. The rule exists for exactly this list. Each ends in
one of two states: **a failing test and a fix, or a paragraph in
`DECISIONS.md` naming what was tried and what was ruled out.** Neither may be
left a rumour.

**Deliverables**

* **7 — a wall vanished with no announcement**, though M11.4 added the
  message. Check this *after* M12.5 and it may already be gone: the notice
  being overwritten is the same fault as the lost deaths. If it survives,
  `crates/sim/tests/dikes.rs` gets a check that every route by which a segment
  stops existing fires `mind_the_wall` — the second candidate is that the
  segments left by a route that is not `Rubble`.
* **8 — `level 1 of 4` refusing a raise with "it is not built yet".** Most
  likely the M11.4 behaviour, in which case the message is right and the fault
  is that a raise looks like nothing happening. Then the fix is the *wording*:
  a segment part-way through a raise is not a segment nobody has started, and
  should not borrow its sentence.
* **9 — a client that got no score screen** and returned to the lobby. Ten
  minutes, and M12.3 may have already answered it. An agent presses keys and
  the lobby is one keypress away, so "benign, and here is why" is a legitimate
  outcome.

**Done when** nothing in group B is still a rumour.

---

# Part IV — the questions the map has been avoiding

Three decisions, not three patches. Each wants a paragraph in `DECISIONS.md`,
each may move a balance constant, and each therefore pastes
`three_full_runs_of_each_strategy` into its doc comment **before and after**.

## M12.8 — Water that drains, and a mark you can see

Group C fault 12, and it is two causes that have to be fixed together.

The high-water mark is a faint blue tint drawn under live water, so on a blue
map it cannot be told from the flood. And **the water does not drain between
ages**: city 1 spent age 3 days 1 and 2 reading "all quiet" while its farm
still read `wading`, and lost two souls to standing water on nominally quiet
days. Fixing the colour without the drainage leaves the mark useless; fixing
the drainage changes the flood model.

**Deliverables**

* The measurement first, on today's build:
  `playtest::how_far_the_water_reaches` and
  `three_full_runs_of_each_strategy`, pasted into the doc comment, each
  stating what it arranges.
* The decision, in `DECISIONS.md`. Candidates: drainage continues between
  surges at a higher rate; the age boundary drains what is left; or nothing
  changes in the model and the panel stops calling those days quiet.
* The mark gets a colour water is not. M11.8's lesson was that a blue city
  vanished into the flood; a blue mark on blue water is the same mistake.
* The same two probes after, in the same doc comment. **If the strategy table
  moves, that is a balance change and it gets its own paragraph.**
* **No per-tick sweep of the map.** Whatever this costs folds into the sweep
  `Water::step` already makes — the first high-water mark had a pass of its
  own and made the determinism test the slowest thing in the suite.
* The snapshot budget. There is about 47 KB of headroom against design §8's
  150 KB; `wire::a_snapshot_is_a_sendable_size` prints the number. A bit a
  cell is affordable, a `u16` a cell is not.

**Done when** a day the panel calls quiet is a day nobody drowns on, and a
player can see where the water reached last time while standing in this time's.

## M12.9 — Ground a player can climb

Group C faults 11 and 13, which are one question. **The largest item in this
plan and the one most likely to end in a written deferral, which is a
legitimate outcome.**

Every cell either player could reach read 16 to 25. City 1: *"my whole
reachable world is 16–18. On terrain with relief this would be the best verb in
the game. Here it only told me I had no options."* Design §3.2's "get uphill"
is an order neither player could obey. And city 1's other sentence — *"a line
is the wrong shape for this terrain; the water arrives from every direction
across ground that is uniformly 17 high, you would need a ring, roughly three
times the cost"* — is the same finding wearing a different hat. **On flat
ground no affordable wall exists.** That is a map fault, not a dike one, and it
may be the real answer to the question M5 and M10 both circled.

**Deliverables**

* The measurement first, and it is the point of the milestone: a probe beside
  `map::where_the_cities_sit` printing, per seed over ten seeds, the spread of
  heights within a citizen's working radius of each hearth. **"16 to 25" is a
  claim from two runs, not a measurement.** State what the probe arranges.
* The decision, in `DECISIONS.md`, with a real chance the answer is *no*.
  Moving `terrain`'s `NOISE_AMPLITUDE` or `SLOPE_SPAN`, or `SITE_HEADROOM`,
  changes every map, which re-opens M5's dike balance and the five-strategy
  table. If the measurement says the relief is there and the hearth pad's
  flatten-a-square removed it, the fix is small and local — take it. If it says
  the generator is flat, write the decision and defer, and say what it would
  cost.
* If changed: the strategy table before and after. **The map is the balance.**
* Either way, design §3.2 and the game end up agreeing: the promise becomes
  true, or the text goes.

**Done when** either every seed offers a citizen higher ground it can walk to,
or `DECISIONS.md` says in writing why the MVP's ground is flat and the design
text no longer promises otherwise.

## M12.10 — Whether growing is worth doing

Group C fault 10. Growth works now, and may still be pointless.

`COMING_OF_AGE` is two ages, so only a child born in age *one* ever works — and
age one is the age with no wood to spare for cottages and a day-four starvation
clock. City 0 reasoned its way to this during the run and it is good reasoning.

**Deliverables**

* The measurement, and it must **play, not feed** — the
  `a_household_in_a_fed_city_settles_without_being_force_fed` shape, not the
  `how_a_city_grows` shape. Over a three-age run: how many working adults does
  a city that spends on cottages and a nursery finish with, against one that
  does not? What it arranges, written down.
* The decision: bring the payoff forward — `COMING_OF_AGE` is a balance
  constant, so measured, with the table either side — or state that growth is a
  fourth-age feature and **stop asking for it in a three-age run.**
* Whichever it is, `tutorial::next_thing` and `AGENT-BRIEF.md` agree with the
  answer before M12.11 runs. Telling a player to do something that cannot pay
  back inside the run is worse than not mentioning it.

**Done when** a player who is told to grow can name what growing buys them
inside the run they are playing.

---

# Part V — the run

## M12.11 — The third run

Two agents, one browser each, the deployed build, neither able to see the
other's screen and neither allowed to read `crates/sim`. `AGENT-BRIEF.md`
current — M11 changed eight visible things and the brief had to change with
every one.

M11 showed that a **directed** run finds more than an open one. The questions:

1. **Does the wall get built**, now the ground can be read and the mark is
   visible? Nobody has ever tested a level-three wall in a played game.
2. **Does a city that grows end with more people than a city that walls?**
3. Did anything in M12.5 or M12.6 change a decision?
4. And the one no harness can ask itself: **two games, back to back, from the
   first game's lobby, without restarting a browser.**

**Deliverables**

* `PLAYTEST-M12.md` — the account, in the players' own words.
* `HANDOFF-M12.md`, and its artifact.
* `PROGRESS.md` and `DECISIONS.md` current.
* The referee's transcript, and its verdict.

**Done when** the referee says CLEAN, both accounts are written down, and the
run ended with a second game started out of the first one's lobby.

---

## The order, and why

| | | why here |
|---|---|---|
| M12.1 | Reproduce the lobby failure | the game cannot be played twice; and reproduce before fixing |
| M12.2 | A joiner is never left in silence | worth doing whatever the cause — it turns a silent hang into a sentence |
| M12.3 | Fix the lobby, and guard it | and add the end-to-end check that has never existed |
| M12.4 | The silent right-click | the most common gesture in the game, failing without a word |
| M12.5 | Deaths, and how deep the water is | the two readouts that cost people their lives |
| M12.6 | Small honesties | cheap, certain, and a lot of the confusion |
| M12.7 | Reproduce or dismiss | nothing stays a rumour |
| M12.8 | Water that drains, a mark you can see | first sim change; measured either side |
| M12.9 | Ground a player can climb | the biggest; may end in a written deferral |
| M12.10 | Whether growing is worth doing | decide before a run is spent asking |
| M12.11 | The third run | the only thing that can say whether any of it worked |

Roughly five sessions: Part I, Part II, M12.7 with M12.8, Part IV's other two,
then the run.

**If only three get done, they are M12.1 to M12.3.** Everything else in this
document assumes two people can get into a room.

## How to work here

Unchanged, and all of it earned. `make test` at every commit, no warnings.
`make browser-test` before pushing anything touching `gui` or `web/`. A `sim`
change ships with its test in the same commit; a `gui` change ships with its
browser check. Reproduce before fixing. When a test fails, find out whether the
test or the code is wrong before changing either — roughly half the failures
across these sessions were a test encoding an old world. `DECISIONS.md` gets a
paragraph for anything somebody else would have written differently.

And the one M11 added: **when you write a probe, write down what it arranges.**

## The single next action

`crates/net/tests/lockstep.rs`: stand up the loopback star, put the world past
`age() > 1`, run the game to finished, then `Lockstep::join` against the same
host and drive frames until something is decided. **Get it failing before
changing a line of `net`.** If it will not fail there, the fault is in the room
rather than the handshake, and `rejoin.py`'s three-tab machinery is where it
goes next.
