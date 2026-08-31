# FLOODLINE — M12 second-order effects

Every change made in M12, and **what else it changes**. `DECISIONS.md` says
why a choice was made; this says what that choice did to things nobody was
looking at.

It exists because the M11 run was lost to exactly this class of thing: M11.6
added deaths with causes and they were invisible, not because the feature was
wrong but because it shared a message slot nobody had thought about. The
feature was reviewed. The slot was not.

**How to read an entry.** *The change* is what was done. *Follows from* is what
it necessarily drags with it. *Watch* is what would show it going wrong, and
where. A change with no second-order effects gets an entry saying so — that is
a claim, and a claim can be wrong.

---

## M12.1 — the lobby fault, reproduced

**The change.** Three tests, no production code:
`a_host_that_finished_a_game_tells_a_new_joiner_something`,
`a_joiner_that_greeted_a_ghost_still_greets_the_host_when_it_arrives`,
`a_joiner_in_a_room_with_churn_is_still_told_about_the_silence`.

**What the reproduction found**, which is not what the handover guessed. The
handover's first suspect was a stale peer in a reused room, and it was right
about the room and wrong about the mechanism. The handshake against a finished
host is *fine* — it answers `this game is full`, which is an answer. The fault
is one line up from the handshake: **`greet` was a `bool`.** A joiner said
`Hello` to the first peer it met and to nobody else, ever. A non-host that
receives a `Hello` does nothing with it — the handler is guarded `if self.host`
and falls through to `_ => {}` — so nothing on the wire ever said *"I am not
the one you want"*.

**Follows from.**

* The build-hash suspect is ruled out and stays ruled out: the room name
  carries the hash, so two builds never meet at all.
* `rejoin.py` being lobby-only was never the gap it looked like. The gap was
  that **no test anywhere put a joiner in a room with more than one peer in
  it.** Every existing test is a star with one edge, which is the shape the bug
  hides behind. That is the gap M12.3 closes, and it is a different gap from
  the one the handover named.

**Watch.** `nearly_over` arranges *where a world starts* and nothing else — the
ending is reached through `age.rs`'s own roll-over. If a future change makes
`finished` reachable some other way, that test stops covering what it says.

---

## M12.2 — greet everybody; a clock nothing resets

**The change.** `greet: bool` → `greeted: BTreeSet<PeerId>`; `host_peer` set
only by `Welcome`; `unanswered` → `waiting_since`, counted off `greeted` and
never reset by `peer_left`; the silence message widened to cover a room with
nobody hosting in it.

**Follows from.**

1. **A joiner now sends one `Hello` per peer in the room.** In a two-person
   game that is one extra message in the worst case. In a room with stale tabs
   it is one each, once. `Hello` is small and the cost is bounded by peers, not
   by time — it is a set, so a peer already asked is never asked twice.
2. **A non-host still ignores `Hello` silently, deliberately.** Answering it
   with `Bye` would have been the honest wire — but `Bye` is what *ends* a
   joiner's run, so a bystander replying would end the game of a joiner that
   has done nothing wrong. Greeting everybody makes the silence harmless and
   `waiting_since` makes it audible. Written up in `DECISIONS.md`; a new
   message type would have changed the wire format and therefore the build
   hash, for a sentence.
3. **`peer_left` no longer ends a joiner's game unless it was welcomed *and*
   the departing peer was the one that welcomed it.** Before, `host_peer` was
   set by greeting, so the peer whose departure ended your game was whoever you
   happened to meet first. This is strictly narrower and strictly more correct.
4. **A peer that drops and reconnects is greeted again** — `peer_left` removes
   it from `greeted`. That is deliberate: from inside a room, a host on a fresh
   connection and a stranger are the same event.
5. **Two existing tests were encoding the old world** and were changed, not the
   code: "Hello is said once, however many peers turn up" *is* the fault. Said
   in the commit message so nobody re-derives it.

**Watch.** `SILENCE_FRAMES` is 500 frames, about eight seconds. It is now a
floor on how long a joiner can be silent, where before it was unreachable in a
churning room — so a *slow* `Welcome` on a bad connection could now produce a
warning that is wrong. The `Welcome` snapshot is 102 KB; if it grows toward the
150 KB budget, check that it still arrives inside eight seconds on a real link
before trusting the message. This is a real regression risk and it is the price
of the message existing at all.

---

## M12.3 — the guard, in a browser

**The change.** `rejoin.py` gains a room where two joiners meet each other
before the host arrives.

**Follows from.** The plan asked for "a run played to its end, then two peers
into a lobby again". That is **not** the check that was needed, and building it
would have cost eighteen minutes a run for a case `cargo` settles in under a
second. The check that was needed is a room with more than one peer in it, and
it takes twenty seconds. Recorded here because the handover named the wrong
gap in good faith and somebody will otherwise go and build the expensive one.

**Watch.** Two seats, so exactly one of the two joiners gets a city and the
other is told the game is full. The assertion is `welcomed(x) or welcomed(y)`
on purpose. If the seat count in the lobby ever defaults to three, this check
gets weaker without failing — it would pass with both welcomed and would no
longer be testing the ordering. Pin it if that default moves.

---

## M12.A — a farm feeds a city rather than a household

**The change.** `FARM_TICKS_PER_UNIT` 32 → 11. A farmer makes 109 units a day
instead of 37; a three-slot farm keeps 26 people instead of nine.

**Asked for directly.** This is the first change in M12 that is not a fault
being fixed, and the distinction matters: the measurement below says what the
change *did*, it is not what chose it.

**The table, either side.** `three_full_runs_of_each_strategy`, three seeds,
five scripted strategies, survivors across all seeds:

    play    before   after
    idle       0        0
    grow       8        9
    dike       8        7
    flee       4       12
    both       0        8

**Follows from.**

1. **The ceiling on doing more than one thing is what lifted.** `dike` and
   `grow` — the two single-verb strategies — did not move: 8 → 7 and 8 → 9,
   which is noise. `both` went from **nought survivors to eight**, and on seed
   31 from two ages with everybody dead to three ages with all eight standing.
   That is the change, and it is exactly the complaint three playtests made:
   walling cost the days that feeding needed.
2. **`flee` tripled, 4 → 12.** Getting uphill means leaving your farm, and
   leaving your farm used to be fatal on its own. It is now a thing a city can
   afford to do. Note this lands on the same verb M12.9 is about — a flat map
   is why fleeing rarely helps — so **M12.9's measurement must be taken after
   this, not before.**
3. **The flood is still what kills you.** `idle` dies on every seed, and two
   of the three seeds still kill every walling strategy outright. Food stopped
   being the only clock; it did not stop being a clock.
4. **`grow` on seed 1000003 got worse**, [8, 4, 0] → [8, 1, 0], and on seed
   4043362590 [8, 8, 1] → [7, 5, 2]. The scripts are fixed, so more food means
   the same script spends its days differently. Not understood, and it is
   noted rather than explained. **M12.10 measures growth and must not read
   these two cells as a growth finding.**
5. **Nothing in the suite noticed.** All 289 tests passed unchanged across a
   3× production change, because not one asked what a farm feeds. That is a
   test-coverage fault of the same family as `how_a_city_grows`: the number was
   only ever checked by playing it. `a_farm_feeds_a_founding_party_several_times_over`
   is the guard now.
6. **`FARM_BUFFER` is the real cap until a granary is standing.** A Hearth
   holds no food, deliberately — design §3.3 gives it no larder. So with
   nowhere on the map to put a farm's output, the buffer fills at 60 and the
   farmers stop: measured without draining, **a farm makes exactly 60 units a
   day at any value of this constant.** Tripling the rate therefore does
   nothing at all for a city that has not built a granary, and a great deal for
   one that has. That is a sharp new edge on an existing decision and no
   readout anywhere says a farm has stopped because its buffer is full.
   **Candidate work, and it is not currently in the plan.**

**Watch.** `three_full_runs_of_each_strategy` is now the *new* baseline. Any
measurement in M12.8, M12.9 or M12.10 taken against the pre-M12.A table is
reading a game that no longer exists.

---

## M12.B — the builder's hut

**The change.** `Kind::BuildersHut`, free, `Job::at` answers `Builder` for it,
and `find_work` lets go of the hut instead of walking to it.

**Follows from.**

1. **A hut's builders are no longer counted against the hut.** `find_work`
   clears `workplace` the first time it looks, so the slot accounting in
   `assign` sees nought held and a player can name a fifth, a sixth, a
   twentieth builder. That is deliberate — `slots_for` returns `usize::MAX` for
   a hut and says why — but it means **`job_slots()` for a hut is a display
   number that constrains nothing.** If a future roster wants to show "4 of 4",
   it has to count `job == Some(Builder)` and not the hut's workers.
2. **`BUILDER_SLOTS` still caps a site**, and that is now the *only* place it
   caps anything. Four builders on one site, any number of builders in a city.
3. **The panel did not move, by luck.** Eleven buttons needed six rows and so
   did twelve, because eleven left a gap at the end and the hut filled it. The
   next building costs a row — and M12.C was the next building, so this was
   luck that lasted one milestone.
4. **The button digit stopped being the index.** With ten buildings the tenth
   is `0`, and `format!("{}", i + 1)` would have drawn `10`. Now read off the
   `KeyCode`, so the label and the shortcut cannot disagree.
5. **An unassigned citizen is still a hauler that builds when idle**, and that
   is untouched on purpose — it is what keeps an unattended city from dying
   with the materials on the ground.

**Watch.** A builder whose site finishes has `workplace = None` and
`job = Some(Builder)` for ever. If a later change makes `find_work` clear the
job when there is nothing to build, the hut silently stops meaning anything.

---

## M12.C — the cookery, and a fifth good

**The change.** `Good::Meal`, `Kind::Cookery`, `Job::Cook`, and `produce` learns
to consume.

**Follows from, and this is the longest list in the document.**

1. **`Goods` is five wide and `Goods::of` still takes four.** Nothing *costs*
   meals, and all twenty-seven call sites meant zero. A fifth parameter would
   have made every one of them say so for no gain. `Goods::meal(n)` is the
   constructor. **Anyone adding a sixth good must decide this again.**
2. **The snapshot grew 60 bytes**, 102 419 → 102 479, against 47 KB of
   headroom. **The build hash changed**, so two machines on the old build and
   the new cannot meet — by design, and it means the deployed page and any open
   tab must both reload.
3. **`produce` has an input path now.** It was "worker-ticks become a good out
   of nothing" for every building in the game. A cookery breaks when its larder
   is empty *and keeps its accumulated `work`*, so cooks waiting on a hauler
   have not wasted the morning.
4. **`stores` grew a sibling, `takes`.** A cookery is somewhere to *put* food
   and not somewhere to *fetch* it — `stores_for` answers both questions, and a
   cookery in the fetch answer would have haulers carrying the same sack
   between the granary and the kitchen for the rest of the run.
5. **A new haul, `next_feed_run`,** placed between supply runs and collection,
   and only for a cookery with somebody in it. An unmanned kitchen is not worth
   a walk.
6. **Meals are eaten first, always.** A granary holding both spends the better
   one. This is the only ordering a player can predict, and it means **a city
   with a cookery drains its meals before its food** — so "how much food do I
   have" and "how long will it last" are different questions now, which is why
   `days_of_food` counts a meal at `MEAL_WORTH`.
7. **`nearest_food` had to learn about meals** or a city with a working cookery
   could starve in front of a full larder. That is one line, and it was one
   line away from being the worst bug in the game.
8. **The treasury row could not take a fifth figure.** Two layout facts,
   measured before anything was drawn: eleven buildings need a seventh row of
   buttons, which is 40 px the panel does not have — the trade offer sat 93 px
   above `VARIABLE_FLOOR` and a selected building's level/move row costs 48, so
   a seventh row at the old pitch left **five**. The overflow counter would then
   have quietly stopped drawing the trade row, which is the M10.6 fault wearing
   its other face: not drawn *over* the foot, but not drawn at all.
   `TOOL_PITCH` went 40 → 36 and the row now costs four pixels. Slack: 93 → 81.
9. **And the row itself is 16 point, not 18**, because five figures do not fit
   on 330 px. It hands its two pixels back so nothing below it moves, and the
   meals figure appears **only when the city has meals**. A row whose *content*
   varies is safe; a row that comes and goes is what has bitten this panel five
   times.
10. **`panel.py` stopped carrying 272 as a literal** and derives the rows under
    the build grid from the grid's height. `play.py` and `assign.py` keep their
    own literals as tripwires and are **expected to fail once** on the next
    browser run — that is them working.

**Watch — three things.**

* **The strategy table has not been re-measured since this landed.** M12.A's
  numbers are the baseline for M12.8–M12.10, and a cookery changes what a city
  can do with the same farmland. **Re-measure before Part IV.**
* A cookery drains the granary into meals. A city that builds one and then
  loses it to the flood has a larder of meals and no way to make more, which is
  fine, and a city that builds one *instead of* a second farm has fewer raw
  units in absolute terms. Nothing tells the player either thing.
* `COOK_TICKS_PER_UNIT` is 22 against a farm's 11 so a cook converts at the
  same worth-per-tick a farmer grows. If `FARM_TICKS_PER_UNIT` moves again,
  **this has to move with it** or the cookery becomes either free food or a
  waste of two people.

---

## M12.D — the people tab, and progress

**The change.** A third tab listing one chip per citizen, and a progress line
over anyone working on the map.

**Follows from.**

1. **No sim change and no snapshot cost**, and that was a constraint rather
   than an outcome. `draw::task_progress` derives everything from `work`,
   `progress`, `food` and `rest`. A progress field per citizen would have been
   two bytes each on the wire to say what the world already knows.
2. **A walker has no bar anywhere.** The world records where somebody is going,
   not where they set out from. This is a real gap in what can be shown and it
   is left as a gap rather than guessed at.
3. **The tab row is three tabs and the same height.** A taller row would have
   moved the entire build tab down and there are 81 px left.
4. **The chip list is capped by `VARIABLE_FLOOR`** and says "and N more". A
   city of eight fits with room to spare; a city of twenty does not, and
   **growth now has a visible ceiling in the panel** that nobody has played
   against. M12.10 may make cities that large.
5. **`ringed` is now shared between two tabs.** Hovering a person's chip rings
   them exactly as hovering a household rings its members. Both clear it on
   entry, so the two cannot fight.
6. **Clicking a chip replaces the selection** rather than adding to it. That is
   the simplest rule and it is not obviously the right one — a player building
   a work gang may want to add. Left as it is, deliberately, and named here.

---

## M12.4–M12.6 — the panel stops lying and stops going quiet

**The change.** A right-click with nobody chosen says so; `room_for` and
`beds_for` carry the reason a building said no; `job == None` reads *hauling*;
the amber line puts the water first.

**Follows from.**

1. **`sim` was innocent, and that was the finding.**
   `a_building_is_where_it_was_clicked` places every kind at every facing and
   finds it under the click every time. The forester at (75,97) was exactly
   where it was put. The plan named a footprint-offset suspect and it was
   wrong; the fault was a silent early return.
2. **`will_take` and `will_house` are now thin wrappers** over `room_for` and
   `beds_for`. Anything that wants the count still gets it; anything that wants
   the reason can have it. Callers that ignore the reason are unchanged.
3. **`RuleError::Full` is now only ever a real capacity problem.** Before, the
   panel said "no room" for four different states, so "no room" carried no
   information. It does now — which means any *new* code that answers `Full`
   for something that is not full has become a lie where before it was only
   noise.
4. **The amber line's ordering is now "what kills you soonest" for real**, and
   the water is above the bootstrap. A city with no granary and a flood on the
   map is told to get uphill. That is deliberate and it is arguable: it means a
   brand-new player on the impact day is never told what a granary is.
5. **Held citizens are counted apart from haulers** in the roster. That is a
   third category where there were two, and the households tab is the only
   place that shows it.

---

## M12.7 — three slots where there was one

**The change.** `notice` (a reply), `report` (news), `toll` (the dead,
accumulating). Plus `RuleError::StillRising`.

**Follows from.**

1. **All three are drawn over the map, not in the panel**, so none of it comes
   out of the 81 px of panel budget. That was the reason for putting them
   there and it is worth knowing before somebody moves them.
2. **Two faults from the M11.9 account closed with one change**, and one of
   them — the wall that vanished with no announcement — was never reproduced
   independently. If it recurs, the notice slot is no longer the explanation
   and something else is wrong.
3. **`StillRising` needs `level >= 2`**, because a dike *site* starts at level
   one. Anything that changes what level a fresh site has silently turns this
   message back into the wrong one.

---

## M12.8 — ground that drinks

**The change.** Per-cell saturation, material-dependent soak rates, an aquifer
that deletes water, `DAMP`, and a high-water mark in silt ochre.

**Follows from, and this is the one to read before touching the water.**

1. **The snapshot is 118 867 bytes against a 150 KB budget.** 47 KB of headroom
   became 31. This is the largest single thing the wire has ever been asked to
   carry and **it should be the last field measured in kilobytes.** Anything
   else per-cell has to be a bit, or it does not fit.
2. **`DAMP` is used in three places** — the soak floor, what the map draws, and
   what `wetness` names — and they must stay the same number. Split them and
   you get either a film the map paints and the ground will not take, or one
   the ground took and the map still paints.
3. **The dike balance is intact and it was nearly not.** Three of the four
   constants exist to protect it. `SOAK_CEILING` is the important one: without
   it, no wall broke at any level under any surge.
4. **`Water::step` takes ground *types* now, not just heights.** Every caller
   passes the real map except the automaton tests, which pass rock on purpose
   so they go on measuring the automaton.
5. **The soak phase is on `Water`, not `World::tick`**, because `step_water` is
   public and half the dike tests drive it without the world turning over.
6. **`accounted()` includes `held_by_ground()`** now. Any conservation check
   written against the old two-term version will fail, correctly.
7. **Water above `WADE_DEPTH` does not drain.** A deep pool with nowhere to run
   to stays a deep pool. That is the price of keeping a dike worth building and
   it is a real limitation, not an oversight.

---

## M12.9 — the high ground was always there

**The change.** A probe, and an `h` overlay. **No change to `map::terrain`.**

**Follows from.**

1. **The handover's finding was wrong and the plan inherited it.** "Every cell
   reads 16 to 25" was a claim from two runs. Ten seeds say the median climb
   within walking distance of a hearth is eleven terrain units against a surge
   twelve deep. **Two accounts agreeing is not a measurement**, and this is the
   second time in M12 that a confident finding from the run turned out to be
   the wrong diagnosis of a real complaint.
2. **`SITE_HEADROOM`, `NOISE_AMPLITUDE` and `SLOPE_SPAN` are untouched**, so
   M5's dike balance is not re-opened — which matters because M12.8 already
   leaned on it in the same session.
3. **The overlay is relative to your own hearth.** On a map with two cities at
   different heights the two players see different overlays for the same cell,
   which is correct and will look like a bug to anybody comparing screenshots.
4. **City 1's "you would need a ring" is still open.** It arrived bundled with
   the flat-ground claim and does not fall with it. On ground with real relief
   a wall across a saddle is a different purchase, and nobody has played a run
   that could see the saddle.

---

## M12.10 — growth, and three fixes that were reverted

**The change.** A probe, a `grow` script that actually grows, and the amber
line no longer asking for children. **No change to the growth mechanism.**

**Follows from.**

1. **The `grow` column of the strategy table has never measured growth**, in
   any milestone, and the numbers in every previous document that cite it are
   citing "a city with two spare cottages". They are not wrong about what they
   measured; they are wrong about what it was called.
2. **Four gates, and M12.A fixed one.** The binding one is that a household is
   *"the lowest two ids currently homed in this cottage"* — the sim houses
   people by itself, so a third resident silently orphans the pair.
3. **Three fixes were written and reverted** and are recorded in `DECISIONS.md`
   with the reason: each looked right and each produced byte-identical
   measurements. **A rule change that fixes nothing observable does not ship.**
4. **The nursery and the cottage still work.** Only the tutorial changed. A
   player who wants to try can; they are simply no longer told to.
5. **`tutorial::next_thing` is now shorter by two rungs**, so the ladder falls
   through to the dike sooner. That is a second, unmeasured effect of a change
   made for a different reason, and the next run is what will show it.
