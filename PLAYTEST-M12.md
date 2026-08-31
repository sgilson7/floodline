# FLOODLINE — the M12 run

M12.11. Two agents, one browser each, a full three-age run, neither able to
see the other's screen and neither allowed to read `crates/sim`. Eighteen
minutes.

**The referee: CLEAN.** 17 days in 18.2 minutes on both peers, zero samples
with no tick, zero red. The lockstep and the clock are not the problem and
have not been since M10.

## The result

**City 1: three souls of eight, all three ages survived. City 0: none, two
ages.** Neither city ever exceeded the eight it started with. Nobody has, in
four playtests.

---

## The finding: eight deaths and five deaths, and neither player was told

City 0 lost eight people. City 1 lost five. **Neither ever saw a death
message**, and both were watching for one — the brief describes the feature in
detail and both had read it.

> Eight people died in this run and the game announced not one of them.
> — city 0

> Five people died and the game told me about masonry.
> — city 1

The line was being drawn. `driver.py panel` — the cheap verb, the one the
brief tells an agent to use most, the agent's eyes — crops from sixty pixels
above the foot. M12.5 put the toll at ninety-six and M12.7 put the report at a
hundred and forty. **The two new slots were outside the players' own eyes.**

That is the fault `driver.py`'s own docstring warns about, in the same words,
about the same crop:

> cropping it away is how both players in M10.5 came to believe their clicks
> were being ignored

It was made again by a milestone that fixed the message and moved it up the
screen. **A slot added above that line has to move that line**, and nothing
said so until two players lost thirteen people between them and told us.

City 1's account of the moment is the one to keep. Its five deaths arrived as
a single sentence about a wall:

> The line under the map said **2 stretches of your wall have given way**.
> That is *all* it said. I learned I had lost five-eighths of my city by
> noticing the souls count had gone from 8 to 3.

---

## The wall, a fourth time — and this time neither player ever had one

Both built. Neither finished. City 0:

> **The wall was never built.** I hovered those segments at the end of the run
> and every single one still said `dike: being built`. Not one of fifteen ever
> reached `level 1 of 4`. Four hundred and forty stone and two days of four
> people's labour went into the ground and produced *nothing* — and the only
> word the game ever had for that state was `being built`.

It had placed a builder's hut on day 3 and never assigned anybody to it,
because nothing ever told it to:

> The households tab has no `building` state, so "0 people are building" is a
> fact the game is structurally incapable of showing you. The amber line never
> once said "your dike has no builders". It was still recommending a trading
> post while 440 stone of wall sat inert.

City 1, independently, on the same state a full age later:

> On age 3 day 3 I hovered the same cell and it still said `dike: being built`.
> Roughly 300 stone and a day of my entire city, and I never once owned a
> finished wall.

And both were led there by the same number. The cost readout says
`1 x dike — 30 stone, 0.1 days of one pair of hands`. City 1:

> That number is a lie of omission, and it's the reason I committed. 0.1 days
> sounds free. What it doesn't count is that thirty stone has to be *carried*
> to a cell twenty tiles away through standing water by people who are also
> the only people fetching your food.

**Four playtests have now asked whether the wall is worth building and not one
has ever finished one.** M10 said it was unreadable; M11 made it legible; M12
finds that the thing being read was a construction site nobody was working on.

---

## `h` — the right answer, rendered below sight

M12.9 measured that the ground is not flat and added an overlay. Both players
found the overlay and neither could see it.

City 0 threw it away in the first ten seconds, then went back afterwards and
measured it properly:

> **The overlay is correct.** The region it shades is precisely the rock
> plateau at heights 30–35 that I retreated to twice. It knew. The largest
> difference on any channel is 36 out of 255, in green, painted onto green
> grass. It is a true signal rendered below my ability to see it.

City 1 kept it, trusted it, and paid:

> I looked at the map and saw brighter green up in the north-east, so that's
> where I sent everybody, twice. [...] **I had twice evacuated my city to
> ground three units lower than the ground it was standing on**, because in a
> screenshot I cannot tell the overlay's green from grass's own green.

Five people drowned on the second of those evacuations. **A signal that cannot
be distinguished from the map under it is worse than no signal, because it is
still trusted.**

What both players used instead was `hover-cell`, one cell at a time — the
tedium the overlay was built to replace. City 0 spent two game-days on it:

> The most useful instrument in the game is priced in the resource the game is
> least generous with. I got the map of my valley and paid for it with the
> flood I was mapping it for.

---

## The food clock, which is now a tutorial wearing a costume

M12.A tripled a farm. Both accounts say the same thing about the result.

City 0:

> That reads like a crisis. It is not one. Food went 5 → 120 → 242 → 294 → 481
> in about a minute and a half. From that moment to the end of the run —
> sixteen minutes — **food was never again a consideration**. I died with 349
> in the granary and nobody to eat it.

That is what the change was for, and it worked. What it also did:

> The starvation clock is a two-minute tutorial wearing the costume of a
> threat, and because it shouts at you on day 1 with the only red-ish urgency
> the panel has, it teaches you to solve the wrong problem first.

**And the amber line then gave the wrong advice about it.** City 1 employed
all eight of its people and starved beside two working farms:

> Food went **428 → 386 → 312 → 219** over one day and I could not see why.
> The amber line told me *"more farmers, or fewer hands carrying stone"*.
> **That line pushed me the wrong way.** More farmers was exactly the disease.

It found the answer in the households roster — `6 farming, 2 cutting wood`,
no hauling — and fixed it in a day. The roster is the best diagnostic in the
game and it nearly never opened it.

---

## The people tab earned its place, and the households tab lied

City 0 opened `people` on age 2 day 1 and found four of its seven citizens had
been standing on an evacuation rock for three days:

    Drogo   waiting where you sent them
    Siward  waiting where you sent them
    Perrin  waiting where you sent them
    Blythe  waiting where you sent them

> It is the best screen in the game and it is the only one that told me the
> truth. **The households tab actively lied to me** and I believed it because
> it is the cheaper look.

`right-click the ground` is a permanent order and nothing revokes it. The
households roster counted those four as hauling.

City 1 used the tab twice and was unmoved by it — *"everything actionable in
it was in the households one-liner in a quarter of the space"* — but kept the
names: *"I remember that Gervase had nothing to do."*

---

## Two design findings, both new, both from city 0

**The high-water mark is a lagging indicator and nothing says so.**

> Each age's flood goes higher than the last, and the only tool for planning
> against it records where the *previous* one stopped. I used the game's own
> evidence, correctly, and it killed my last two people.

It parked its final two citizens on rock at height 35, unmarked after **both**
previous floods, and the third flood covered it.

**The only ground that survives is ground you cannot build on.**

> Rock at 30–35 was dry through two floods; grass tops out around 29 and
> drowns. So the city has to live in the flood plain and evacuate, every age,
> forever. That's a coherent design — but I'd have liked to learn it in age 1
> rather than age 3.

Attempting to move a granary onto it answers `not on that ground`, which does
not say *why*, and does not say *that is rock*.

---

## Growth, again

Neither city gained a soul. City 0 built a cottage and a nursery in age 2 and
finished neither; city 1 formed two households that read `0 children - next
0%` for the whole run and left a nursery as a hole in the ground.

City 0 also caught the brief contradicting itself, which is M12.10's decision
arriving half-applied:

> One paragraph says *"getting above eight is part of what you are testing.
> Try for it."* Two paragraphs later: *"Growing a city is not worth attempting
> in a three-age run."* I split the difference and wasted 70 wood.

---

## Smaller things, all new

* **`builders hut: 0 of 4294967295 working`**, on both screens.
* **A granary being moved reads `food 0`** while it walks. City 1 thought it
  had destroyed 369 food and its last three people. *"Everywhere else the
  panel is careful about this: it writes `wood 40+130`."*
* **The dike cost tooltip is drawn on the map, not the panel** — so the one
  number the wall decision turns on is invisible to the cheap verb.
* **`box-select` over four visible people chose nobody**, twice, for city 0.
  Not explained.
* **`back to hauling` with nobody chosen writes nothing anywhere.** The
  right-click path answers; the button does not.
* **Once a city is dead, hovering returns nothing at all** — the ground
  readout switches off at the moment a player most wants to ask how deep it
  got.

---

## Would they play again?

Both said yes, and both said it the same way — because they now know something
the game would not tell them.

City 1:

> **The first flood is not a thing you survive, it's a survey.** I'd found
> where I'm told to found, spend age 1 doing nothing but food and hauling,
> hover a dozen cells the moment the water recedes, and move the whole city to
> the far side of the water-mark before age 2. **I would not build a wall.**

City 0:

> Only because I now know three things the game would not tell me, and I
> resent having had to buy them at this price: put people in the builders hut
> before you draw anything; never issue an order after day 4; and the safe
> height goes up every age, so last year's flood line is a lie.
