# FLOODLINE — the rehearsal

Design step 7 is "playtest the flood until it is fun". This is the first time
anybody has played this game for a full age, and the first time two players
have been in the same world for more than a few minutes.

M10.5: two agents, one browser each, twelve minutes on the deployed build.
Neither could see the other's screen — each was given one debugging port and
never the other — and neither was allowed to read `crates/sim`. What they knew
is `packaging/browser/AGENT-BRIEF.md` and what the panel told them.

The full run to age three is M10.6. This is the rehearsal that says whether the
loop works, and it found enough that it was worth doing on its own.

---

## The setup

* Deployed build `9880d246fa15`, two Chromium instances, room `fl-m10-122141`.
* City 0 on the east bank at about (98,53), city 1 on the west at (38,34),
  the river between them.
* Both started with 8 souls, food 0, wood 200, stone 720, gold 0.

## What the referee saw

Fifteen minutes, a look every five seconds, 153 samples:

    10:44:47  both turned a day (1)      10:52:49  both turned a day (5)
    10:46:51  both turned a day (2)      10:54:47  both turned a day (6)
    10:48:50  both turned a day (3)      10:56:48  both turned a day (7)
    10:50:50  both turned a day (4)
    10:58:28  city 0: 7 days in 15.0 min, 0 samples with no tick, 0 red
    10:58:28  city 1: 7 days in 15.0 min, 0 samples with no tick, 0 red
    10:58:28  CLEAN

**The two worlds never parted and the clock never slipped.** Day turns came
124, 119, 120, 119, 118 and 121 seconds apart against a nominal 120. Neither
page ever drew no tick, and neither status row ever went red. Two browsers, two
agents clicking, a flood, and fifteen minutes: the harness holds, and thirty-six
minutes is not in doubt on this evidence.

---

## City 1's run, in its own account

**It starved to death on day 4 of age 1, and the water never reached it.**

It built a granary, then a farm, staffed the farm three-of-three by day 3, and
then watched `food` stay at 0 for three days while the amber line repeated
*"the granary is empty - give the farm a moment"*. Its city went to 0 souls
before the flood arrived.

Its own summary of why it could not save itself is the most useful sentence
either agent wrote:

> The amber line is described as "never wrong about what is missing", and it
> was technically correct and practically fatal: it named the mechanism ("the
> granary is empty") but never the clock.

What it wanted was one line — `food 0 - 8 mouths, 0 days left` — and, on the
farm, `+N food/day`. It could not tell whether it was a day too slow or whether
food was never being hauled at all, because **nothing in the panel says what a
citizen is doing.** It had five people not in the farm and never dared press
"back to hauling" because that would have emptied its only farm.

## The wall, which is what M5 parked for this

City 1's answer is **no, and it could not have known**:

> I sat 15 cells from the river against high ground and the flood stopped seven
> cells short of my buildings. 180 stone and several people-days of hauling
> would have bought me literally nothing.

> The dike is the only building whose payoff you cannot estimate before you buy
> it. A farm's value is obvious. A dike's value depends on how far the water
> comes, how hard it leans, and what level survives — none of which is visible
> or hinted anywhere. Right now the decision is a coin flip with a 180-stone
> stake.

It watched city 0 build a wall along its bank and lose five of eight souls
anyway, and could not tell from the map whether that wall broke, was overtopped
or was washed away.

**This is a sharper answer than the probes could give.** M5 measured *which*
dikes break and got the numbers into the target band. What it could not measure
is that a player has no way to estimate the payoff before paying, which makes
the decision a guess rather than a judgement — and that is a design problem
rather than a balance one.

## City 0's run, and the answer M5 was waiting for

City 0 built the wall. It came out of age one with **3 souls of 8**, and only
two of the five it lost had drowned.

> The dike cost me a day and a half of my only three free labourers, and that
> labour is *the same labour that feeds the city*. Building the wall is what
> caused the famine. Five of my eight deaths trace directly to the decision to
> build it, and none of them drowned.

Its own counterfactual:

> Had I spent day 4 and 5 on a third farm and kept three haulers moving grain,
> I would have gone into the flood with eight fed people and lost at most the
> two who drowned. I would then have had eight pairs of hands to build a
> *proper* wall before age 2's flood. Building the wall in age 1, with eight
> people and no food buffer, was a straightforward mistake — and the game let
> me walk into it while showing me a reassuring amber line.

**This is M5's parked question, answered.** M5 wrote: "on the other two seeds
building a wall still costs the city the run, because the labour has to come
from somewhere and those maps have no slack in age one. That is a question
about the food economy, not about dikes, and the instrument for answering it is
two people playing a run — which is M10." The instrument has now run, and it
names the mechanism precisely: **the hands that build a wall are the hands that
carry grain**, and nothing on screen connects those two facts. It is not that
the wall is too expensive in stone — city 0 spent 220 of 648 and never noticed
the cost. It is that the wall is paid for in *food*, invisibly.

Both cities also confirm the wall bought nothing they could see. City 0's
disappeared from the map without a word:

> There was no stress readout, no "holding", no "breached", no sound, no
> message — the wall silently consumed stone and people and then silently
> disappeared. If it absorbed anything at all, the game never told me.

## The two accounts disagree about one thing, and it matters

City 1 sat 15 cells from the river on high ground and the flood stopped seven
cells short of it. City 0 sat on the bank and lost most of its people. Neither
had any way to know which of those two situations it was in before choosing
whether to spend a day and a half of labour on a wall. The game asks its
central question — grain or wall — without telling either player the one fact
that decides the answer.

## What was fun

Both agents, independently, named the same three things.

**Reading the other city off the shared map.** City 1: "inferring a stranger's
whole strategy from a few pixels, with no chat, is the best thing in this game
by a distance." City 0: "seeing the other city's orange wall across the flooded
river was a proper 'someone else is out there' moment, and it did it without
ever showing me their screen."

**The omen ladder.** *all quiet* to *the elders are uneasy* to **THE WATER IS
HERE**. City 0: "reading that and then having to choose — grain or wall, with
the same three pairs of hands — was the single best moment of the run. A real
dilemma with real stakes."

**The amber line as onboarding.** City 0: "the best thing in this game... after
the first sixty seconds I never opened the manual again." Which makes what
follows worse rather than better.

## Where the amber line fails

It is the most trusted element on screen and it has two failure modes, both
fatal, both reported independently.

* **It names the mechanism and never the clock.** "the granary is empty — give
  the farm a moment" repeated for two days while both cities starved. City 1:
  "technically correct and practically fatal". Both wanted the same line:
  something like `food 0 — 8 mouths, N days left`, and on a farm `+N food/day`.
* **On day 4 it told city 0 to build a trading post**, with food at 1 and the
  flood two days out. The one element a player has learned to trust gave
  actively bad advice at the worst moment.

## What no panel would say

The single most requested number, by both, was **what a citizen is actually
doing**. City 0: "for two full days the one thing I needed to know was *is
grain piling up at the farm or is the farm producing nothing*, and the UI could
not tell me. That single missing number is what killed my city." Neither could
see whether people were working, hauling or standing still, and both had idle
hands they did not dare move.



> Watching the other city through the shared map. Their granary appearing, then
> a trading post, then that dike creeping along their bank — inferring a
> stranger's whole strategy from a few pixels, with no chat, is the best thing
> in this game by a distance.

The flood's scale startled it — "it is not a tidy line rising up the bank; it
swallows the map" — and the omen ladder, *all quiet* to *the elders are uneasy*
to **THE WATER IS HERE** in red, "is excellent, cheap, and does real work".

## What it could not read

Recorded as reported; each is checked separately before anything is changed.

1. **The first click after a build hotkey did nothing**, every time —
   **not the game, and not reproducible.** A probe placed a cottage on the
   first click 8 times out of 8, three different ways of clicking, in a clean
   game. This is the same thing as the next item.
2. **A refused placement says nothing at all** — **and this one was our
   fault.** A build tool shades the cell under the cursor green or red, and a
   refused click writes a red line saying why. Both are drawn on the *map*;
   `driver.py panel` cropped the panel and nothing else, and the brief told
   both agents to prefer `panel` because it is cheaper. They were structurally
   unable to see a single refusal. `panel` now stitches that line beneath the
   panel — a deliberate refusal reads **"something is already there"** — and
   the brief says to hover-and-`shot` before committing to doubtful ground.
3. **`hover-cell` names buildings but never terrain**, so it could not find
   rock for a quarry, could not find trees, and could not tell during the flood
   whether its own hearth was under water.
4. **The treasury dips while goods are in transit** — 200 to 20 to 150 after
   siting one 50-wood granary — and it burned a screenshot thinking it had
   built three granaries by accident.
5. **There is no death state.** At 0 souls the amber line simply went blank and
   the game carried on saying `playing`. It found out by noticing a grey `0` in
   the roster.
6. **Escape did not clear a selection.**
7. **At this size the map is unreadable**: buildings are 14-pixel letters and
   people are 3-pixel dots that blur together. It could never count its own
   citizens.

---

## What the rehearsal changed before the run

Three of the findings were the harness rather than the game, and all three
would have wasted the thirty-six minutes:

* **`panel` cropped away the refusal line**, so neither player could see why a
  click was refused. Fixed; the line is stitched under the panel now.
* **The brief did not mention the placement ghost**, which is the game telling
  you in advance whether a cell will take a building. Added.
* The first-click complaint was neither of those and does not reproduce; it is
  recorded here rather than fixed, because there is nothing yet to fix.

Everything else in this document is about the game, and none of it is changed
here. M10.8 is where the run's findings are answered, and the run has not
happened yet.

## What still has to be true for M10.6

The referee says the harness will hold. The rehearsal says something harder:
**both cities died or nearly died in age one, and neither death was the water.**
City 1 starved with the farm staffed; city 0 starved because it built the wall.
A run to age three needs at least one city to solve food in the first four
days, and neither player managed it on its first attempt with the panel as it
stands.

That is not a reason to change the game before the run — it is the finding, and
M10.6 will say whether it is a first-time-player problem or a design one.
