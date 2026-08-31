# FLOODLINE — the plan for M11: make the world legible

M10 played the game to the end twice and wrote `PLAYTEST-M10.md`. This is what
that account asks for, as milestones.

The plan of record for M10 is `PLAN-M10.md`; its M10.8 was a placeholder for
"what the run demands", written before anybody knew what that would be. **It is
this.** M10.8 is closed and replaced by the nine steps below.

This document is also an artifact, if a link is easier to hand on than a file:
**<https://claude.ai/code/artifact/73e39edb-7920-4b0e-953f-25732dc49111>**

---

## The one sentence this plan comes from

> The wall is not underpowered. It is unreadable.

Nothing the run found asks for a balance change. M5 measured which dikes break
and got the numbers where it wanted them, and that work stands untouched. What
two played runs found is that **the decision those numbers balance is one the
player is asked to make blind** — and that almost every other complaint in the
account is the same fault wearing a different hat: the game knows something and
will not say it.

So the ordering principle here is not "biggest feature first". It is:

1. what stops the panel telling the truth about itself,
2. what the players said they would have traded anything for,
3. what cost them people,
4. what cost them time,
5. what is merely wrong.

## What is already done, so nobody redoes it

* **The food clock.** The amber line names what a city costs to keep, and a
  working building says what is standing on it. Fixed between the rehearsal and
  the run; neither city starved in the run.
* **The refusal line reaching the player.** `driver.py panel` cropped it away.
  That was the harness, not the game.
* **`peers at` saying something in a browser**, and the panel's variable row no
  longer moving three fixed rows.

## What is *not* here

Anything numeric. If a milestone below tempts somebody into changing a balance
constant, that is a sign the milestone has been misread: every one of these is
about saying a number, not choosing one. The house rule stands — measure it,
paste the table into the doc comment, and do it in its own commit.

---

## M11.1 — The clock — **done**

First, because it costs nothing and it halves the wall-clock price of every
test that comes after it, including M11.9's.

A run is thirty-six minutes and both accounts read like people who had been at
it a while. Design §11 has suspected since phase 1 that an age is too long.
`TICKS_PER_SECOND` is the knob, and **it is a pure wall-clock knob** — it
appears in no rule in `sim`, only in `gui`'s `Clock`, two `net` timeouts and
three balance constants.

**Two of those three are balance written in seconds, and have to be pinned
first** or doubling the rate quietly changes the game:

```rust
pub const SURGE_TICKS: u32 = 300;   // was 30 * TICKS_PER_SECOND
pub const DROWN_TICKS: u32 = 50;    // was  5 * TICKS_PER_SECOND
```

Left as they are, doubling the rate makes the surge pour for six hundred ticks
instead of three hundred — twice the water — and doubles how long a citizen
takes to drown *relative to the day*. The other three are genuinely wall-clock
and should keep scaling: `DROP_AFTER_TICKS` stays thirty real seconds,
`WAIT_WARN_TICKS` five, `PING_LIFETIME` three.

**Measured, not assumed.** With the two pinned and the rate at 20, the
five-strategy table is identical to the baseline:

```text
                      survivors        tallest wall by flood 1
  10 ticks/s   idle 0 grow 8 dike 8 flee 4 both 0     60 stone
  20 ticks/s   idle 0 grow 8 dike 8 flee 4 both 0     60 stone
```

Same game, half the clock, and everything on screen moves twice as fast because
there are twice as many ticks in a second.

**Deliverables**
* `SURGE_TICKS` and `DROWN_TICKS` pinned in ticks, each with a doc comment
  saying why it is no longer written in seconds.
* `TICKS_PER_SECOND` at 20, and a `sim` test that the two pinned constants do
  not move when it changes.
* Re-run `three_full_runs_of_each_strategy` and paste the table above into
  `TICKS_PER_SECOND`'s doc comment, in the house style.
* A note in `DECISIONS.md`: a run is eighteen minutes now, which is design
  §11's worry answered rather than deferred.
* **`AGENT-BRIEF.md` says "a day is two minutes of real time. A whole run is
  thirty-six minutes."** That line moves with the clock, and so does the
  polling cadence written beside it — an agent told to look every twenty-five
  seconds is looking twice as rarely once a day is half as long.

**Not** `WALK_SPEED`. Doubling that is the obvious way to make people move
faster and it is the wrong one: it is safe at exactly 128 — a road doubles it
and 256 is one cell a tick, which is the point at which a citizen would step
over a wall without the passability check seeing it — but it is not
balance-neutral. Measured, it takes the tallest wall a city can raise before
the first flood from 60 stone to 540, because haulers carry stone nine times
better, and `flee` drops from four survivors to one. That would undo M5.

**Done when** a full run is eighteen minutes and the strategy table is
unchanged.

**Done.** The table did not move. Two things turned up that this plan had not
listed: `Clock::MOST_PER_FRAME` is counted in ticks but caps *wall clock*, so
it doubled to 16 to keep the 1.25-frames-a-second floor the "two browsers"
decision rests on; and `referee.py` computed expected day-turns as
`minutes / 2`, which would have called every future run late — it has a single
`DAY_SECONDS` now. Everything else the change falsified was swept: the profile
probe's tick budget, `TICKS_PER_DAY`'s summary line, the brief's cadence, and
five harness files promising thirty-six minutes.

---

## M11.2 — A panel that tells the truth

The instrument first, because M10's own referee was fooled by it and because
half of this is self-inflicted.

**Deliverables**
* **The foot of the panel is never overdrawn.** A pending trade offer and the
  level/move row grow past `LOGICAL_H - 74` and cover `tick`, `peers at` and
  `build`/`seed` — the three rows a player is told to read when something is
  wrong. Decide deliberately between: drawing the foot last, giving the
  variable stack a hard ceiling, or admitting the panel is out of room. Write
  the choice down; the panel has moved six times.
* **`panel_rows.py` grows a second case**: with an offer pending *and* a
  building selected, the foot rows are still legible. Verify it fails on
  today's build before keeping it.
* **`day 7 of 6`.** `day_of_age` is `(tick - age_start_tick) / TICKS_PER_DAY + 1`
  and the world finishes on the last tick of the last age, so the age never
  rolls over. A test at the final tick.
* **A city that has died says so.** At 0 souls the amber line goes blank and the
  status stays `playing`; a player finds out by noticing a grey zero. Both
  rehearsal agents were confused by this.
* **A refusal that outlives a glance.** `NOTICE_SECONDS` is 4.5, so a player who
  clicks and looks away never learns why nothing happened. Either hold the last
  refusal until it is superseded, or put it somewhere that persists.
* **The joiner is told when the host desyncs.** Recorded in `DECISIONS.md`
  during M10.1 and deliberately left: today the host shows `DESYNC` and stops,
  and the joiner freezes on `playing` for ever with no explanation.

**Done when** a player watching only the panel can tell, at any moment, what
tick they are on, whether the peers agree, whether their last click was
refused, and whether their city is alive — and a browser check says so.

---

## M11.3 — Ground you can read

**The thing both players asked for above everything else**, from opposite banks
and without conferring.

> I wanted the game to draw last flood's high-water line on the map. That one
> feature would have turned the whole run from guessing into planning.

> That is reading the renderer, not playing the game.

City 0 planned its entire third age by screenshotting the second flood at its
peak and noting which pixels stayed green.

**Deliverables**
* **Hovering ground says what it is and how high it stands.** `Map::height`
  already exists, 0..=255 per cell, with `height_at`. This is a display change
  and nothing more — the hover row currently answers only for buildings, which
  is why neither player could find rock for a quarry or trees for a forester.
* **The map remembers the last flood's reach**, and draws it. `Water::depth` is
  already a `Vec<u16>`; the mark is its running maximum, reset when an age
  turns.
* **Price it before choosing the width.** `Water` is postcard-encoded into the
  `Welcome` snapshot, which design §8 budgets at 50–150 KB, and at 16 384 cells
  `depth`, `flow_x` and `flow_y` already cost about 96 KB between them. A `u16`
  mark adds 32 KB; a `u8` band adds 16 KB; one bit a cell adds 2 KB and answers
  the question both players actually asked, which was *where did it reach*, not
  *how deep was it*. Measure a `Welcome` before and after and put the number in
  the doc comment.
* **A determinism test.** The mark is new state in the checksum: two peers must
  agree on it through a flood, and `tests/determinism.rs` is where that is said.

**Done when** a player can point at a cell and be told what it is, how high it
stands, and whether the water reached it last time — and choosing a dike line
is a judgement rather than a guess.

---

## M11.4 — A wall you can read

The dike is the game's central purchase and the only building whose value
cannot be estimated before buying it.

**Deliverables**
* **A raise that works must look like it worked.** `raise_dike` adds a level and
  returns the segment to a site, so the hover row stops saying `level 1 of 4`
  and starts saying `being built` — *strictly less* than before the click. City
  1 played an entire run believing the interaction was broken, which is why its
  verdict is only ever "a level-one wall is not worth building". Say the level
  on a site: `dike: level 2, being built`.
* **A wall under strain says so.** `Building::stress` exists and is already
  drawn as a darkening; it is not legible. The hover row should say how hard
  the water is leaning and how close the segment is to going.
* **A wall that breaks says so.** Both players discovered it by noticing the
  wall was no longer drawn.
* **The cost of a wall, before it is bought.** The dike tool already shows a
  running cost in stone under the cursor. The run's finding is that stone was
  never the scarce thing — the labour was. Consider showing what a run will
  cost in *builder-time* beside its price in stone.

**Done when** a player can tell a holding wall from a failing one, and a raised
one from an ignored click, without taking a screenshot — and somebody can
finally play a run with a level-three wall, which nobody ever has.

---

## M11.5 — Sending some of the people

Named by both players, independently, as the worst part of the game. City 0
spent about a third of its entire run on a workaround.

**Deliverables**
* **A way to send fewer than everybody.** Right-clicking a building with the
  city chosen takes as many as will fit — including people already working
  somewhere else — so filling a second farm empties the first, silently. A
  count control on a selected building, or "send N of the chosen", removes the
  whole problem.
* **A roster.** There is no way to ask what the city is doing without hovering
  every building in turn. The households tab is the shape of the answer and
  already exists.
* **Escape clears a selection.** It does not, and city 1 resorted to
  box-selecting empty ground far away.
* **The first right-click after a build tool gives the order.** Today it only
  puts the tool down, which is documented behaviour and cost city 0 a whole
  assignment cycle. Decide whether cancelling the tool and issuing the order
  can be the same gesture; if not, say which it was.

**Done when** putting three people on a second farm does not empty the first,
and a player can see where everybody is without hovering the map.

---

## M11.6 — What the flood did

**Deliverables**
* **Who died, and of what.** The soul count drops and nothing else is said.
  Neither player could tell drowning from starving during a flood — which is
  exactly when the difference decides what to do next, and city 0's blind
  evacuation may have marched people into deep water.
* **Hauling into a flood is fatal and unremarked.** City 1 gave a routine "back
  to hauling" order on day five and came back to a city less than half its
  size: the order sent its people into the floodplain. Either warn, or let
  people flee rising water on their own, or both — but design §3.2 says "get
  uphill" is the one order that matters, and it should not be undone by a
  routine one.

**Done when** a player can say, after a flood, who they lost and where — and a
routine order given on the eve of a flood does not silently kill people.

---

## M11.7 — Small honesties

Each of these is one sentence the game already knows and does not say. Cheap,
and together they account for a lot of the confusion in both accounts.

**Deliverables**
* **Reserved goods look like spent goods.** Placing one 50-wood granary took
  wood from 200 to 40 and back to 150; both players thought they had been
  overcharged. Distinguish what is committed from what is free.
* **A stalled site says why.** City 1's first dike sat on "being built" for two
  entire ages without receiving one stone, with no explanation. "Nobody can
  reach this" or "waiting on a hauler" is the missing line.
* **The amber line says *where*.** It told city 1 to build a quarry, "it needs
  rock beside it", for two ages — with no rock within about 35 cells and no way
  to ask the map where any was.
* **The trade dialog is visible.** It draws over the map, so a player watching
  the panel sees a button that appears to do nothing; city 0 had it open unseen
  through several actions and city 1 had a click fall through it onto the map.
* **An offer goes stale.** "20 food for your 20 stone" sat in city 0's panel
  through the whole of age 3 while it had no stone, with no way to answer or
  dismiss it from that line.

**Done when** none of the five produces a question in the next playtest.

---

## M11.8 — Legible at the fit

Last, because it is the only group nobody died of.

**Deliverables**
* **A city is not the colour of water.** During the flood city 0's buildings,
  its dike and the water were all blue: "my settlement was five blue squares on
  a blue field", while the orange city stayed legible from across the map.
  Player 0's colour is the problem, on a map that is largely water twice a run.
* **People and buildings at the default zoom.** Buildings are 14-pixel letters
  and citizens 3-pixel dots that blur into one another; neither player could
  count their own people.
* **Bring the households forward.** *"Pagan and Oswin — settling in"* was, for
  city 1, "the only moment the city stopped being counters and became people",
  and it is behind a tab it opened out of curiosity.

**Done when** a player can find their own city on a flooded map at a glance.


---

## M11.9 — The second run, and a city that grows

M10's two play sessions produced more usable design feedback than every probe
in the repo put together, so M11 ends the same way it began: two agents, one
browser each, a full run on the deployed build, and a written account.

**With one thing asked of them that nobody has ever done.** Both M10 runs ended
with cities *smaller* than they started — eight down to two, and eight down to
three. City 0 skipped cottages and a nursery entirely, reasoning that "with
food at 1 and the flood two days out, growth was not the problem". So **M9 —
families, households, children, the nursery, the largest addition to `sim`
since the MVP — has never been played on purpose by anybody**, and no city has
ever risen above the eight it was founded with.

**Deliverables**
* **`AGENT-BRIEF.md` gains a growth objective**: both players are asked to get
  their city *above* eight souls, and to say what it cost them. In the game's
  own words and nothing more — two adults sharing a fed cottage become a
  household, a fed household with a nursery place and a spare bed has a child,
  and a child works two ages later.
* **The account asks four new questions**: how many souls at its height;
  whether growing was worth it against the flood that followed; whether a child
  born in age one ever paid its way by age three; and whether the households
  tab was worth opening.
* **And whether M11 worked.** Did the high-water mark change where the wall
  went? Did anyone raise a dike past level one? Could they tell who drowned?
  Each of M11.2–M11.8 either changed a decision or it did not, and the account
  is where that is settled.
* The harness needs nothing new: `table.py`, `driver.py`, `referee.py` and
  `panel.py` all still stand, and at 20 ticks a second the run is eighteen
  minutes rather than thirty-six.

**This is a directed playtest and that is a change.** M10's runs asked "what
does a player do?"; this one asks "can a player do this?" It measures something
narrower on purpose, because the alternative is shipping a family system nobody
has ever used. Keep the open-ended questions in the brief as well — the best
findings in M10 came from things neither agent was asked about.

**Done when** two agents have finished a run having tried to grow, and the
account says whether a city that grows survives better than a city that walls.

---

## The order, and why

| | | why here |
|---|---|---|
| M11.1 | The clock | **done** — free, measured neutral, and it halves the wall-clock cost of every test after it |
| M11.2 | A panel that tells the truth | the instrument everything else is judged with, and half of it is our doing |
| M11.3 | Ground you can read | what both players asked for above everything else |
| M11.4 | A wall you can read | the game's central purchase, still never tested above level one |
| M11.5 | Sending some of the people | the worst part of the game by both accounts |
| M11.6 | What the flood did | the deaths nobody could explain |
| M11.7 | Small honesties | cheap, and a lot of the confusion |
| M11.8 | Legible at the fit | nobody died of it |
| M11.9 | The second run | the only thing that can say whether any of it worked |

M11.1 and M11.2 are the two that change what a run *is*. If only two get done,
those are the two — and M11.2 is the one that turns the flood from something
that happens to you into something you can plan against, which is design step
7's whole question.

## How to work here

Unchanged. `make test` at every commit, `make browser-test` before pushing
anything touching `gui` or `web/`. A `sim` change ships with its test in the
same commit; a `gui` change ships with its browser check. Reproduce before
fixing. And when one of these turns out to be a test encoding an old world
rather than a bug, say so in the comment — that has been worth more than the
fix about half the time here.

## The single next action

M11.1's first deliverable: decide how the foot of the panel stops being
overdrawn, write the decision down, and extend `panel_rows.py` to a case with
an offer pending and a building selected. Get that failing on today's build
before changing a line of layout.
