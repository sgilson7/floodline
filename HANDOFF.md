# FLOODLINE — handoff

Written at the end of the session that finished the MVP — phases 4, 5 and 6 of
`floodline-mvp-plan.md` (the v2, no-server plan), on top of the session that
built 0 to 3. Everything below is true of
commit `main` as it stands; `git log` is the authority if this drifts.

Read `CLAUDE.md` first, then `floodline-design.md` and `floodline-mvp-plan.md`,
then `DECISIONS.md`. This file is the shortcut through the last of those.

---

## Where it is

**The MVP is finished, deployed and green.**
<https://sgilson7.github.io/floodline/>

One player hosts and shares a room code or a pasted invitation; the other
joins; both play the same world, and nothing runs anywhere but GitHub Pages.
212 tests, seven browser checks, no warnings, `make test` in about twelve
seconds.

| Phase | | |
|---|---|---|
| 0 | Repository and pipeline | **done** |
| 1 | `sim`: land, citizens, buildings | **done** |
| 2 | `sim`: water and bodies | **done** |
| 3 | Lockstep on `net::Loopback` | **done** |
| 4 | `quad_rtc.js` and `net-web` | **done** |
| 5 | `gui` | **done**, except that nobody has played it — see below |
| 6 | Hardening the serverless deployment | **done** |

The one thing left in the plan is the one thing no test can discharge: design
step 7, "playtest the flood until it is fun". `PROGRESS.md` names the three
questions a person needs to answer.

---

## The shape of the thing

```
crates/sim/     the whole game. serde + postcard, nothing else, no floats
crates/net/     Peer trait, Loopback, wire format, the star lockstep
crates/net-web/ Peer over web/quad_rtc.js — wasm32 only
crates/gui/     macroquad: renderer, panel, lobby, input, score screen
web/            index.html, quad_rtc.js, config.js, echo.html
web/vendor/     Trystero, pinned by filename and sha256
packaging/      package-web.sh, and browser/ for the checks that need Chromium
```

`sim` is the bulk of it and its tests are most of that again. That ratio is
deliberate and is most of why the thing works.

### Where to start reading

* `crates/sim/src/balance.rs` — every number that is a judgement rather than a
  fact, with the reasoning next to it. If something feels wrong to play, this
  is the file to argue with.
* `crates/sim/src/world.rs` — `World::tick` is nine lines and its doc comment
  explains why the stages are in that order. `World::apply` is the only door
  into the world.
* `crates/net/src/lockstep.rs` — the star from design §8.
* `crates/net/tests/lockstep.rs` — the best single description of what the
  networking is supposed to do.
* `web/quad_rtc.js` — the whole of the browser's side of the network, and the
  file DECISIONS.md's "The handshake, written down before the plugin" is about.
* `crates/sim/tests/playtest.rs` — five strategies through full three-age runs.
  Not an assertion: a measurement, run with `--ignored --nocapture`. It is the
  only thing in the repo that answers "is this a game".

---

## What to do next

**Play it.** Design step 7 is the only item in the plan that a passing test
cannot discharge, and `PROGRESS.md` names the three questions that came out of
measuring what could be measured: age three kills everybody on two seeds in
three whatever is done, nothing in the MVP produces stone so a player gets
exactly one wall in a whole run, and a run is thirty-six minutes.

Everything else in `floodline-mvp-plan.md` is checked. `M2` onward in "Later
milestones" is where new work goes.

## Things that will bite you

Each of these cost real time to find. They are all in `DECISIONS.md` in full;
this is the index.

* **A `Turn`'s checksum is tagged with the tick it describes.** Design §8 says
  "after tick T − 1" and that cannot be true, because a turn for T is sent
  `DELAY` ticks early. Do not "simplify" it back.
* **Nothing happens until the host presses Start.** Without the lobby the host
  is fifty ticks ahead before anyone finishes connecting.
* **The host waits only for players actually connected**, not for the empty
  seats a world was generated with.
* **Giving up on a silent player does not go through the ordinary command
  queue** — it deadlocks on the very player it means to drop.
* **A day is 1200 ticks, not 200.** Design §4's three numbers contradict each
  other; the prose won. Everything keyed to a day moved with it.
* **Water is in sixteenths of a terrain unit**, and the terrain's relief is 40,
  not 255. Both are forced by the design's own dike and surge heights. Changing
  either without re-running `map::probe::sweep_noise_amplitude` and
  `tests/water.rs` will quietly ruin the flood.
* **`sim` may not gain a third dependency.** `tests/boundary.rs` reads its
  `Cargo.toml` and will stop you. If you genuinely need one, that is a decision
  to write down, not a line to edit.
* **macroquad's bundle needs `var register_plugin;` declared before it loads.**
  It assigns to an undeclared global under `"use strict"`. Already in
  `index.html`; do not tidy it away.
* **macroquad's built-in font is ASCII** and draws a hollow box for anything
  else, with no fallback and no warning. `gui` lints its own string literals
  for this; `Refusal::to_message` and `RuleError::to_message` are checked in
  their own crates, because the lint reads only `gui`'s files.
* **Both data channels are negotiated on fixed stream ids**, so there is no
  `ondatachannel` event and no race about who was listening. Do not "simplify"
  that back to the in-band form; the race it removes cannot be closed.
* **The first byte on the reliable channel says host or joiner.** Trystero
  rooms are meshes and two joiners will meet; design §9.2's "a joiner accepts
  the first peer" is a guess that is wrong about a third of the time with three
  players.
* **Hearth sites sit on a line at a fixed distance from the low corner**, not
  on a ring around the map centre, and `balance::SHORE_DISTANCE` carries the
  measurement that says why. Moving them back is moving whole cities out of the
  flood.
* **A citizen with nothing to carry builds.** Removing that is removing the
  reason an unattended city does not starve on day four. Farming is
  deliberately *not* automatic, and `nobody_takes_a_job_that_was_not_given_to_them`
  holds that line.
* **A `MoveTo` holds**, until `Unassign`. "Get uphill" is the order the flood
  is about and it does not work otherwise.
* **A locally built page and the deployed page have different build hashes**,
  and design §8 says mismatched builds cannot join. The hash is the sha256 of
  the wasm, and CI's rustc is not the same version as yours, so the same source
  produces a different binary. This is correct and is the guard doing its job —
  but a local tab and the deployed one will refuse each other and the reason
  will not be obvious at the time. Test two tabs on the *same* build.

## How the tests are meant to be used

* `make test` — everything, ~12s. Must be green at every commit.
* `cargo test -p sim --test scenario` — the game: two cities, a road, three
  days of trade, and a flood a dike survives.
* `cargo test -p net --test lockstep` — the networking, all of it, no browser.
* `cargo test -p sim --release --test profile -- --ignored --nocapture` —
  a tick at 500 citizens with the flood running. 0.36 ms against a 20 ms budget.
* `cargo test -p sim probe -- --ignored --nocapture` — the terrain sweep. A
  measurement, not an assertion.
* `cargo test -p sim --release --test playtest -- --ignored --nocapture` — five
  strategies through full three-age runs, and how far each age's water reaches
  from the corner it comes out of. Also a measurement, and the one that found
  four bugs no assertion had.
* `make browser-test` — the seven things only a real browser can answer.

`sim` is built with `opt-level = 2` under test. Without it the flood tests take
two minutes, and a determinism test nobody runs is worse than none.

## Verifying the browser build

```
make browser-test
```

Seven checks: three on the transport alone through `web/echo.html` (no wasm, no
`sim`, no lockstep), one on the mouse reaching the simulation, one on the
letterbox at a device pixel ratio of 1 *and* 2, and two on the whole stack in
two tabs — one per signalling path. `packaging/browser/README.md` says what each
answers. The first run builds a virtualenv and downloads Chromium.

Two habits worth keeping, because between them they found the `register_plugin`
error, the letterbox bug, the font's hollow boxes and Trystero's
"User-Initiated Abort": **read the console**, and **run at a device pixel ratio
of 2 as well as 1**. Every script forwards `pageerror` and `console.error`.

Against the deployed build instead of a local one:

```
./.venv-test/bin/python packaging/browser/echo_room.py \
    https://sgilson7.github.io/floodline/echo.html
```

## What has never been tested

Honesty about the edges:

* **Nobody has played it.** Two tabs on one machine have played it, and the
  browser checks assert that they do; a person has never sat down with it. The
  flood has been tuned against measurements only, which is the part measurement
  can reach — see `DECISIONS.md`, "Design step 7".
* **Two browsers on two different networks have never talked to each other.**
  Everything is verified across tabs and against the deployed Pages build,
  including over the public Nostr relays and the BitTorrent trackers, but both
  ends were always this machine. The case that cannot work without a TURN
  server — both players behind strict NATs — is by definition untested and is
  the one thing in the game that can cost money to fix.
* **More than three peers has only been tested on `Loopback` and in
  `echo.html`.** The star is enforced and the three-tab check passes; six
  browsers have never been in one room.
* Ages 4+ exist in the escalation table and are unreachable: the MVP stops at 3.
