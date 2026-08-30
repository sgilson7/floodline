# FLOODLINE — handoff

Written at the end of the session that built phases 0 to 3 of
`floodline-mvp-plan.md` (the v2, no-server plan). Everything below is true of
commit `main` as it stands; `git log` is the authority if this drifts.

Read `CLAUDE.md` first, then `floodline-design.md` and `floodline-mvp-plan.md`,
then `DECISIONS.md`. This file is the shortcut through the last of those.

---

## Where it is

**Playable, deployed, and green.** <https://sgilson7.github.io/floodline/>

The page shows a generated map with two cities on it, both eight strong, a
side panel, and a lockstep game running between two in-process peers. Press
**space** to start it. 204 tests, no warnings, `make test` in about twelve
seconds.

| Phase | | |
|---|---|---|
| 0 | Repository and pipeline | **done** |
| 1 | `sim`: land, citizens, buildings | **done** |
| 2 | `sim`: water and bodies | **done** |
| 3 | Lockstep on `net::Loopback` | **done** |
| 4 | `quad_rtc.js` and `net-web` | **not started** ← you are here |
| 5 | `gui` | renderer, panel and score screen done; **no input at all** |
| 6 | Hardening the serverless deployment | **not started** |

Phase 5's map, panel and score screen were built early because phase 3's
done-condition needed a window to look at, and a blank canvas would have proved
only that the lockstep does not crash. What is missing from it is everything a
player *does*: selection, the build menu, the road tool, the trade dialog, and
the lobby.

---

## The shape of the thing

```
crates/sim/     the whole game. serde + postcard, nothing else, no floats
crates/net/     Peer trait, Loopback, wire format, the star lockstep
crates/net-web/ Peer over web/quad_rtc.js — wasm32 only, currently EMPTY
crates/gui/     macroquad: renderer, panel, score screen; no input yet
web/            index.html, quad_rtc.js (a stub), config.js
packaging/      package-web.sh
```

`sim` is 6 400 lines and its tests are 3 000. `net` is 400 lines and its tests
are 540. That ratio is deliberate and is most of why the thing works.

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

---

## What to do next, in order

### 1. Phase 4 — `quad_rtc.js` and `net-web`

The plan's own checklist is in `floodline-mvp-plan.md`. Two things it says that
are worth repeating here because they are easy to skip:

**Write the message sequence down before writing the plugin.** The design's
§9.2 lists the imports; write out in `DECISIONS.md` exactly what two peers
exchange, in order, for both paths — trystero and pasted code — and implement
to that. This is the riskiest phase in the project and the one where a
half-understood handshake costs a day.

**Vendor trystero, pinned by filename and sha256**, into `web/vendor/`, and
record the version and hash in `DECISIONS.md`. No npm at build time; the same
rule the workbench and redactor follow. `packaging/package-web.sh` copies
`web/` wholesale, so a new file there is picked up with no script change — but
add the version check to the script the way it already checks macroquad's and
sapp-jsutils'.

Note `net-web` and `net-native` were **not** the same thing: `net-native` and
the `bot` crate were deleted when the project moved to v2 (see DECISIONS).
`net::Loopback` does that job now, in-process and inside `cargo test`.

**A trap specific to this phase.** `web/quad_rtc.js` today is a stub that does
one real thing: it hands the wasm its own build hash through `sapp-jsutils`.
That was deliberate — it proves the bridge phase 4 rests on. It also exports
`quad_rtc_crate_version()` from `crates/gui/src/buildid.rs`, and miniquad
compares that against the `version` in the plugin's `miniquad_add_plugin` call.
Bump both together or the console tells you they have drifted.

### 2. Phase 5 — the rest of `gui`

Input is the whole of what is missing, and there is one trap in it, written up
in the plan's phase 5 checklist and in `DECISIONS.md`:
**`screen::Viewport` is the only thing allowed to convert between the screen
and the map.** It carries a `dpi` because `Camera2D::viewport` is in framebuffer
pixels while `screen_width()` and `mouse_position()` are in logical ones. The
drawing half of that was got wrong once already — the deployed game rendered
into the bottom-left quarter of the window on any retina display — and the
input half is where it will be got wrong again, because there the ratio must
*not* be applied. **Check anything that touches it at a device pixel ratio of
1 and 2.**

`crates/gui/src/draw.rs` already has `map_rect()` and `cell_at()` waiting for
you, marked `#[allow(dead_code)]`.

### 3. Phase 6 — hardening

Small, and mostly about failure messages that say what to do. There is nothing
to deploy but static files; if GitHub Pages is up, the game is up.

---

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

## How the tests are meant to be used

* `make test` — everything, ~12s. Must be green at every commit.
* `cargo test -p sim --test scenario` — the game: two cities, a road, three
  days of trade, and a flood a dike survives.
* `cargo test -p net --test lockstep` — the networking, all of it, no browser.
* `cargo test -p sim --release --test profile -- --ignored --nocapture` —
  a tick at 500 citizens with the flood running. 0.36 ms against a 20 ms budget.
* `cargo test -p sim probe -- --ignored --nocapture` — the terrain sweep. A
  measurement, not an assertion.

`sim` is built with `opt-level = 2` under test. Without it the flood tests take
two minutes, and a determinism test nobody runs is worse than none.

## Verifying the browser build

There is no browser tooling in the repo, deliberately, but this is what the last
session used and phase 4 will need something like it:

```
python3 -m venv .venv-test && ./.venv-test/bin/pip install playwright
./.venv-test/bin/playwright install chromium
make web && (cd dist/web && python3 -m http.server 8123 &)
```

then drive it with Playwright, **at a device pixel ratio of 2 as well as 1**,
and read `page.on("pageerror")` — that is how the `register_plugin` error and
the letterbox bug were both found, and neither was visible any other way.

## What has never been tested

Honesty about the edges:

* **Two real browsers have never talked to each other.** All of the networking
  is proven against `Loopback`. That is phase 4's whole job.
* **Nobody has played it.** There is no input. The flood has never been tuned
  against a person's judgement, only against measurements — design step 7
  ("playtest the flood until it is fun") has not begun.
* **`net-web` is an empty crate.**
* Ages 4+ exist in the escalation table and are unreachable: the MVP stops at 3.
