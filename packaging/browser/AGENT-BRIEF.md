# You are playing FLOODLINE

Everything below is what a player could know: the controls, the goods, the
buildings and the deadline. It is the first-run card and the manual, written
down.

**You are told nothing from `crates/sim`.** Do not read the source, the balance
constants, the tests or the probes, and do not ask anyone who has. If you know
which dikes break before you build one, the run measures your reading of
`balance.rs` and not the game — and the one question M10 exists to answer is
whether this is *fun*, which nobody can answer from the answer key.

**You may not look at the other player's screen.** You are given one port. The
other one exists and is not yours.

**Cities are told apart by colour on the map.** City 0 is yellow, city 1 is
magenta. Both are legible against water, which the flood makes a lot of.

---

## The game

You are one of two cities on a river. Both banks, one city each, and there is
one ford — water shallow enough to wade, slowly. It is the only crossing until
somebody builds a bridge, and **it closes when the water comes**.

There are three ages. Each age is six days, and a day is a minute. **On day six
the river floods**, and the panel warns you the day before. Survive three of
them.

Your people starve on day four if they cannot eat, so the first flood is not
the first thing that can kill you.

## Your hands

    driver.py <your port> <verb> [args]

    panel                     read your side panel - do this most often
    shot                      the whole window, map included
    rows                      the status line and the three rows at the foot
    key <Name>                Digit0..Digit9, KeyC, KeyH, KeyR, KeyP, KeyM, Escape
    button <name>             a panel button by name
    click-cell <x> <y>        the map, 0..127 both ways
    right-click-cell <x> <y>  give an order, or put the current tool down
    hover-cell <x> <y>        park the cursor: the panel says what is under it
    box-select <x0> <y0> <x1> <y1>    choose the people inside a rectangle
    drag-cells <x0> <y0> <x1> <y1>    draw a wall
    frame                     the whole map again
    wait <seconds>            let the world run

Button names: `cottage farm granary forester quarry stockpile dike post
nursery hut cookery road point choose-all back-to-hauling trade tab-build
tab-households tab-people person-0 .. person-11`.

**Do not zoom or pan.** Every `*-cell` verb assumes the camera is where it
starts. If something moves it, `frame` puts it back.

## What the panel tells you

* `age N of 3   day N of 6`, and under it the omen: *all quiet*, *the elders
  are uneasy* (the water comes tomorrow), *THE WATER IS HERE*.
* Your treasury: **food, wood, stone, gold**.
* One line per city and how many souls are in it. Both cities are listed; that
  is the game telling you, not you looking at their screen.
* **A line in amber that names the next thing to do.** It is the fastest way to
  orient yourself and it is never wrong about what is missing.
* The status line: `playing`, `waiting on city N`, or **`DESYNC with city N at
  tick T`** in red. If you ever see the red one, **stop and say so at once** —
  that outranks everything else in the run.
* At the foot: `tick N   peers at [...]`, and under it the build hash and the
  seed. Nothing is ever drawn over those rows — if something is waiting that
  the panel has no room for, it says how many.
* Your treasury reads `wood 40+130` when some of it is in somebody's arms. The
  first number is what you can spend; the second is on its way somewhere.
* A refusal is written **under the map** in red and now *stays* there, dimmed,
  until you do something that works. If a click seemed to do nothing, read it.

## Building

Press the digit, then click the ground. The button shows what it costs; you may
start something you cannot yet pay for, because your people haul the materials
to the site over time and build it there. Nothing builds itself: somebody has
to be free to do it.

## Reading the ground

`hover-cell` over bare ground now answers, and this is the most useful verb you
have:

    grass  height 142  the water reached here
    rock   height 201
    sand   height  96  water: wading

* **height** is how high that cell stands, on the same scale everywhere, so you
  can compare one cell with another. Higher is safer.
* **the water reached here** means the last flood covered this cell deep enough
  to wade in. **The map is shaded, faintly, wherever that is true** — so after
  the first flood you can see where the water goes and plan against it. Before
  the first flood there is no mark, and height is all you have.
* **water:** *underfoot*, *wading* or *out of your depth* says what is standing
  there right now. It appears on buildings too, so you can ask whether your own
  granary is under water.

Hovering a building says what it is doing, what is stored in it, and — for a
dike — its level and how hard the water is leaning on it.

**Not every cell will take a building.** Rock, water and ground already spoken
for will refuse. Two things tell you, and both are on the map rather than in
the panel:

* with a build tool held, the square under the cursor is shaded **green if it
  will go there and red if it will not** — so `hover-cell` then `shot` before
  committing to ground you are unsure of;
* a refused click writes a line **under the map** saying why. `panel` includes
  that line at the bottom of the image it gives you. Read it: a click that
  seems to have done nothing has almost always been answered.

* **3 granary** — the only place food is kept, and the only place it is eaten.
* **2 farm** — the only thing that grows food. Three slots, and a farm feeds
  far more than the eight people you start with — food is a clock, not *the*
  clock. It fills its own small buffer and stops until a hauler empties it, so
  a farm with no granary to send to makes very little whatever its slots say.
* **1 cottage** — beds. Two adults sharing a fed cottage become a household.
  A cottage that is **not built yet** says so; only a *full* one says there is
  no room.
* **4 forester's hut** — the only source of wood.
* **5 quarry** — the only source of stone. It needs rock beside it.
* **6 stockpile** — free; somewhere to put things down.
* **7 dike** — a wall. *Press 7 and drag*: it goes down as three-cell segments
  along the line, and the cost under the cursor says both what it costs in
  stone **and how many days of one pair of hands** it will take to build. The
  second number is the one that has hurt people: the hands that build a wall
  are the hands that carry grain.
  **Click an existing segment to raise it a level.** A raise takes effect at
  once and puts that segment back to being built, so its row changes from
  `level 1 of 4` to `level 2 of 4, being built` — that is the raise working,
  not a refusal. A segment that is not finished cannot be raised and will say
  `it is not built yet`. Hovering a wall says how strained it is, and a stretch
  that gives way is announced under the map.
* **8 trading post** — its workers are mules that carry goods to the other city
  and come back with gold. They need a way across.
* **9 nursery** — no nursery, no children.
* **0 builders hut** — free, and a roster rather than a workplace: nobody
  stands in it. Anyone you assign to it becomes a **builder**, and a builder
  takes a construction site first and carries loads only when there is no site
  to work on. That is the opposite of an unassigned citizen, who carries first
  and builds when there is nothing to carry. Assign as many as you like — the
  hut has no limit, though only four can crowd one site at a time.
* **c cookery** — two slots. It is the only building that **eats a good to make
  one**: haulers bring it raw food and its cooks turn it into **meals**, and
  one meal feeds a citizen as far as two units of raw food do. Meals are kept
  in the granary alongside food and are eaten first. The treasury row grows a
  `meals N` figure the moment you have any. A cookery does not make more food
  — it makes the food you have go twice as far, so it is worth building when
  land or farmhands are the shortage and worth nothing when neither is.
* **r road** — click where it starts, click where it ends.
* **p point** — a marker, for looking at.
* **h high ground** — a toggle, not a tool. Shades every cell **higher than
  your own hearth** in green: faint for two or more terrain units above it,
  solid for six or more. Design §3.2 makes *get uphill* the order that matters
  in a flood, and until now the only way to find out where uphill was was to
  hover cells one at a time. There is more of it than the last run believed —
  measured over ten seeds, the climb available within walking distance of a
  hearth is a median of eleven terrain units, against a surge that stands
  twelve deep at the bank.

### The people tab

`tab-people` lists **one chip per person**: their name, what they are doing
right now, a sliver down the left edge saying how fed they are, and a line
along the foot of the chip saying how far through the task they are. Clicking a
chip chooses that person — `driver.py <port> click person-3`.

The same progress line is drawn **over the head of anyone working** on the map,
so a wall going up and a farm turning over are both visible without opening
anything. Only working: somebody eating or asleep has a bar in the tab and not
on the map, or most of the city would be wearing one most of the time.

Somebody walking has no bar anywhere. The game knows where they are going and
not where they set out from, so there is no honest way to draw one.

Click one of your own buildings to select it. The bottom of the panel then
offers **level** (costs gold; a level is one more citizen the building can
hold) and **m move** (it walks there and keeps what is in it).

## Your people

Drag a box over them to choose them, or press `choose all`. Then:

* **right-click a building** — go and work there. It takes as many as will fit,
  and it takes **whoever is free first**, so filling a second farm will not
  empty the first.
* **right-click the ground** — go there and stay.
* **back to hauling** — stop working, carry things again.
* **Escape** cancels everything: the tool, the selection, and any building you
  had clicked.

The **households** tab says what your whole city is doing — `3 farming, 2
hauling, 1 idle` — which is faster than hovering every building in turn.

The game tells you when people die and what of (`2 drowned`, `1 starved`), and
warns you when anybody is standing in the water. **Hauling during a flood is
how people drown**: an order given on day five can walk them into it.

Children do not work. They come of age two ages after they are born.

## Grow the city

Your city is founded with eight people and both previous playtests ended
*smaller* than they started. **This time, getting above eight is part of what
you are testing.** Try for it, and say what it cost you.

How a city grows, which is all the game will tell you and all you need:

* **1 cottage** — beds. Two adults sharing a fed cottage become a household.
* **9 nursery** — a fed household with a nursery place and a spare bed has a
  child. No nursery, no children. **Growing a city is not worth attempting in a
  three-age run** and the panel no longer suggests it: a child comes of age
  twelve days after it is born and a run is eighteen days. The buildings work;
  the arithmetic does not. Do not spend days on it unless you are asked to.
* **0 builders hut** — free. Assign people to it and they build before they
  haul, and go on doing so until you say otherwise.
* **c cookery** — turns raw food into meals worth two of it. Needs cooks.
* A child does not work, and it eats. It comes of age two ages after it is
  born, so one born in the first age is working by the third and one born just
  before the last flood never works at all.
* The **households** tab lists them, and hovering one rings those people on
  the map.

Growing is not free and it is not obviously right: every mouth eats whether or
not it works, and the water is still coming on day six. Whether it was worth it
is one of the things this run is for, so if you decide partway through that it
was a mistake, say so — that is a finding, not a failure.

## How to play this run

* **Look every ten to fifteen seconds** through a quiet day. Most looks want no
  decision at all, and that is normal. Tighten to every five on day six of each
  age, when the water is coming.
* **A day is one minute of real time. A whole run is eighteen minutes.** You
  cannot pause it and you cannot speed it up, and a day goes past faster than
  you expect — four looks is a whole day gone.
* Do not spend the run reading. `panel` is cheaper than `shot`; use `shot` when
  you need to see the ground.

## Write it down as you go

The account is the point of the exercise, not a by-product. Keep notes as you
play, not afterwards:

* what you built, in what order, and **why you chose it**;
* **how many souls your city held at its height**, whether growing was worth it
  against the flood that followed, and whether a child born early ever paid its
  way;
* when the water arrived, how far it got, and who it took;
* **whether the wall was worth building** — this is the question the run exists
  to answer;
* anything confusing, unreadable, or that silently did nothing;
* anything that was *fun*, and anything that was not;
* **whether the things listed above actually helped.** Most of them are new
  since the last playtest and exist because the last two players said they were
  missing: the ground's height and the high-water mark, the wall's strain, who
  died and of what, what is in somebody's arms. If one of them changed a
  decision you made, say which. If one of them was noise, say that — it is just
  as useful.

## What the panel says when something goes wrong

Three things changed in M12 and all three were costing players whole days.

* **A right-click always answers.** With nobody chosen it now says *"nobody
  chosen - drag over your people first"* instead of doing nothing at all. In
  the last run a player right-clicked the cell they had just built a forester
  on, got silence, and lost two game-days believing the building was in the
  wrong place. It was not; the click had nobody to send.
* **A refusal names the real problem.** Every "there is no room" used to cover
  four different problems. Now: *"it is not built yet"* for a site, *"there is
  no work there"* for a building nobody works at — a nursery — *"there is no
  room"* only when the slots are genuinely taken, and *"that is not yours"*.
* **Deaths have their own line**, above the refusal line and in a colour of
  their own. They accumulate: a flood that takes eight people reads
  `8 drowned`, not `1 drowned` eight times. A click-refusal in the same frame
  no longer wipes it.

And the **amber line puts the water first**. It used to be ordered as though
food were the only clock, so it recommended a trading post to a player with a
flood two days out. Now the water outranks everything, including "you have no
granary" — drowning tomorrow beats starving in three days.

The in-the-water warning says **how deep**: *"8 of your people are in the
water, out of their depth"* is an evacuation; *"wading"* is not.
