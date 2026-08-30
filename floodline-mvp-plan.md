# FLOODLINE — MVP build plan for Claude Code

This is the working plan for an agent building the minimum viable product. It
sits next to `floodline-design.md` (the design) and `CLAUDE.md` (the rules).
Read all three before writing code. The design says *what*; this says *in what
order and how you know each step is done*.

## What "MVP" means here

A stranger with the GitHub Pages link and a room code can, with one friend:

1. found a city each on a shared map, with a Hearth and eight citizens;
2. build cottages, farms and a granary, assign citizens, watch them walk, eat and sleep;
3. lay a road to the other city and set up one barter trade that haulers carry;
4. build a dike;
5. survive — or not — an age-1 flood that comes out of the low corner;
6. see the score screen, and start a new run with a new seed;
7. never see a desync, and see a clear banner if one happens anyway.

Out of scope for MVP, deliberately: families and children, friendships,
Guildhall specialisation, Tavern, Watchtower, Lend, debris physics, any
disaster after the flood, chat, spectating, host migration. Each is a later
milestone; none is needed to know whether the flood is fun.

Ages 1–3 exist (flood, escalating height); the run ends when both cities fall
or after age 3, whichever first.

## Ground rules (also in CLAUDE.md — repeated because they are the plan)

* **`sim` is pure.** Dependencies: `serde` only. No `f32`/`f64`, no `HashMap`,
  no `std::time`, no `rand`. One `Rng` in `World`. Iteration order is always
  defined. If you need to break one of these, stop and say so.
* **Commands are the only door.** `gui` and `net` change the world only by
  handing `Command`s to the lockstep. `World::apply(player, cmd)` validates,
  including ownership.
* **The determinism test runs on every change** to `sim`: two worlds, same
  seed, same command script, `checksum()` equal for 10 000 ticks. If it fails,
  nothing else is worked on until it passes.
* **Work in the dependency order of the crates.** `sim` → `net` → transports →
  `gui`. Do not start `gui` polish while `net` is unproven.
* **Riskiest thing first.** The JS WebRTC plugin (phase 3) is done before the
  game has a second building type.
* **Small commits, each runnable.** Message in the imperative, body says why.
  `make test` passes at every commit. No commit that only says "WIP".
* **Copy the discipline, not the code, from the reference repos.** Clone
  `sgilson7/gear-master`, `sgilson7/perturbation-workbench`,
  `sgilson7/pdf-redactor` and `johanhelsing/matchbox` into `reference/`
  (gitignored). Take: `Rng`, the fixed logical canvas and letterbox code, the
  `Makefile` and `packaging/package-web.sh` conventions, the panic hook, the
  cache-busting stamp, the workspace `[profile.test]`. Do not copy the 13 000
  line `main.rs`.
* **Ask before deciding** anything §11 of the design leaves open. Otherwise
  decide, write the decision in `DECISIONS.md` with one paragraph of why, and
  keep going.
* **No third-party physics, ECS or networking framework** beyond what the
  plan names: `serde`, `postcard`, `macroquad`, `sapp-jsutils`,
  `matchbox_socket` (native only), `matchbox_server` (as a dependency of
  nothing — it is deployed, not linked).

## Phases

Each phase ends with a demo you can run and a checklist that is literally
checked. Estimated effort is in agent-sessions, not hours.

### Phase 0 — Repository and pipeline (½ session)

* Workspace `floodline/` with the crate layout from design §7 (empty crates
  compile). `rust-version = "1.88"`, same `[profile.release]` and
  `[profile.test]` as the workbench.
* `Makefile`: `make`, `make test`, `make check`, `make web`, `make serve`,
  `make publish`, `make signal`, `make bot`, `make help`.
* `packaging/package-web.sh` adapted from gear-master: builds `gui` for
  `wasm32-unknown-unknown`, copies `mq_js_bundle.js` and `sapp_jsutils.js`
  from the pinned crate versions, copies `web/quad_rtc.js` and
  `web/config.js`, stamps the wasm sha256 into `index.html` and exports it as
  `window.FLOODLINE_BUILD`.
* `.github/workflows/pages.yml`: on push to `main`, run `make test`, then
  `make web`, then deploy `dist/web/` to Pages with `actions/deploy-pages`.
  (Keep `make publish` too, for the docs/ route gear-master uses; the workflow
  is the one that should be used.)
* `CLAUDE.md`, `DECISIONS.md`, `.gitignore` (`target/`, `dist/`, `reference/`).

**Done when:** `make test` is green with zero tests, `make web` produces a page
that shows a macroquad canvas with the build hash in the corner, and the Pages
workflow deploys it.

### Phase 1 — `sim`: land, citizens, buildings (2 sessions)

Implement, in this order, with a unit test per item:

1. `Fx` fixed-point (8 fractional bits), `Rng` (xorshift64*, from gear-master),
   `checksum()` (FNV-1a over the `postcard` encoding of `World` — simple, and
   any float or map-order bug shows up as a mismatch).
2. Map generation from a seed: 128 × 128 heightmap (value-noise with integer
   lerp), a low corner, a high corner, shallows band, rock; one Hearth site per
   player, 2–6 players, sites at least 40 cells apart.
3. `Citizen { id, owner, pos, vel, home, job, food, rest, state }`; needs decay;
   death by starvation.
4. Buildings: Hearth, Cottage, Farm, Granary, Stockpile, Dike, Road, Bridge.
   Placement rules (footprint free, on ground, Dike/Bridge conditions).
   Construction: materials hauled, builder-ticks applied.
5. Flow fields per building; walking; roads double speed.
6. Jobs: Hauler, Farmer, Builder. Farms produce food into the nearest Granary
   via haulers; citizens eat at a Granary and sleep at their Cottage.
7. `Command` (design §7, minus `Lend`), `World::apply(player, cmd)` with
   ownership checks, `World::tick(&[(PlayerId, Command)])`.
8. Roads between cities: `Road` pathing, `AcceptRoad`, joined roads.
   `Trade` / `AcceptTrade`: once per day, haulers carry the agreed goods along
   the joined road.
9. Ages: day counter, age timer, the age-start disaster draw (only Flood),
   warning day, score.
10. `tests/determinism.rs`: the 10 000-tick two-world test with a scripted
    command stream covering every `Command` variant.
11. `tests/boundary.rs`: asserts `sim`'s dependency list is exactly `serde`
    and `postcard`, by reading `Cargo.toml`.

**Done when:** a scripted two-player game (`cargo test -p sim --test scenario`)
founds two cities, builds a road, trades food for wood for three days, and the
determinism test passes.

### Phase 2 — `sim`: water and bodies (1½ sessions)

1. Shallow-water automaton (design §3.4): `depth`, `flow`, transfer rule with
   per-tick cap, drain off map edges. Test: volume conserved except at edges;
   a puddle on flat ground spreads symmetrically; water behind a level-2 dike
   stays behind it for a height-12 surge.
2. The surge (design §5): corner selection, `SURGE_TICKS` injection with
   centre-pointing flow. Test: front reaches the map centre within N ticks.
3. Bodies: citizens gain `flow * drag`, wade/swim/drown thresholds, rooftop
   survival, road cells break under flow above a threshold, wooden buildings
   take damage from flow, stone resists, rubble refunds materials.
   Test: a citizen on open lowland in a height-18 surge ends at the far edge
   or dead; a citizen on the high corner is untouched.
4. Profile `tick()` at 500 citizens with the flood running in release mode.
   Target: < 20 ms on native, so there is headroom for wasm. If the automaton
   is the cost, run it every other tick (record that in `DECISIONS.md`).

**Done when:** the scenario test's cities survive an age-1 flood only if a
dike was built, and the determinism test still passes with the flood in the
script.

### Phase 3 — `quad_rtc.js` and the two transports (1½ sessions) — **do this before phase 4**

1. `net::Peer` trait: `join(room_url, players)`, `poll() -> Option<Event>`,
   `send(peer, bytes, reliable)`, `local_id()`, `peers()`.
2. `web/quad_rtc.js`: miniquad plugin implementing matchbox's signaling
   protocol (design §9.1–9.2). One `RTCPeerConnection` per peer, reliable and
   unreliable data channels, lower-id offers, ICE relayed as it arrives,
   `KeepAlive` every 10 s, incoming bytes queued for `rtc_poll`. Byte and
   string transfer through `sapp-jsutils`. Reconnect is *not* in MVP: a
   dropped signaling socket after all peers are connected is fine; a dropped
   peer is `Event::Left`.
3. `net-web`: the `Peer` impl over the plugin.
4. `net-native`: the `Peer` impl over `matchbox_socket` 0.14 with the same
   two channels.
5. `bot` crate: joins a room natively, prints events, echoes bytes.
6. A `web/echo.html` test page (not published) that loads a tiny macroquad
   binary using only `net-web`, joins `?room=`, and shows peers and round-trip
   time on screen.

**Done when:** two browser tabs and one `make bot` process, all in the same
room on a local `matchbox_server`, exchange bytes on both channels, and a
closed tab produces `Left` on the others within a few seconds. Then repeat
with the signaling server on Fly.io over `wss://` and the tabs on two
different networks. Screenshot or log both in `DECISIONS.md`.

### Phase 4 — Lockstep (1 session)

1. Wire format (design §8): `Hello`, `Welcome`, `Turn`, `Bye`; `postcard`.
2. `net::Lockstep`: input delay 3 ticks, advance only when every live peer's
   `Turn` is held, checksum comparison, "waiting on" status, drop after 30 s
   (as a `Command::Drop` agreed by the others, so it is deterministic).
3. Host = first `IdAssigned` with no peers present; host answers `Hello` with
   `Welcome { seed, tick, snapshot }`. Build hash check refuses mismatches.
4. Late join via snapshot (MVP: only before age 1 starts; mid-age join is
   later).
5. `bot` gains a script mode: it plays a scenario from a file so a real
   browser can be tested against a deterministic partner.

**Done when:** browser tab + bot play a scripted two-player game; the tab's
checksums match the bot's every tick; killing the bot shows the banner; a
deliberately introduced `f32` in `sim` is caught by the desync banner in a
mixed native/wasm session (then reverted — keep that as a documented
experiment, it proves the guard works).

### Phase 5 — `gui` (2 sessions)

Fixed 1600 × 980 logical canvas with a 366 px side panel, letterboxed like
gear-master. Map cells at 8 px. Everything drawn with primitives.

* Map: terrain shaded by height, water as blue with alpha from depth, roads,
  building rectangles with a glyph, citizens as a circle with two lines for
  legs, owner colour ring when selected.
* Input: click/drag select own citizens; right-click a building to assign,
  right-click ground to `MoveTo`; build menu in the panel; road tool
  (click from, click to); trade dialog (with, give, take); accept buttons
  when the other player proposes.
* Panel: population, food/wood/stone, age and day, warning line, peers and
  their status, build hash.
* Lobby: enter or generate a room code, `?room=` in the URL, "Start" when
  2–6 players are present (host only); config-driven signaling URL and ICE
  list from `web/config.js`.
* Score screen: ages survived, peak population, seed, "New run".
* Single player is the lockstep with one peer, so there is no separate path.

**Done when:** two people on two machines play a full run to age 3 on the
Pages build with the Fly.io signaling server, and the design's step-7 test
("playtest the flood until it is fun") has been run at least three times with
notes in `DECISIONS.md` about what was tuned.

### Phase 6 — Deployment hardening (½ session)

* Fly.io: `deploy/fly.toml` and a `deploy/README.md` with the exact commands
  used, TLS on, `?next=` honoured.
* TURN: `web/config.js` carries an ICE list; document adding coturn or a
  metered.ca entry. Not required to pass, required to be one edit away.
* Pages: confirm the build hash namespacing of rooms (`/<hash>/<room>`) works
  end-to-end so a stale tab cannot join a new build's room.
* `README.md`: play link, how to host a game, how to run locally, how to run a
  bot, the determinism rules in one paragraph.

**Done when:** a fresh clone can `make serve` and reach the deployed signaling
server, and the README's play instructions were followed by someone who did
not write them.

## Later milestones (not MVP)

M2 families and children · M3 Guildhall and skilled trades · M4 friendships,
Tavern, mood · M5 Watchtower and early warning · M6 debris physics ·
M7 second disaster (fire) · M8 Lend, chat, spectating, host migration,
mid-age join · M9 map variety and the age-4+ table.

## How to report progress

At the end of each session, append to `PROGRESS.md`: phase, what is done
against the checklist, what is not, what was decided, what is blocked, and the
single next action. Keep it short enough to read in one minute.
