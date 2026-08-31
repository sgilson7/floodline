# FLOODLINE — the plan for M10

`HANDOFF-M10.md` says where the ten-milestone plan stands and what M10 is for.
This says how to execute it: eight sub-milestones, each with its deliverables
and its done-condition, in an order chosen so that nothing which can fail
cheaply is allowed to fail during the thirty-six minutes.

Read `CLAUDE.md`, `HANDOFF.md` and `HANDOFF-M10.md` first. Nothing here
replaces them.

This document is also an artifact, if a link is easier to hand on than a file:
**<https://claude.ai/code/artifact/b7021af4-ec24-4772-a08f-c14b1706003b>**

---

## Where this stands

| | | |
|---|---|---|
| M10.1 | Two agents in one room | **done** — green on localhost and the deployed build |
| M10.2 | The clock, decided and written down | **done** — three paragraphs in `DECISIONS.md` |
| M10.3 | Hands and eyes | **done** — `table.py`, `panel.py`, `driver.py`, `driver_check.py` |
| M10.4 | The referee, and ten minutes of nothing going wrong | **done** — clean, though the soak is eight minutes and not ten |
| M10.5 | A rehearsal: one age | **done** — and it answered M5's parked question |
| M10.6 | The run | ← **next**; push first |
| M10.7 | The account | |
| M10.8 | What the run demands | two findings waiting; the food line was pulled forward |

Setting the table found three things, which is what the ordering was for. Two
are fixed (`peers at` said nothing in a browser; the panel's variable row still
moved three fixed rows). Two are recorded and deliberately left for M10.8: only
the host can notice a desync, and on one the joiner freezes without being told.
All four are in `DECISIONS.md`.

The one addition to the plan below: `table.py`, which owns the lobby dance so
that `two_agents.py` and a real run cannot drift apart, and which leaves the
browsers standing with a debugging port each.

---

## What M10 is, and what it is not

It is **a game, played to the end, by two hands that do not share a screen** —
and a written account of it. Everything built since M1 is exercised because the
game contains it, not because a script visits it.

It is **not** a balance pass. M5 parked a question about the food economy
precisely so that a run would answer it, and the answer is a *finding*, not a
patch. Tuning belongs to M10.8 at the earliest, with a probe and a table
behind it, and probably to a session after this one.

The two things that end the run early are both successes of the instrument:

* **a desync banner** — that is the headline finding and it outranks everything
  else in this document;
* **a peer dropped** — thirty seconds of a page not rendering, which M10.4
  exists to make impossible before it can cost a run.

---

## Four decisions, made here rather than during the run

### 1. The run happens at real speed. There is no clock multiplier.

The handoff offers a test-only multiplier on `Clock::ticks_due` as the small
version. It is not available, and the reason is not taste:

* The done-condition names **the deployed build**. A multiplier lives in
  `crates/gui/src/main.rs` and must never reach a shipped build. Both cannot
  hold — to multiply the clock on the deployed page you have to deploy the
  multiplier.
* The escape is to run both peers on a matching local build, which the
  build-hash guard permits. But then the one artefact nobody has ever played —
  the page a player actually opens — stays unplayed, and that is the milestone.
* And it would compress the variable under test. A day is two minutes of
  thinking time. At four times the clock it is thirty seconds, and "was the
  wall worth building" is partly a question about whether a city can afford the
  *attention*, not only the labour.

Accepted cost: thirty-six minutes of wall clock for the run, about twelve for
the rehearsal, both of them spent watching. A multiplier would not make the
agents faster; if anything it makes keeping up harder. `DECISIONS.md` gets this
paragraph in M10.2.

### 2. Two browsers, not two contexts.

The handoff asks for two contexts so the agents do not share a clipboard, a
`localStorage` or a permission grant. Two separate `chromium.launch()` calls
give that and one more thing that matters more over thirty-six minutes:
**independent renderer scheduling**.

`Lockstep::DROP_AFTER_TICKS` is 300 ticks — thirty seconds — and
`Clock::MOST_PER_FRAME` is 8, so a page must render at least 1.25 frames a
second to hold ten ticks a second, and a page that stops for thirty seconds is
dropped from the game by the other peer. Chromium throttles animation frames in
occluded and backgrounded pages, and two pages in one browser cannot both be in
front. Each browser is launched with
`--disable-background-timer-throttling --disable-backgrounding-occluded-windows
--disable-renderer-backgrounding`, and M10.4 proves it rather than assuming it.

### 3. An agent's hands are one-shot commands over CDP.

An agent's turns are separate processes; it cannot hold a `sync_playwright()`
session open across them. So the harness launches the two browsers once, each
with its own `--remote-debugging-port`, and every agent action is a short
`connect_over_cdp`, act, screenshot, disconnect. The state lives in the
browser, which is where it already lives.

Each agent is given **exactly one port and never learns the other's**. That is
what turns "neither may read the other's page" from a promise into a property
of the setup.

### 4. The agents are told what a player could know, and nothing else.

A brief: the controls, the goods, the buildings, the deadline, the river and
the ford — the first-run card and the manual, written down. **Nothing from
`crates/sim`**: no balance constants, no source, no probe tables, no idea which
dikes break. Otherwise the run measures an agent's reading of `balance.rs`
rather than the game.

---

## The milestones

### M10.1 — Two agents in one room

The handoff's single next action, and nothing more than that. No decisions
about what to build, no reading of the panel.

**Deliverables**
* `packaging/browser/two_agents.py`, from `game_two_tabs.py`: two browser
  launches with the anti-throttling flags, one page each, the room-code path
  only, a random room typed into both, Host on one and Join on the other,
  `links.size > 0` on both, Start, the modal first-run card dismissed on both,
  and the map drawn on both.
* It takes a URL so it can be pointed at either build, and defaults to the
  local one.
* A line in `packaging/browser/run.sh` and a row in
  `packaging/browser/README.md`.
* Lobby geometry stays as literals copied from `crates/gui/src/lobby.rs`, for
  the reason `game_two_tabs.py` says: if the lobby moves, this should notice.

**Done when** it is green against `http://localhost:8123/index.html` *and*
against <https://sgilson7.github.io/floodline/>, and committed. `make test`
green, `make browser-test` green.

---

### M10.2 — The clock, decided and written down

**Deliverables**
* A `DECISIONS.md` paragraph carrying decision 1 above, in date order.
* Two more paragraphs beside it: the thirty-second drop window and what it
  demands of the harness (decision 2), and the CDP shape and the one-port rule
  (decision 3).
* A stated polling cadence for the agents: about every twenty-five seconds
  through a quiet day, tightening to five or ten during day six of each age.
  Roughly ninety looks each over a run.

**Done when** the paragraphs are in and committed. This is minutes of work and
is its own milestone only because the handoff asks for the choice to be made
deliberately rather than by accident.

---

### M10.3 — Hands and eyes

The verbs an agent needs, proven before anything depends on them.

**Deliverables**
* `packaging/browser/driver.py` — one-shot CDP verbs, importing `view.py` for
  every map coordinate and never doing the letterbox arithmetic itself:
  `shot`, `panel`, `key`, `click-cell`, `right-click-cell`, `click-logical`,
  `drag-cells` (the dike gesture), `box-select`, `hover-cell`.
* `packaging/browser/panel.py` — the panel's rows as named running totals from
  `crates/gui/src/draw.rs` and `input.rs`: treasury, the city list, the two
  tutorial rows, the status row, `tick`, `peers at`, `build`/`seed`, the tab
  row, the build buttons, the hover row, and the variable row that now sits
  below everything fixed. One copy, for the new scripts. The literals in
  `play.py` and `assign.py` are left alone: their job is to notice when the
  panel moves, and they keep it.
* `packaging/browser/driver_check.py` — the driver doing one of each verb in a
  live single-player game, in the shape `play.py` already proves by hand.
* A row in `README.md` for each, and `driver_check.py` in `run.sh`.

**Done when** `driver_check.py` is green at device pixel ratios 1 and 2 and
committed. A verb the run turns out to need and this does not have is a bug in
this milestone, found late.

---

### M10.4 — The referee, and ten minutes of nothing going wrong

The instrument that says whether the peers stayed together, and the soak that
says the harness can survive a run at all. This is the milestone that pays for
itself: rendering throttle is invisible for four minutes and fatal at thirty.

**Deliverables**
* `packaging/browser/referee.py` — attaches to both pages over CDP, issues no
  input, touches no map, and every fifteen seconds appends a timestamped line
  and saves a crop of each panel's bottom rows. It reads only what is drawn:
  the desync banner is detected as red in the status row, the way
  `assign.py::alarm_band` detects a refusal. It is an instrument, not a third
  player.
* A soak mode: both tabs in a real game for **ten unattended minutes**,
  reporting the tick rate of each page and the largest gap between the two
  numbers in `peers at`.
* Whatever the soak demands to come out clean — headed mode, one browser per
  agent, different flags — landed here, with a paragraph in `DECISIONS.md` if
  somebody would have chosen differently.

**Done when** a ten-minute soak on the deployed build shows both pages holding
about ten ticks a second, `peers at` never parted by more than the ordinary
turn delay, no drop, no desync — and the log is committed as the evidence that
it did.

---

### M10.5 — A rehearsal: one age

Twelve minutes, two agents, the deployed build, through the first flood. Not a
dry run of the harness — a real short game, played for real, to shake out the
loop before thirty-six minutes are committed to it.

**Deliverables**
* `packaging/browser/AGENT-BRIEF.md` — decision 4's brief, and the first thing
  the rehearsal tests.
* `PLAYTEST-M10.md` started, and kept *during* the rehearsal rather than after.
* A list of harness gaps: a verb the driver lacks, a panel row that cannot be
  read at the fit, a cadence that is too slow to react to the water.
* Any *game* bug found, filed where it belongs — a `sim` bug gets a `sim` test
  in the same commit, a `gui` bug gets a browser check — and reproduced before
  it is touched.

**Done when** both cities have finished age one with no desync, the fixes are
committed and green, and the loop is judged good enough to run to age three.
If it is not, the rehearsal repeats. It is twelve minutes; the run is
thirty-six.

---

### M10.6 — The run

Three ages, six days each, both cities, on the deployed build, at ten ticks a
second.

**Deliverables**
* The referee log, end to end, with every fifteen-second sample in it.
* The panel filmstrip: both panels, throughout.
* Both score screens, screenshotted — "The map stood." or "The last city fell."
* A running account, written as it happens (M10.7 is the shaping of it, not the
  gathering).

**Done when** two agents have finished a run and neither client ever showed a
desync banner. If one appears: **stop, screenshot both pages, record both tick
counts and the tick the banner names, keep the log, and write it up first.**
That is the milestone's most valuable possible outcome and the rest of the
account is then secondary.

---

### M10.7 — The account

The deliverable design step 7 has been waiting for since phase 5.

**Deliverables** — `PLAYTEST-M10.md`, saying:
* what each city built, in what order, and why;
* when the water arrived, how deep it got, and who it took;
* **whether the wall was worth building** — M5's parked question, answered by
  hands rather than by a probe;
* what was confusing, unreadable at the fit, or silently did nothing;
* what was *fun*, which is the one thing no probe in the repo can report.
* Published as an artifact and linked from the file, the way `HANDOFF-M10.md`
  is — a link is easier to hand on than a file.

**Also**
* `DECISIONS.md` paragraphs for anything the run decided.
* `PROGRESS.md` session entry: phase, done/not done against the checklist,
  decisions, blockers, the single next action.
* `HANDOFF-M10.md`'s table updated, or a handover for whatever comes after.

**Done when** somebody who was not there can read what happened and what it
means for the game.

---

### M10.8 — What the run demands (contingent)

Everything the run found and nothing it did not.

**Deliverables, if and only if the run asks for them**
* The food economy, which is M5's parked question: a farm's yield and how many
  hands a city of eight can spare. **Measured, not picked** — a probe, run, and
  its table pasted into the constant's doc comment, in the house style.
* Any bug that made the run unfinishable, with the test that would have caught
  it.

**Done when** the changes carry their measurements and `make test` is green —
or, just as good, when this milestone is empty because the run said the numbers
were right.

**This is very likely its own session.** The run is not held open waiting for
it.

---

## The traps, and which milestone eats them

Every one of these is in `HANDOFF-M10.md` and every one of them has cost real
time already.

| trap | where it is handled |
|---|---|
| The panel has shifted five times and each time broke two checks silently | M10.3 — `panel.py` is one copy; the old literals stay as the tripwire |
| Two pages in one browser share a clipboard and a scheduler | M10.1 decision 2, proved in M10.4 |
| A page that stops rendering for thirty seconds is dropped | M10.4's soak, before it can cost a run |
| A local build and the deployed build cannot join each other, silently | M10.1 — one URL argument, both pages, never mixed |
| The first-run card is modal and covers the map | M10.1 — dismissed on both, checked |
| An agent that reads the world out of the page is not playing the game | Decision 3's one-port rule, and decision 4's brief |
| Reading `balance.rs` instead of the panel | Decision 4 |
| Tuning from a probe before the run rather than from the run | M10.8 is last, and contingent |

---

## What it costs

| | |
|---|---|
| M10.1 – M10.4 | the harness; an afternoon, most of it in M10.3 |
| M10.5 | twelve minutes of play, plus the fixes it demands |
| M10.6 | thirty-six minutes of wall clock, unskippable |
| M10.7 | the writing |
| M10.8 | a session of its own, if the run asks for it |

The run is about ninety looks per agent. That is the real budget, and it is the
price of the only question in this repo that no test has ever been able to
answer.

---

## The single next action

`packaging/browser/two_agents.py`: two browsers, two contexts, one room, the
deployed build, green and committed. Nothing that decides what to build gets
written before that is done.
