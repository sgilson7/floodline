# FLOODLINE — the M11 run

M11.9. Two agents, one browser each, a full three-age run on the deployed
build, neither able to see the other's screen and neither allowed to read
`crates/sim`. Eighteen minutes rather than thirty-six, because M11.1 halved the
clock without touching the game.

**It was a directed playtest.** M10 asked "what does a player do?"; this asked
two narrower questions: *can a player grow a city above eight*, and *did any of
M11.2–M11.8 change a decision*.

The first question found a bug that had been there since M9 and that no probe
in the repo could see.

---

## What the referee saw

    16:08:08  city 0: 17 days in 17.5 min, 0 samples with no tick, 0 red, ended
    16:08:08  city 1: 17 days in 17.5 min, 0 samples with no tick, 0 red, ended
    16:08:08  expected about 17.5 days each over 17.5 min
    16:08:08  CLEAN

**No desync.** Day turns 57 to 63 seconds against a nominal 60. And the
referee's own arithmetic agrees with itself for the first time: the M10.6 run
reported 116 phantom stalls because a trade offer was drawn over the tick row,
and M11.2 put a floor under that.

## The result

**"The map stood."** Three ages survived. City 0: eight at its height, **none
left**. City 1: eight at its height, **six left, standing**.

Neither city ever exceeded eight souls. Nobody has, in three playtests.

---

## The finding: no city can grow, and the probe could not see it

Both players did everything the brief asked. Both built cottages and a nursery,
both kept 300 or more food in the granary, both formed households — city 1 had
two, *Everard and Richenda* and *Kentigern and Basilia* — and both watched them
read **"settling in" for four days and never bear a child**.

> Nothing anywhere told me what was still missing. — city 1

The cause. A household accumulates `together` only while **both** members are
at or above `CHILD_FOOD`, and needs `TICKS_PER_DAY` — 1 200 — *consecutive*
such ticks to settle. `CHILD_FOOD` was `FED_ENOUGH`, which is the exact level
at which a citizen **stops eating**. Food then decays a point a tick. So a
citizen is at that level for one tick of each cycle and below it for the rest,
and the counter reset almost every tick.

Measured, on an ordinary fed city with nobody force-feeding it:

    a citizen's food over four days: 0 to 999
    CHILD_FOOD is 900, and FED_ENOUGH 900
    the best any household managed: together = 99
    it needs 1200 consecutive ticks to settle

**Ninety-nine of twelve hundred.** Eight per cent, for ever.

`families::how_a_city_grows` reports healthy growth — 8 to 10 by day 6, 12 by
day 12 — and always has. It sets `c.food = NEED_FULL` every tick, so it has
only ever tested a city that cannot get hungry. The one probe pointed at this
mechanism was the reason nobody found it.

The bar has to sit *below the trough* of an ordinary cycle, because the cycle
is inherent: a citizen goes to eat at `HUNGRY`, stops at `FED_ENOUGH`, and dips
further while walking to the granary. Measured at `HUNGRY` a household still
stalls at 699 of 1 200. It is `NEED_FULL / 10` now, which means "neither of
them is failing to get fed" — and the gate still gates: fed cities grow to 10
and 12, unfed ones stay at eight or die.

`a_household_in_a_fed_city_settles_without_being_force_fed` is the guard, and
it plays rather than feeds.

---

## Did M11's readouts earn their place?

Both accounts, independently.

**Goods in transit — kept, both called it the best of them.**

> It was the only thing that told me my haulers were actually working. — city 0

> It is how I found the hauling bottleneck. Keep it. — city 1

**Water depth on hover — the most valuable, and in the wrong place.** City 0:
*"the only thing on the whole screen that distinguishes survivable water from
lethal water. It changed exactly one decision and by then everyone was dead."*
Both asked for it in the panel during a flood rather than under the cursor —
the warning "8 of your people are in the water" does not say *how deep*, and
wading versus out-of-your-depth is the difference between ignore and evacuate.

**Ground height — right idea, wrong map.** Every cell either player hovered
near home read 16 to 25. City 1: *"my whole reachable world is 16–18. On
terrain with relief this would be the best verb in the game. Here it only told
me I had no options."* That is a finding about the map generator, not the
readout.

**The high-water mark — did not work, for a reason worth having.** The water
from flood 1 had not drained by flood 2, so the map was blue for most of ages 2
and 3 and a faint blue tint on blue water was invisible. City 0: *"the one
readout specifically designed to let me site the wall was unreadable in the
window where I needed it."* City 1 saw it shade *almost the whole map*, which
told it there was nowhere to run — real information, but not *where*.

**Deaths with causes — right idea, badly under-reported.** City 0 lost eight
people and read "1 drowned" throughout; city 1 never saw a death message at
all, because its own click-refusal overwrote it in the same frame. There is one
message slot and deaths share it with "you clicked a full building".

**Wall strain — legible, and only one of them ever saw it.** City 1: `level 1
of 4, 21% strained`, and it could see the wall working on the map — bluer west
of the line, greener east.

**The persistent refusal — earned its place.** City 0: *"having it stay dimmed
meant I could read it one screenshot later. This is how I learned a click had
been answered rather than ignored."*

---

## The wall, a third time

City 1 built one — 27 cells, nine segments, about 270 stone — and gave the
clearest statement yet of what it costs:

> Stone was never the problem: I started with 720 and ended with 290. **Hands
> were.** With three haulers the dike moved 40 stone in a day. With all eight
> hauling it moved 370 stone in half a day. But the price is paid in the wrong
> currency: for a day and a half nobody farmed and nobody finished the
> cottages, which is very likely part of why the births never came.

And a second shape of the same problem:

> A line is the wrong shape for this terrain. The water arrives from every
> direction across ground that is uniformly 17 high; you would need a ring,
> roughly three times the cost.

Its wall held one flood at level 1 and had *almost entirely disappeared from
the map* by age 3, with no announcement — the break message added in M11.4 did
not reach it.

City 0 never built one, and says so plainly: it deferred the wall to read the
high-water mark after flood 1, flood 1 killed nobody, and flood 2 killed
everybody.

---

## What killed city 0, and why it is a design problem

> On day 6 I did a choose-all and right-clicked the cottage — trying to finally
> form that household — and marched everyone across the city into deep water.
> Eight souls to five to zero in about forty seconds.

It had been warned, in those words, and did it anyway because forming a
household was the objective it was behind on:

> The panel gives you an objective that requires a right-click on a building,
> and the flood turns that same right-click into a death sentence with no
> confirmation and no visible difference in the click.

---

## New faults, for whatever comes next

1. **Right-clicking the cell you built on does not always staff the building.**
   City 0 placed a forester at (75,97); `right-click-cell 75 97` did nothing at
   all — no refusal — and (76,98) worked instantly. It cost two game-days with
   a forester standing empty while the amber line said "nobody is cutting
   wood". **Not yet reproduced; the first thing to chase.**
2. **`back to hauling` leaves people reading `idle`.** The households roster
   added in M11.5 labels an unassigned citizen idle, and unassigned *is*
   hauling. Both players hit it; one re-pressed the button for two days.
3. **Deaths share one message slot with refusals**, so they are lost.
4. **The high-water mark needs a colour water is not**, and the water needs to
   drain between ages — or the mark is invisible exactly when it is wanted.
5. **"no beds left there"** on a cottage that is not built yet, and **"no room
   left there"** on a nursery, which is not a workplace at all.
6. **A wall that vanished was never announced**, though M11.4 added the message.
7. **Growth may be structurally pointless in a three-age run** even now it
   works: a child comes of age two ages after birth, so only an age-one child
   ever works, and age one has no wood to spare for cottages. City 0's
   reasoning, and worth a run to settle.
8. **No score screen for city 1** — its window returned to the lobby.
