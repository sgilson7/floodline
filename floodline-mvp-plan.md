# FLOODLINE — MVP build plan for Claude Code (v2-noserver)

This is the working plan for an agent building the minimum viable product. It
sits next to `floodline-design-v2-noserver.md` (the design) and `CLAUDE.md`
(the rules). v2 means: no server of ours. The host's browser is the hub, peers
reach it through public signalling (Trystero) or a pasted code, and the only
thing deployed is static files on GitHub Pages.
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
7. never see a desync, and see a clear banner if one happens anyway;
8. do all of this with nothing running anywhere except GitHub Pages — both the
   Trystero path and the pasted-code path work.

Out of scope for MVP, deliberately: families and children, friendships,
Guildhall specialisation, Tavern, Watchtower, Lend, debris physics, any
disaster after the flood, chat, spectating, host migration, TURN. Each is a later
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
* **Riskiest thing first.** The JS WebRTC plugin (phase 4) is done before the
  game has a second building type. Lockstep (phase 3) is proven on the
  in-process loopback before the plugin exists, so the two can never be
  confused.
* **Small commits, each runnable.** Message in the imperative, body says why.
  `make test` passes at every commit. No commit that only says "WIP".
* **Copy the discipline, not the code, from the reference repos.** Clone
  `sgilson7/gear-master`, `sgilson7/perturbation-workbench`,
  `sgilson7/pdf-redactor` and `dmotz/trystero` into `reference/`
  (gitignored). Take: `Rng`, the fixed logical canvas and letterbox code, the
  `Makefile` and `packaging/package-web.sh` conventions, the panic hook, the
  cache-busting stamp, the workspace `[profile.test]`. Do not copy the 13 000
  line `main.rs`.
* **Ask before deciding** anything §11 of the design leaves open. Otherwise
  decide, write the decision in `DECISIONS.md` with one paragraph of why, and
  keep going.
* **No third-party physics, ECS or networking framework** beyond what the
  plan names: `serde`, `postcard`, `macroquad`, `sapp-jsutils` on the Rust
  side; a pinned, vendored copy of `trystero`'s browser bundle on the JS side.
  No npm at build time, no server code of any kind.

## Phases

Each phase ends with a demo you can run and a checklist that is literally
checked. Estimated effort is in agent-sessions, not hours.

### Phase 0 — Repository and pipeline (½ session)

* Workspace `floodline/` with the crate layout from design §7 — `sim`,
  `net`, `net-web`, `gui` — empty crates compile. `rust-version = "1.88"`, same `[profile.release]` and
  `[profile.test]` as the workbench.
* `Makefile`: `make`, `make test`, `make check`, `make web`, `make serve`,
  `make publish`, `make help`. (No `make signal`, no `make bot`: there is
  nothing to run but the game.)
* `packaging/package-web.sh` adapted from gear-master: builds `gui` for
  `wasm32-unknown-unknown`, copies `mq_js_bundle.js` and `sapp_jsutils.js`
  from the pinned crate versions, copies `web/quad_rtc.js`,
  `web/config.js` and `web/vendor/trystero-<ver>.js` (pinned by filename and
  sha256), stamps the wasm sha256 into `index.html` and exports it as
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

### Phase 3 — Lockstep on loopback (1 session)

1. `net::Peer` trait: `poll() -> Option<Event>`, `send(peer, bytes, reliable)`,
   `peers()`, `is_host()`. Events: `Peer(id)`, `Left(id)`, `Msg{id, reliable,
   bytes}`, `Error(text)`.
2. `net::Loopback`: N in-process peers in a star, with configurable one-way
   latency (in ticks) and unreliable-channel loss. Deterministic itself (uses
   its own seeded rng for loss) so tests are repeatable.
3. Wire format (design §8): `Hello`, `Welcome`, `Roster`, `Turn`, `Bye`;
   `postcard`.
4. `net::Lockstep`: host collects one `Turn` per player per tick, emits the
   bundle, all peers advance on the bundle; input delay 3; checksum
   comparison at the host; "waiting on" status; `Drop` after 30 s of silence,
   emitted by the host as a command so it is deterministic.
5. Host answers `Hello` with `Welcome { player_id, seed, tick, snapshot }`;
   refuses mismatched `build_hash`. Late join via snapshot (MVP: only before
   age 1 starts).
6. Tests, all in `cargo test -p net`: three loopback players play the phase-1
   scenario to age 2 with 200 ms latency and identical checksums; a player
   whose `sim` is deliberately fed an extra command desyncs and everyone
   freezes at the same tick; a silent player is dropped at the same tick on
   every peer; a fourth player joins before age 1 from a snapshot and stays
   in sync.

**Done when:** those tests pass and the native `gui` (phase 5 stub is fine —
a blank canvas that logs ticks) runs a two-player loopback game in one window.

### Phase 4 — `quad_rtc.js` and `net-web` (1½ sessions) — **the risky one**

1. Vendor `trystero`'s browser bundle into `web/vendor/`; record version and
   sha256 in `DECISIONS.md`. Load it with a script tag in `index.html`.
2. `web/quad_rtc.js` as a miniquad plugin (design §9.2) with both modes:
   * **trystero**: host and joiners `joinRoom({appId, password, rtcConfig},
     "<build_hash>/<room>")`; the host accepts every `onPeerJoin`, joiners
     accept only the first peer (the host) and ignore others. Two data
     channels attached per connection.
   * **code**: host builds an `RTCPeerConnection`, creates both channels,
     waits for ICE gathering to complete, exposes a compressed base64 offer;
     joiner consumes it and produces an answer blob; host applies it.
     Compression: drop inferable SDP lines, `CompressionStream("deflate-raw")`,
     base64url. Target: an offer under 600 characters.
   Incoming bytes queued for `rtc_poll`; byte/string transfer via
   `sapp-jsutils`. Reconnect is not in MVP.
3. `net-web`: the `Peer` impl over the plugin, including star semantics
   (joiners see exactly one peer).
4. `web/echo.html` (not published): loads a tiny macroquad binary using
   `net-web` only; Host / Join by room / Host by code / Join by code
   buttons; shows peers, round trip and bytes per second on screen.
5. Write in `DECISIONS.md`, *before* coding step 2, the exact sequence of
   events you expect for one host and two joiners in each mode, by reading
   trystero's source and MDN. Implement to that; update it if reality differs.

**Done when:** on two machines on two different home networks, via the Pages
build of `echo.html`: (a) trystero mode connects two joiners to a host and
bytes flow on both channels, (b) code mode connects with one pasted exchange,
(c) closing a joiner's tab produces `Left` at the host within 10 s, (d) a
mismatched `build_hash` is refused. Log the measured join time for the
trystero strategy used; if Nostr is slow from either network, try MQTT and
record which one is the default. Then swap `Loopback` for `net-web` under
the phase-3 lockstep and play the phase-1 scenario in two tabs.

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
* Lobby: *Host* (generates a room code, shows it and a shareable
  `?room=` link), *Join* (room code field, filled from the URL), and a
  *by code* toggle that shows the paste boxes instead; "Start" when 2–6
  players are present, host only; strategy, appId and ICE list from
  `web/config.js`.
* Score screen: ages survived, peak population, seed, "New run".
* Single player is the lockstep with one peer, so there is no separate path.

**Done when:** two people on two machines play a full run to age 3 on the
Pages build with nothing else running anywhere, and the design's step-7 test
("playtest the flood until it is fun") has been run at least three times with
notes in `DECISIONS.md` about what was tuned.

### Phase 6 — Hardening the serverless deployment (½ session)

* Failure messages that say what to do: "no peers found on the relays —
  try *Join by code*", "direct connection failed — one of you may be behind
  a strict NAT; try another network or add a TURN server in config.js".
* `web/config.js` carries `rtcConfig.iceServers` with public STUN and a
  commented example of a free-tier TURN entry; `README.md` explains when it
  is needed and that it is the only thing in the game that can cost money.
* Strategy fallback: if Trystero reports no relay connection within 15 s,
  the lobby offers *by code* automatically.
* Confirm the `build_hash` prefix on room names keeps a stale tab out of a
  newer build's game, end-to-end on Pages.
* `README.md`: play link, how to host a game (both ways), how to run
  locally, the determinism rules in one paragraph, and one sentence stating
  that the project runs no servers.

**Done when:** a fresh clone can `make serve` and host a game reachable from
another machine, and the README's play instructions were followed by someone
who did not write them.

## Later milestones (not MVP)

M2 families and children · M3 Guildhall and skilled trades · M4 friendships,
Tavern, mood · M5 Watchtower and early warning · M6 debris physics ·
M7 second disaster (fire) · M8 Lend, chat, spectating, host migration (re-signal to the new host),
mid-age join, optional TURN · M9 map variety and the age-4+ table.

## How to report progress

At the end of each session, append to `PROGRESS.md`: phase, what is done
against the checklist, what is not, what was decided, what is blocked, and the
single next action. Keep it short enough to read in one minute.
