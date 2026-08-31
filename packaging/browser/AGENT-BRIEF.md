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

---

## The game

You are one of two cities on a river. Both banks, one city each, and there is
one ford — water shallow enough to wade, slowly. It is the only crossing until
somebody builds a bridge, and **it closes when the water comes**.

There are three ages. Each age is six days. **On day six the river floods**,
and the panel warns you the day before. Survive three of them.

Your people starve on day four if they cannot eat, so the first flood is not
the first thing that can kill you.

## Your hands

    driver.py <your port> <verb> [args]

    panel                     read your side panel - do this most often
    shot                      the whole window, map included
    rows                      the status line and the three rows at the foot
    key <Name>                Digit1..Digit9, KeyR, KeyP, KeyM, Escape
    button <name>             a panel button by name
    click-cell <x> <y>        the map, 0..127 both ways
    right-click-cell <x> <y>  give an order, or put the current tool down
    hover-cell <x> <y>        park the cursor: the panel says what is under it
    box-select <x0> <y0> <x1> <y1>    choose the people inside a rectangle
    drag-cells <x0> <y0> <x1> <y1>    draw a wall
    frame                     the whole map again
    wait <seconds>            let the world run

Button names: `cottage farm granary forester quarry stockpile dike post
nursery road point choose-all back-to-hauling trade tab-build tab-households`.

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
* At the foot: `tick N`, `peers at [...]`, the build hash and the seed.

## Building

Press the digit, then click the ground. The button shows what it costs; you may
start something you cannot yet pay for, because your people haul the materials
to the site over time and build it there. Nothing builds itself: somebody has
to be free to do it.

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
* **2 farm** — the only thing that grows food.
* **1 cottage** — beds. Two adults sharing a fed cottage become a household.
* **4 forester's hut** — the only source of wood.
* **5 quarry** — the only source of stone. It needs rock beside it.
* **6 stockpile** — free; somewhere to put things down.
* **7 dike** — a wall. *Press 7 and drag*: it goes down as three-cell segments
  along the line, with a ghost and a running cost under the cursor. Click an
  existing one to raise it a level. Walls take stress from water leaning on
  them and break if it is too much for their level.
* **8 trading post** — its workers are mules that carry goods to the other city
  and come back with gold. They need a way across.
* **9 nursery** — no nursery, no children.
* **r road** — click where it starts, click where it ends.
* **p point** — a marker, for looking at.

Click one of your own buildings to select it. The bottom of the panel then
offers **level** (costs gold; a level is one more citizen the building can
hold) and **m move** (it walks there and keeps what is in it).

## Your people

Drag a box over them to choose them, or press `choose all`. Then:

* **right-click a building** — go and work there. It takes as many as will fit.
* **right-click the ground** — go there and stay.
* **back to hauling** — stop working, carry things again.

Children do not work. They come of age two ages after they are born.

## How to play this run

* **Look every twenty to thirty seconds** through a quiet day. Most ticks want
  no decision at all. Tighten to every five or ten on day six of each age,
  when the water is coming.
* A day is two minutes of real time. A whole run is thirty-six minutes. You
  cannot pause it and you cannot speed it up.
* Do not spend the run reading. `panel` is cheaper than `shot`; use `shot` when
  you need to see the ground.

## Write it down as you go

The account is the point of the exercise, not a by-product. Keep notes as you
play, not afterwards:

* what you built, in what order, and **why you chose it**;
* when the water arrived, how far it got, and who it took;
* **whether the wall was worth building** — this is the question the run exists
  to answer;
* anything confusing, unreadable, or that silently did nothing;
* anything that was *fun*, and anything that was not.
