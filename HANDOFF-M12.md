# FLOODLINE — handing over after the M12 run

`HANDOFF.md` is the guide to the *codebase* and is still true of it.
`HANDOFF-M10.md` is the guide to how the two-agent playtest works and is still
true of that. **This is the guide to what M12 did and what the M12.11 run found
about it.** Read `CLAUDE.md`, then `HANDOFF.md`, then this.

---

## Where it is

298 cargo tests, 63 browser checks across 16 scripts, no warnings.
`make test` ~35s, `make browser-test` ~14 minutes.

| | |
|---|---|
| The plan M12 executed | `PLAN-M12.md` |
| **Every change, and what it dragged with it** | **`SECOND-ORDER-M12.md`** — this is new, and it is where changes were listed rather than approved |
| The account of the run | **`PLAYTEST-M12.md`** — read this before anything else here |
| Every decision, in date order | `DECISIONS.md` — the last dozen entries are M12 |

Fifteen milestones, fourteen done plus the run. The lobby is fixed, four
requested features are in, and the flood model has changed for the first time
since M5.

---

## The one sentence

> **Two accounts agreeing is not a measurement.**

M11 paid for *a probe that arranges the condition it is measuring can only
confirm itself*. M12 found the other half of it, three times, and it is the
most useful thing in this document.

1. **The silent right-click** was diagnosed in the M11.9 handover as a
   footprint offset — the ghost drawn in one place, the building landing in
   another. It is not. `a_building_is_where_it_was_clicked` places every kind
   at every facing and finds it under the click every time. The fault was a
   right-click with nobody chosen returning without a word.
2. **The flat ground** was diagnosed as a generator fault, and the handover
   said so in as many words: *"a finding about `map::terrain` and
   `SITE_HEADROOM`, not about the readout"*. It is not. Measured over ten
   seeds, the climb available within a citizen's walking radius of a hearth is
   a **median of eleven terrain units** against a surge twelve deep. The ground
   is not flat. It was invisible.
3. **The `grow` column of the strategy table has never measured growth**, in
   any milestone. Its plan had no nursery and nothing ever paired anybody into
   a cottage. Every number ever published for it is a number about a city with
   two spare cottages.

Every one of those complaints was **real**, and every diagnosis was confident,
plausible, agreed on by two independent players, written into a handover and
then into a plan. **Before you act on a finding from a run, ask what would
measure it.**

---

## First priority: nobody has finished a wall, in four runs

M10 asked whether the wall is worth building and said *"the wall is not
underpowered, it is unreadable"*. M11 spent nine milestones making it legible.
M12.11 played it again and found that **what both players were reading was a
construction site nobody was working on.**

City 0 put 440 stone into fifteen dike segments across two ages. Not one ever
reached `level 1 of 4`. It had a builder's hut and never assigned anybody to
it, because nothing told it to. At the end it was informed that a wall it had
never owned had given way.

City 1, independently, put ~300 stone and a day of its whole city into a wall
that read `dike: being built` a full age later.

**The immediate causes are fixed** — a site now says its percentage and
"nobody is building it", and the hut says how many builders you have named —
but the question the last three milestones exist to answer is *still open*, and
it is now open for a different reason than anybody thought. **Nobody has ever
seen a finished wall meet a flood in a played game.** That is what the next run
has to produce.

Start by asking why an unassigned citizen, which builds when there is nothing
to haul, never got to those sites. There was 440 stone delivered and idle
hands; something in `take_a_site` or in what counts as "nothing to haul" is
not doing what its comment says.

---

## The rest of what the run found

### A — mine, from this session, and fixed

All five were introduced by M12 and are fixed in `7aefb1c`. They are listed
because the *shape* of the first one recurs.

1. **Neither player saw a single death.** Thirteen deaths between them, both
   watching for the line, both had read the brief. The line was drawn.
   `driver.py panel` crops from sixty above the foot; M12.5 put the toll at
   ninety-six and M12.7 put the report at a hundred and forty. **The new slots
   were outside the players' own eyes** — which is the fault `driver.py`'s
   docstring warns about, in the same words, about the same crop. **A slot
   added above that line has to move that line.**
2. `builders hut: 0 of 4294967295 working`, on both screens.
3. A site nobody is building said only `being built`.
4. **The `h` overlay was invisible and it killed people.** City 0 measured it:
   36 of 255 on one channel, green on green. City 1 trusted it and evacuated
   twice to ground *lower* than its own hearth; five drowned. It is violet now.
5. **The larder line said "more farmers" when the shortage was haulers.** A
   farm fills a small buffer and stops, so a city that employs everybody
   starves beside two working farms. Both players hit it.

### B — reported, not reproduced. Reproduce before touching

6. **`box-select` over four visible people chose nobody**, twice, for city 0.
   The people were `held` — standing where they had been sent — which may or
   may not be the reason and is the first thing to check.
7. **`back to hauling` with nobody chosen writes nothing anywhere.** The
   right-click path answers now; the button does not.
8. **A granary being moved reads `food 0`.** City 1 believed it had destroyed
   369 food and its last three citizens. Everywhere else the panel writes
   `wood 40+130` for goods in hand; a building in transit should too.
9. **Once a city is dead, hovering returns nothing at all** — at the moment a
   player most wants to ask how deep it got.

### C — design questions. Decide, do not patch

10. **The high-water mark is a lagging indicator and nothing says so.** Each
    flood goes higher than the last, so the only tool for planning against the
    next one records where the previous one stopped. City 0 parked its last two
    citizens on rock unmarked after *both* previous floods and the third covered
    it. *"I used the game's own evidence, correctly, and it killed my last two
    people."*
11. **The only ground that survives a flood is ground you cannot build on.**
    Rock at 30–35 was dry through two floods; grass tops out around 29. So a
    city must live in the floodplain and evacuate every age, forever. That is
    coherent, and nothing says it — `not on that ground` does not say *that is
    rock*.
12. **The food clock is a two-minute tutorial wearing the costume of a
    threat.** M12.A did what it was asked: food stopped being the only clock.
    City 0: *"from that moment to the end of the run — sixteen minutes — food
    was never again a consideration. I died with 349 in the granary."* Whether
    the day-one panic should still shout is now a question.
13. **The dike cost tooltip is drawn on the map, not the panel**, so the one
    number the wall decision turns on is invisible to the cheapest verb. And
    `0.1 days of one pair of hands` counts the building and not the hauling,
    which is the part that costs a city its people. City 1: *"that number is a
    lie of omission, and it's the reason I committed."*

---

## Things that will bite you

The lists in `HANDOFF.md`, `HANDOFF-M10.md` and `HANDOFF-M11.md` all still
hold. These are new.

* **The snapshot is 118 867 bytes against design §8's 150 KB.** M12.8's
  per-cell saturation cost 16 KB and M12.C's fifth good cost 60 bytes, so 47 KB
  of headroom is now 31. **This should be the last field on the wire measured
  in kilobytes.** Anything else per-cell has to be a bit, as the high-water
  mark is.
* **`driver.py panel` is the players' eyes and its crop is a literal.** See
  fault 1. It reaches 150 above the foot now, which covers all three message
  slots and nothing more.
* **`balance::DAMP` is used in three places** — the soak floor, what the map
  draws as water, and what `wetness` names — and they must stay one number.
  Split them and you get a film the map paints that the ground will not take,
  or one the ground took that the map still paints.
* **Three of M12.8's four water constants exist to protect M5's dike balance.**
  `SOAK_CEILING` is the one that matters: without it, *no wall broke at any
  level under any surge*. `dike_pressure_on_flat_ground` is the check and the
  table is in `SOAK_EVERY`'s doc comment.
* **`Water::step` takes ground types now**, and the automaton tests pass rock
  on purpose so they go on measuring the automaton and not the soil.
* **The soak phase lives on `Water`, not `World::tick`**, because `step_water`
  is public and half the dike tests drive it without the world turning over.
* **`Goods::of` still takes four arguments and `Goods` has five fields.**
  Nothing costs meals. Anyone adding a sixth good has to decide this again.
* **The panel has 81 pixels of slack** between the trade offer and
  `VARIABLE_FLOOR`. M12.C's eleventh building needed a seventh row of buttons,
  which is forty of them; `TOOL_PITCH` went 40 → 36 so it cost four. The next
  building costs a row.
* **`panel.py` derives the rows under the build grid from the grid's height**
  rather than carrying 272 as a literal. `play.py` and `assign.py` still keep
  their own literals as tripwires, and both survived M12's layout changes.
* **A hut's builders are not counted against the hut.** `find_work` clears
  `workplace` the first time it looks, so `job_slots()` for a hut is a display
  number that constrains nothing. Count `job == Some(Builder)`.

## The instruments

* `make test` — 298 tests, hermetic, no window.
* `make browser-test` — 63 checks across 16 scripts in a real browser.
* **The probes are all `#[ignore]`**; run with `--ignored --nocapture`. New in
  M12: `map::probe::how_much_higher_a_citizen_can_get` (the relief within reach
  of a hearth, which is the probe that contradicted the run) and
  `playtest::whether_growing_pays_inside_three_ages` (which found that the
  `grow` strategy has never grown).
* `three_full_runs_of_each_strategy` is still the table to check either side of
  any balance change, and **its `grow` column means something different now
  than in every previous document.**
* The two-agent harness works. `table.py`, `driver.py`, `referee.py`,
  `AGENT-BRIEF.md`. The brief is current for everything M12 changed that a
  player can see, which is a lot.

## The single next action

**Find out why 440 stone of delivered dike sites never got built.** An
unassigned citizen builds when there is nothing to haul; both players had idle
hands and full sites for two ages. Either that rule is not doing what its
comment says, or "nothing to haul" is never true in a real city — and in either
case it is the reason four playtests have asked whether a wall is worth
building without anybody ever owning one.

Then the run again, with one question: **does a finished wall change the
outcome of a flood?** Nobody knows. Nobody has seen one.
