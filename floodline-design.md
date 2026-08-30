# FLOODLINE — design and multiplayer plan

*v2-noserver. A multiplayer medieval city builder — one map, one city per player, roads and trade between them — where every age ends in a
catastrophe, played in the browser, built on the gear-master / workbench /
redactor toolchain.*

Draft 3 (v2-noserver) — separate cities on a shared map, roads and trade; no server of ours anywhere: the host's browser is the hub and peers find it through public signaling or a pasted code. For review. Everything below is a proposal; the sections marked
**Decision** are the ones that lock in architecture and are expensive to change
later.

---

## 1. The pitch

Two to six players share one map, each founding their own city on it.
Citizens are stick figures who need food, sleep and company, have to be sent to
jobs, form families and friendships, and walk everywhere they go. Cities are
linked by roads the players lay between them, and along those roads goods move
under standing trade agreements — so a flood that drowns one city's farms is
everyone's problem. At the end of
each **age** a disaster strikes — the first one is a wall of water that comes
out of one corner of the map and sweeps across it. Whatever survives is what
you rebuild from. The next disaster is worse. A run ends when every city is gone; the score is
how many ages the map stood, and each player's own city is scored alongside.

Reference points: StarCraft (select units, send them to work), Black & White 2
(a village that grows and has a personality), roguelikes (seeded runs, fresh
start every time, escalating difficulty), Dwarf Fortress (losing is fun).

Visual target: dots and stick figures, flat colour, no sprites. The map is a
grid; buildings are rectangles with a glyph; citizens are a circle with legs;
water is a blue layer whose opacity is depth. Everything can be drawn with
`draw_circle` / `draw_rectangle` / `draw_line` in macroquad, which is the point:
all the effort goes into simulation and networking.

---

## 2. What carries over from the three repos

| From | Take | Use as |
|---|---|---|
| gear-master `engine` | zero-dependency rules crate, own xorshift RNG, "deterministic and simulated up front" | `sim` crate |
| gear-master `console` | `Verb` enum + `menu / apply / view` | `Command` enum, the *only* way to change the world |
| gear-master `gui` | macroquad, fixed logical canvas, letterboxing, `mq_js_bundle.js` loader | `gui` crate, unchanged pipeline |
| gear-master `agent` / `cli` | headless play through the console | headless bot peers for testing lockstep without opening six browser tabs |
| workbench / redactor | state lives in Rust, JS renders a computed view, panic hook, hash-stamped cache busting, version-pinned toolchain in `package-web.sh` | build scripts and the discipline |
| all three | `docs/` → GitHub Pages | hosting the game client |

Nothing in any of the three does networking. That is the new work, and it is
described in §7–§9.

---

## 3. Simulation model

**Decision: deterministic lockstep, integer maths, no external physics crate.**

Every peer runs the same simulation from the same seed and applies the same
commands on the same tick. Only commands go over the wire. This is what lets
six browsers share thousands of citizens and a flood on a few kilobytes a
second, and it is what forces every rule below.

### 3.1 Units and time

* **Tick:** 10 per second. Rendering runs at display rate and interpolates
  positions between the last two ticks.
* **Fixed point:** positions and velocities are `i32` in 1/256 of a cell
  (`Fx = i32`, 8 fractional bits). No `f32` anywhere in `sim`. Trigonometry
  is a 256-entry lookup table; square roots are integer Newton steps.
* **Map:** 128 × 128 cells to start. Each cell has terrain height (`u8`, 0–255),
  a ground type (grass, rock, sand, shallows), an optional building footprint,
  and a water depth (`u16`). Rendered at 8 logical pixels per cell, so the whole
  map fits gear-master's 1600 × 980 canvas with a side panel.
* **Rng:** one `Rng` in `World`, seeded by the host. Nothing else may
  generate randomness. Iteration over any collection that feeds a decision
  must be ordered (`Vec` or `BTreeMap`; never `HashMap`).

### 3.2 Citizens

A citizen is a small struct: id, position, velocity, home, job, needs, family,
friends, state machine.

**Needs**, each 0–1000, decaying per tick:

| need | filled by | when it hits zero |
|---|---|---|
| food | eating at a granary or home larder | starvation timer starts; death after 3 days |
| rest | sleeping in an assigned bed | works at half speed, wanders |
| company | standing near friends or family (tavern, hearth, home) | mood drops, may refuse jobs |

**Jobs** are assigned StarCraft-style: drag-select citizens, right-click a
building. A building has job slots; assigning fills one. Unassigned citizens
idle near their homes. Jobs:

* *Hauler* — carries goods between stockpiles (default job for anyone unskilled).
* *Farmer, Fisher, Forester, Quarrier* — produce food / wood / stone at their building.
* *Builder* — walks to construction sites and converts delivered materials into progress.
* *Mason* — builds and repairs dikes and walls (the flood counter-play).
* *Watcher* — staffs a watchtower; gives the village earlier warning of the disaster.

**Specialisation** happens at the Guildhall: a citizen assigned there for one
in-game day emerges with a trade tag (`Skilled(Farmer)` etc.) that doubles
output at that job. Specialisation is the thing you lose when a citizen drowns.

**Families and friends.** Two adult citizens sharing a cottage for a day become
a *household*; households with a full larder and spare bed produce a child at
the Hearth (see §4). Citizens accumulate *affinity* with whoever they stand
near while satisfying needs; affinity above a threshold is a friendship.
Friends fill each other's company need faster and, importantly, a citizen whose
friend dies loses mood for an age. This is the only "personality" system in
version one and it is enough to make losses hurt.

**Movement.** Citizens walk on the cell grid using a *flow field* per
destination building (Dijkstra from the building outward, recomputed only when
terrain or footprints change), so five hundred citizens heading to the granary
cost one field, not five hundred A* searches. Between cells they move with
fixed-point velocity, and this is what the physics layer pushes on when the
water hits.

### 3.3 Buildings

| building | cost | does |
|---|---|---|
| Hearth | free, one per village | spawns the founding citizens; households bring children here to be born |
| Cottage | wood | 4 beds, 1 larder; makes a household possible |
| Farm / Fishery / Forester's hut / Quarry | wood, stone | job slots that produce food / wood / stone |
| Granary | wood | food stockpile; citizens eat here |
| Stockpile | free | wood and stone storage |
| Guildhall | wood, stone | specialises citizens |
| Tavern | wood | fast company; friendships form here |
| Watchtower | stone | earlier disaster warning, on high ground only |
| Dike | stone, mason-built | raises effective terrain height by 3 per level; the flood answer |
| Bridge | wood | crosses shallows |

Buildings are placed by any player; construction needs materials hauled to the
site and builder-ticks spent on it. Damage is a number; a building at zero is
rubble, and rubble returns a fraction of its materials.

### 3.4 Physics

**Decision: a purpose-built fixed-point physics layer inside `sim`, not Rapier.**

Rapier is excellent but it is floating point; its `enhanced-determinism`
feature makes runs reproducible across *platforms* for the same build, which is
enough for wasm-only lockstep but breaks the moment a native headless peer
joins, and it drags a large dependency into a crate whose whole value is being
dependency-free. Stick figures and water do not need rigid-body contact
resolution. They need three things, and each is small:

**Water — a shallow-water cellular automaton.** Each cell holds `depth` and
`flow (vx, vy)`. Each tick, for each cell, water surface = terrain + dike +
depth; water moves to lower neighbours proportionally to the surface
difference, capped by a per-tick transfer limit; flow is the net movement.
This is the classic height-field method and it is fully integer. It gives you
water that pools in valleys, backs up behind dikes, spills over when the level
exceeds them, and drains off the map edges.

**Bodies — kinematic points.** A citizen is a point with velocity. Each tick:
`velocity += water_flow(cell) * drag_factor`, then position advances, then
collision against building footprints and rock resolves by pushing the point
out along the shallowest axis. A citizen in water deeper than `WADE` moves at
half speed; deeper than `SWIM` is swept (loses control, follows flow); deeper
than `SWIM` for more than `DROWN_TICKS` dies. Debris — loose logs and stones
from smashed buildings — use the same point model with more mass and knock
citizens over on contact.

**Buildings — damage from flow.** Each tick a building takes damage
proportional to the flow magnitude across its footprint cells minus its
material resistance. Stone resists more than wood; a building behind a dike
sees zero flow and takes nothing. When it breaks, it spawns debris.

That is the whole engine: ~600 lines, testable with `cargo test`, and the flood
in §5 falls out of it.

---

## 4. The run

**Roguelike loop.** Start a run with a seed → play ages → the village dies →
score → new seed. No persistent unlocks in version one, because they undercut
"start over every time"; if the game needs meta-progression later it should be
cosmetic or variety (new map types), not power.

**A run begins** with the Hearth placed by the host, a starting party of eight
citizens (four households), a stockpile of wood, and the first age's timer
running. The map is generated from the seed: a heightmap with a low corner,
a high corner, a river of shallows, and scattered rock.

**An age** is a fixed length of in-game days (say 6, about 12 real minutes at
10 ticks/s with 200 ticks per day). Its shape:

1. *Warning.* The disaster is decided at age start (from the seed) but not
   shown. A watchtower reveals it — which corner, how bad — some days early;
   without one, the village gets one day's notice from the Hearth ("the elders
   are uneasy").
2. *Preparation.* Build dikes, move stockpiles uphill, pull citizens off
   distant farms.
3. *Impact.* The disaster runs for one day. Commands still work — you can
   order citizens uphill mid-flood — which is where the tension is.
4. *Aftermath.* Survivors, rubble, drowned specialists, broken households. The
   next age begins immediately.

**Escalation.** Each age's disaster draws from a table keyed by age number.
Version one ships only the flood, at growing intensity:

| age | disaster | intensity |
|---|---|---|
| 1 | flood from the low corner | surge height 12, one corner |
| 2 | flood | surge height 18 |
| 3 | flood, two corners | height 18 each, offset by half a day |
| 4 | flood, random corner (can be the high one) | height 24 |
| 5+ | height +6 per age; from age 7 the river bursts too | |

The table is data. Later disasters — fire that spreads cell to cell, plague
that travels along the friendship graph, an earthquake that drops terrain —
each reuse a piece of §3 (fire is the water automaton with different rules;
plague is the affinity system used against you).

**Losing.** The run ends when no citizens are alive. It is allowed to be
sudden. The score screen shows ages survived, peak population, and the seed,
so the same map can be replayed.

---

## 5. The flood, in detail

The first disaster is the vertical slice; if it is fun, the game is.

1. At age start `World.rng` picks the source corner (age 1–3: always the lowest
   corner, so the first floods are learnable) and the surge height `H`.
2. On impact tick, the automaton begins injecting water: for `SURGE_TICKS`
   (about 30 seconds), every tick sets `depth = H` on the 8 × 8 cells of the
   source corner and gives them flow pointing toward the map centre. This is
   the "wall of water" — not a scripted wave but a source strong enough that
   the automaton produces a front.
3. The front advances at roughly one cell per tick, slows as it spreads, pools
   in low ground, and stacks up against dikes. A dike two levels high stops
   an age-1 flood dead; the water goes around, which is the teaching moment.
4. Citizens in its path get `velocity += flow`; those on farms in the lowlands
   are swept toward the far edge. Anyone who reaches high ground or a rooftop
   (buildings above `depth + 2` are climbable) survives.
5. Wooden buildings in the main flow break within a few seconds and shed logs
   that travel with the water and hit other buildings.
6. After `SURGE_TICKS` the source stops. Water drains off the edges over the
   next day; the low ground stays as shallows for the rest of the age (farms
   there are lost until it dries).

Player counter-play, in order of sophistication: build on the high corner;
build dikes; keep a watchtower staffed; keep a spare granary uphill; keep
households together so survivors don't lose company on top of everything.

---

## 6. Multiplayer design: your city, our map

One map, two to six cities, one player each.

* **Ownership.** Every citizen and building belongs to a player. Only the
  owner may command them. A player's colour is their city's colour.
* **Founding.** The map generator places one Hearth site per player, spread
  around the map with comparable (not identical) ground: each site has some
  high ground and some lowland, but the low corner is nearer to some than
  others. The seed decides; the score screen shows it, so an unfair map can be
  replayed.
* **Roads.** `Command::Road { from, to }` lays a road cell-by-cell along the
  cheapest path between two cells the player owns or that are unowned;
  builders from the ordering city construct it. A road that reaches another
  city's edge is *joined* when that player accepts it (`Command::AcceptRoad`).
  Roads across shallows need bridges. Citizens walk twice as fast on roads,
  and the flood breaks road cells it flows over, which is what makes
  rebuilding the link after an age a decision.
* **Trade.** `Command::Trade { with, give: (Good, u16), take: (Good, u16) }`
  proposes a standing exchange per day; the other player's
  `Command::AcceptTrade` makes it live. Each day, haulers from both cities
  walk the joined road carrying the goods. A hauler that drowns loses the
  cargo. There is no market, no price and no currency in version one: trade is
  barter along a road you can watch.
* **Aid.** `Command::Lend { citizens, to }` sends idle citizens to work at
  another city for one age; they keep their home and walk back.
* Cursors are shared (unreliable channel, 20 Hz, relayed by the host) so
  players can point.
* Ping is a command (`Command::Ping { cell }`) so it lands on the same tick
  for everyone and can be replayed.
* Text chat goes over the unreliable channel and is not part of the sim.
* Pause is a command that needs every player's `Resume` to lift.
* **Losing.** A city with no living citizens is out; its player can spectate
  and still chat and ping. The run ends when the last city falls.

## 7. Crates

```
floodline/
  Cargo.toml                 workspace, same profile settings as gear-master
  Makefile                   make / make test / make web / make serve / make publish
  packaging/package-web.sh   gear-master's, plus copying quad_rtc.js and the vendored trystero build
  docs/                      published web build (GitHub Pages) — the only thing on the internet
  crates/
    sim/        World, Citizen, Building, Water, Command, tick(), checksum()
                deps: serde, postcard. Never macroquad, never a physics crate.
    net/        Peer trait; Star topology (host relays); Lockstep scheduler;
                Welcome/snapshot for joiners; Loopback peer for tests
                deps: sim, serde, postcard
    net-web/    Peer impl over the quad_rtc.js plugin (wasm32 only)
                deps: net, sapp-jsutils
    gui/        macroquad: render, input → Command, lobby, panels
                deps: sim, net, net-web (wasm) — native build uses net::Loopback only
  web/
    quad_rtc.js              the plugin: signaling (trystero or pasted codes) + RTCPeerConnection
    vendor/trystero-*.js     pinned copy of trystero's browser bundle (no npm at build time)
    config.js                ICE servers, trystero strategy and appId
```

There is no server crate and nothing to deploy but static files. Native
builds are for development: the GUI runs against `net::Loopback`, which
hosts N in-process peers in one binary, so lockstep, desync detection and
late-join can all be tested in `cargo test` with no browser and no network.

The dependency rule is the gear-master one, and `tests/boundary.rs` asserts
it: `sim` names no graphics or network crate; `gui` never constructs a
`World` change except by handing a `Command` to the lockstep.

`Command` (first cut):

```rust
pub enum Command {
    Place { kind: BuildingKind, x: u8, y: u8 },
    Demolish { building: BuildingId },
    Assign { citizens: Vec<CitizenId>, building: BuildingId },
    Unassign { citizens: Vec<CitizenId> },
    MoveTo { citizens: Vec<CitizenId>, x: u8, y: u8 },  // "get uphill"
    SetHome { citizens: Vec<CitizenId>, cottage: BuildingId },
    Road { from: (u8, u8), to: (u8, u8) },
    AcceptRoad { road: RoadId },
    Trade { with: PlayerId, give: (Good, u16), take: (Good, u16) },
    AcceptTrade { trade: TradeId },
    Lend { citizens: Vec<CitizenId>, to: PlayerId },
    Ping { x: u8, y: u8 },
    Pause, Resume,
}
```

Every `Turn` carries the sender's `PlayerId`, and every command is validated
inside `sim` (`World::apply(player, cmd)` returns `Result<(), RuleError>`) —
including ownership, so a peer commanding another city's citizens is rejected
identically everywhere, whether by bug, desync or tampering.

---

## 8. Lockstep over a star

**Decision: star topology. Every joiner connects only to the host; the host
relays. No full mesh.**

Reasons: a joiner needs exactly one WebRTC connection, which makes the
pasted-code path a single exchange; the host already owns the seed and the
snapshot; and browsers cap the number of `RTCPeerConnection`s, which mesh hits
first. Cost is one extra hop for non-host players — invisible under a 300 ms
input delay.

Messages on the **reliable, ordered** channel:

```
Hello    { proto_version, build_hash, name, colour }          joiner → host
Welcome  { player_id, seed, tick, snapshot: Option<World>, players }   host → joiner
Roster   { players }                                          host → all, on change
Turn     { player, tick, commands: Vec<Command>, checksum_prev }
                                                              joiner → host; host → all (relayed, plus its own)
Bye      { reason }
```

Messages on the **unreliable** channel: `Cursor { player, x, y }`,
`Chat { player, text }`, relayed by the host the same way.

Rules:

* A command issued locally at tick `T` is scheduled for `T + DELAY`, `DELAY`
  = 3 ticks (300 ms). The host collects one `Turn` per live player per tick,
  and once it has them all it emits the bundle for `T`; everyone (host
  included) advances tick `T` only on that bundle. Nobody simulates ahead;
  nobody rolls back. The host is a relay with a clock, not an authority: it
  runs the same `sim` as everyone else.
* `checksum_prev` is a 64-bit FNV of `World` after tick `T − 1`. The host
  compares all of them; a mismatch freezes the game everywhere with a
  "desync with Alice at tick T" banner. In development the sim also dumps
  both worlds so the divergence can be diffed.
* `build_hash` in `Hello` is the wasm's sha256 (the stamp `package-web.sh`
  already computes). Mismatched builds cannot join.
* **The host is whoever clicked Host.** If the host's tab closes, the game is
  over for MVP; host migration (the lowest surviving `PlayerId` re-hosting and
  everyone reconnecting to it through a fresh signal) is a later milestone.
  Because the sim is identical everywhere, that migration is a reconnect, not
  a state transfer.
* A player whose `Turn` is more than 5 s late is shown as "waiting on …";
  after 30 s the host emits a `Drop` command and continues without them.
  Background tabs keep timers at 1 Hz, so a backgrounded player still sends
  turns, slowly; the 5 s banner is what you will see.
* **Late join:** host serialises `World` with `postcard` (~ 50–150 KB at 500
  citizens), sends `Welcome { snapshot }`, and the joiner starts contributing
  `Turn`s from `tick + DELAY`.

---

## 9. Multiplayer without a server

**Decision: nothing of ours runs anywhere except GitHub Pages. Game traffic is
browser-to-browser over WebRTC. The introduction step uses public decentralised
signalling by default and a pasted code as the fallback.**

### 9.1 What WebRTC needs and who provides it

Two browsers cannot dial each other by address. Before a direct link exists
they must exchange a few kilobytes of connection information (an SDP
offer/answer and ICE candidates) through some third channel — *signalling*.
After that exchange the third channel is idle; the game never touches it
again. Matchbox's server did only this. The v2 design replaces it with two
channels nobody has to operate:

1. **Trystero** (`web/vendor/trystero-*.js`, pinned). A JS library that does
   WebRTC matchmaking over public infrastructure that already exists — Nostr
   relays by default, with MQTT brokers and BitTorrent trackers as
   alternatives — and hands back connected `RTCPeerConnection`s. Your app data
   never touches the signalling medium. The host joins room
   `floodline/<build_hash>/<room_code>` and waits; joiners join the same room
   name and Trystero introduces them. Config lives in `web/config.js`:
   strategy (`nostr`), `appId`, an optional password (which encrypts the
   signalling so an untrusted relay cannot tamper with it), and `rtcConfig`.
2. **Pasted codes.** Host clicks *Host by code*, waits for ICE gathering to
   finish (no trickle), and shows a compressed base64 blob of its offer. The
   joiner pastes it, gets an answer blob, sends it back over whatever chat
   they are already using, host pastes it. One round trip per joiner, thanks
   to the star. This path has no dependency on anyone's relays and is the one
   to reach for when a tracker is down or a network blocks them.

Both paths end in the same object — one `RTCPeerConnection` to the host with
a reliable and an unreliable data channel — so `net-web` cannot tell them
apart.

### 9.2 `web/quad_rtc.js`

A miniquad plugin (registered with `miniquad_add_plugin` before `load()`),
exposing to Rust:

```
rtc_host(room: JsObject, mode: u32)       mode 0 = trystero, 1 = code; returns immediately
rtc_join(room: JsObject, mode: u32)
rtc_code_offer() -> JsObject              host: the offer blob once gathered, else null
rtc_code_answer(blob: JsObject)           host: accept a joiner's answer
rtc_code_accept(blob: JsObject) -> JsObject   joiner: consume offer, produce answer blob
rtc_poll() -> JsObject                    next event or null:
                                          {kind:"peer", id} | {kind:"left", id}
                                          | {kind:"msg", id, reliable, bytes} | {kind:"error", text}
rtc_send(peer: JsObject, reliable: u32, ptr, len)
rtc_close()
```

Inside: per connection two `RTCDataChannel`s (`{ordered:true}` and
`{ordered:false, maxRetransmits:0}`); with Trystero, `room.onPeerJoin` gives
the connection and the plugin attaches its own channels; with codes, the
plugin builds the connection itself, disables trickle by waiting for
`icegatheringstatechange === "complete"`, and compresses the SDP (strip
lines the other side can infer, then deflate via `CompressionStream`,
base64) so the blob fits comfortably in a chat message. Incoming bytes are
queued and drained by `rtc_poll` from the macroquad frame loop. Strings and
byte buffers cross the boundary through `sapp-jsutils`.

Trystero is loaded as a plain script tag from `web/vendor/`, no bundler —
the same "no npm at build time" rule the workbench and redactor follow.
Updating it is copying a new file and bumping the pin in `package-web.sh`.

### 9.3 STUN / TURN — the one thing that is not free

ICE needs STUN to learn public addresses; Google's public servers are fine
and cost nothing. When both players are behind strict NATs, no direct path
exists and only a TURN relay can carry traffic. There is no serverless
substitute for that case. The plan is: ship with STUN only, show a clear
"could not connect directly" message (Trystero reports this via
`onJoinError`; the code path reports it via `iceconnectionstate === failed`),
and keep `rtcConfig.iceServers` in `config.js` so a free-tier hosted TURN
entry (Cloudflare's or metered.ca's) is one edit away for anyone who forks the
game and wants it. For friends on home connections this is the uncommon
case, not the common one.

### 9.4 Rooms

A room is a short code (`brisk-otter-42`) the host shares; the URL
`https://<user>.github.io/floodline/?room=brisk-otter-42` opens the lobby
with it filled in. Trystero room names are prefixed with the build hash, so a
stale tab cannot find a newer build's game. Code-mode games need no room
name at all — the pasted offer is the invitation.

### 9.5 What "publish" means

Unchanged from gear-master: `make web` builds the wasm, copies
`mq_js_bundle.js`, `sapp_jsutils.js`, `quad_rtc.js`, `config.js` and the
vendored trystero file, stamps hashes, writes `dist/web/`; the Pages workflow
(or `make publish` into `docs/`) puts it online. Nothing else exists. If
GitHub Pages is up, the game is up.

### 9.6 Testing without browsers

`net::Loopback` implements `Peer` with in-process queues, optional simulated
latency and message loss, and N players in one binary. The lockstep, desync
banner, drop-after-timeout and late-join logic are all tested there in
`cargo test`. The browser plugin is tested separately with an `echo.html`
page (not published) that only exercises connect/send/receive, so a
networking regression and a lockstep regression can never be confused.

---

## 10. Build order

Each step ends in something runnable.

1. **`sim` skeleton and the determinism test.** Map, terrain, one cottage, one
   farm, citizens with food and rest, flow-field walking, `Command`, `tick`,
   `checksum`. Test: two worlds, same seed and commands, equal checksums for
   10 000 ticks. `cargo test`, no browser.
2. **Water.** The automaton, the corner surge, a dike. Test: water conserves
   volume minus what leaves the edges; a level-2 dike holds a height-12 surge.
3. **Bodies.** Citizens pushed by flow, drowning, debris. Test: a citizen in a
   height-18 flow with no obstacles ends at the far edge.
4. **`net::Loopback` and lockstep.** Star scheduler on top of the `Peer`
   trait, tested with N in-process peers, simulated latency, a forced desync,
   a dropped player and a late join. No browser yet.
5. **`quad_rtc.js` and `net-web`.** Two tabs on two networks, both paths
   (trystero and pasted code), bytes both ways, a closed tab produces `Left`.
   This is the riskiest step; do it before the game has a second building.
6. **`gui`.** Render the map, select and command citizens, place buildings,
   the side panel. Single-player is just lockstep with one peer.
7. **The first age.** Warning, impact, aftermath, score. Playtest the flood
   until it is fun. Nothing else matters until it is.
8. **Content.** Remaining buildings, Guildhall, families, friendships, the
   escalation table. Then the second disaster.

---

## 11. Open questions for review

* Age length and tick rate are guesses; 12 real minutes per age may be too
  slow for a session with friends.
* Should citizens be individually nameable? It costs nothing and might be the
  thing that makes drownings land.
* Trade is barter at fixed daily quantities. If players want haggling, a
  price mechanism comes later; the road is the interesting part.
* Map size vs. citizen count: 128 × 128 and ~500 citizens should be fine in
  wasm at 10 ticks/s, but the water automaton is 16 k cells per tick and the
  first profile will say whether it needs to run at 5 Hz.
* Trystero's default Nostr strategy versus MQTT or BitTorrent for this game:
  pick by measured join time from two home networks, and keep the pasted-code
  path regardless.
* Whether to reuse gear-master's actual `Rng`, fixed-canvas layout code and
  panel style, or start `gui` clean. Reusing saves a week; starting clean
  avoids inheriting a 13 000-line `main.rs`.
