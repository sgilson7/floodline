# Decisions

One entry per choice that would be expensive to reverse or surprising to
rediscover. Newest last. Anything design §11 leaves open is asked, not decided
here.

---

## 2026-08-29 — What each reference repo contributes

The four checkouts in `reference/` were read before the workspace existed:
gear-master at `9857569`, perturbation-workbench at `ea7f450`, pdf-redactor at
`c6eceaf`, matchbox at `2347405`.

**gear-master.** Taken: the shape of `crates/engine/src/rng.rs` almost
verbatim — xorshift64\*, a `state: u64` that refuses to be zero, `below(n)`
returning 0 for `n == 0` rather than dividing by it, and Fisher-Yates
`shuffle` so that drawing without replacement is "shuffle and take". It is
already exactly what design §3.1 asks for and rewriting it would only risk a
worse one. Taken from `crates/console/src/verb.rs`: the discipline rather than
the code — one enum whose variants are the complete set of things a player can
do, a `line()`/`parse()` pair so a game is a replayable transcript, and the
principle that what is deliberately absent from the enum is as much of the
definition as what is present. Our `Command` gets `line()`/`parse()` for the
same reason: the `bot` crate's script mode in phase 4 needs a text format, and
a scripted scenario is the only way to test a browser against a deterministic
partner. Taken from the `Makefile`: the `## name: text` convention that makes
`make help` generate itself from the file, and `make serve` as "build, open a
browser, serve the directory". Taken from `packaging/package-web.sh`: reading
the macroquad version out of `Cargo.lock` and copying `mq_js_bundle.js` out of
the pinned registry source rather than a CDN, and stamping the wasm's own
sha256 into the loader URL so a changed build is a different URL.

Not taken from gear-master: `Verb`'s domain, obviously, but also its
`#[derive(Copy)]` — our `Command` carries `Vec<CitizenId>` and cannot be
`Copy`, and it must be `Serialize`/`Deserialize` because commands are the
things that go over the wire. Not taken: `main.rs`. The plan is explicit that
the 13 000-line one is what we are avoiding, so `gui` is several small modules
from the first commit. Not taken: `[profile.test]` from gear-master's root —
the workbench's is the same idea with a better-argued comment and the plan
names the workbench as the source, so that is the one copied. Not taken: the
SoundCloud widget and the itch.io zip from `package-web.sh`; this game ships
to Pages and has no audio.

**perturbation-workbench.** Taken: `[profile.release]` verbatim
(`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`) together
with its comment explaining why `panic = "abort"` is deliberately absent — a
panic compiled to a bare `unreachable` surfaces in the browser as
"Unreachable code should not be executed" with no clue what broke, which is
precisely the failure mode a desync investigation cannot afford. Taken:
`[profile.test]` with `debug = "line-tables-only"`, for the same reason it was
written — file-and-line in panic messages is the only part of debuginfo a
failing assertion reads. Taken from `packaging/package-web.sh`: the portable
`sha256()` helper that falls back from `shasum` to `sha256sum` because the
Linux CI runner has only the second, the `die()` that makes a packaging
failure loud, and — most important — the closing `grep` assertions that fail
the build if the cache-busting stamp did not actually apply, so the script can
never quietly ship a page that serves a stale wasm. Taken from
`crates/wasm/src/lib.rs`: the panic hook installed at start-up so a wasm panic
prints something a person can read, and the boundary rule the file's own
header states — everything worth testing lives in a crate that runs under
`cargo test` without a browser, and the bridge file moves values across and
does nothing else. That is our `sim` / `net-web` split, restated.

Not taken from the workbench: wasm-bindgen and everything downstream of it.
Design §9.1 is the reason — macroquad boots through `mq_js_bundle.js` and
never runs wasm-bindgen, so the workbench's `wasm-bindgen-cli` version-pinning
dance, its `pkg/` directory and its per-file `bust()` calls have no analogue
here. What replaces them is one hash stamped into one `load()` call. Not
taken: `serde_json` as the boundary format. The workbench talks JSON to a
hand-written page because a person reads its exports; we talk `postcard` to
another copy of the same binary, where compactness and an exact byte layout
are the point.

**pdf-redactor.** Taken: nothing structural yet — it is the third instance of
the same three habits (state in Rust, a `web/` directory copied wholesale into
`dist/web/`, a version-pinned toolchain in the packaging script), and its value
this session was confirming those are house style rather than one repo's
accident. Its `web/` layout — hand-written files copied verbatim, no bundler —
is what `packaging/package-web.sh` does with `web/quad_rtc.js` and
`web/config.js`. Not taken: its PDF machinery, and its testing/ directory
convention, which we do not need while every test is a `cargo test`.

**matchbox.** Taken: the wire protocol, exactly. `matchbox_protocol`'s
`PeerEvent` (`IdAssigned` / `NewPeer` / `PeerLeft` / `Signal { sender, data }`)
and `PeerRequest` (`Signal { receiver, data }` / `KeepAlive`) are 68 lines of
JSON-serialisable enums, and `web/quad_rtc.js` will speak them verbatim so a
browser peer and a `matchbox_socket` native peer are indistinguishable to the
server. Taken from `matchbox_socket/src/webrtc_socket/wasm.rs`, and these are
the details phase 3 lives or dies on: both data channels are **negotiated**
(`negotiated: true`) with explicit ids, named `matchbox_socket_0` and
`matchbox_socket_1`, so channel 0 is `{ordered: true, maxRetransmits: unset}`
and channel 1 is `{ordered: false, maxRetransmits: 0}`; `binaryType` is
`"arraybuffer"`; and the implementation waits for ICE gathering to complete
before sending its offer or answer rather than trickling, with a comment saying
that removing the wait broke NAT punching in practice.

Not taken from matchbox: `matchbox_socket` itself in the browser, for design
§9.1's reason — it is built on web-sys and macroquad never runs wasm-bindgen.
It is a native-only dependency of `net-native`. Not taken: `matchbox_signaling`
or `bevy_matchbox`. And `matchbox_server` is deployed, never linked: it is not
a workspace member and appears in no `Cargo.toml`, so `make signal` runs a
binary from `PATH` and tells you the `cargo install` line if it is missing.

---

## 2026-08-29 — Who offers is decided by arrival order, not by peer id

Design §9.2 says "the peer with the lower id offers". **matchbox 0.14 does not
do that**, and since `net-web` (our JS plugin) and `net-native`
(`matchbox_socket`) must connect to each other, ours cannot either.

The real rule, read out of `matchbox_socket/src/webrtc_socket/mod.rs`: a peer
that receives `PeerEvent::NewPeer(id)` starts an *offer* handshake with that
peer, and a peer that receives a `PeerEvent::Signal` from a sender it has no
handshake for starts an *accept* handshake. The signaling server sends
`NewPeer` only to the peers already in the room, so the incumbent offers and
the newcomer answers. Arrival order, not id order.

This matters because the two rules disagree exactly half the time. If the
browser used lower-id-offers against a native peer using arrival-order, then
whenever the newcomer happened to hold the lower id both sides would offer at
once — glare — and whenever it held the higher id neither would, and the
connection would hang until the room was closed. `web/quad_rtc.js` implements
arrival order. Flagged to the author as a design correction; the design wins on
content, but not against the code it has to interoperate with.

---

## 2026-08-29 — `sim` depends on serde *and* postcard

Design §7 lists `sim`'s dependencies as "serde", and the plan's ground rules
repeat "Dependencies: `serde` only" — but the same plan's phase 1 item 11 and
CLAUDE.md both say `tests/boundary.rs` asserts the list is exactly `serde` and
`postcard`. The two-crate reading wins, because the plan's own item 1 defines
`checksum()` as FNV-1a over the postcard encoding of `World`, and §8's
`Welcome { snapshot }` needs the same encoding to send a world to a late
joiner. A `serde`-only `sim` could not compute its own checksum.

`postcard` is taken with `default-features = false, features = ["alloc"]`: the
default pulls in `heapless`, which we have no use for, and `alloc` is what
`to_allocvec` needs.

---

## 2026-08-29 — Package names are unprefixed

The crates are `sim`, `net`, `net-web`, `net-native`, `gui` and `bot`, not
`floodline-sim` and friends, because the plan writes its acceptance commands
literally: `cargo test -p sim --test scenario`. Nothing here is published to
crates.io, path dependencies never consult the registry, and a name that
collides with some crate we later depend on is still resolved by source, so
the only cost is a slightly generic name in `cargo tree`. The wasm artefact is
named `floodline.wasm` regardless of the crate that produced it.

---

## 2026-08-29 — Pages deploys `dist/web/`; `docs/` is kept but is not the route

Design §7 and §9.6 publish by copying into `docs/`, which is what gear-master
does. The plan's phase 0 instead deploys `dist/web/` from a workflow with
`actions/deploy-pages`, and phase 0 wins: it is the plan, it is the phase whose
checklist mentions Pages, and a build that is tested and then deployed by CI
cannot be a build somebody forgot to regenerate before committing. `make
publish` still exists and still writes `docs/`, as the plan explicitly asks, so
the manual route is one command away if the workflow is ever the thing that is
broken.

---

## 2026-08-29 — Citizens carry a name index, not a name

Design §11 asks whether citizens should be nameable; asked, and answered: yes,
as `name: u16` indexing a `const NAMES: [&str; N]` table in `sim`, drawn from
`World.rng` when a citizen spawns. A `String` per citizen would put heap
allocations into `World`, grow every `postcard` snapshot and every checksum,
and hand the determinism rules a new way to be violated. Two bytes buys
"Aldwin drowned" in the score screen, which is the entire reason the question
was worth asking.

---

## 2026-08-29 — The build hash reaches Rust through the JS bridge

`window.FLOODLINE_BUILD` is the wasm's own sha256, so the binary cannot be
compiled with its own hash inside it. The GUI therefore reads it back out of
the page: `packaging/package-web.sh` stamps the stamp into `index.html`, and
`web/quad_rtc.js` — otherwise a phase-3 stub — exposes `fl_build_hash()`
through `sapp-jsutils` for the wasm to call.

The alternative was to render the hash in JS and overlay it on the canvas,
which would have been less code. This way is chosen because §8 needs the build
hash *inside* the simulation, in `Hello`, to refuse mismatched peers — so it
has to cross the boundary eventually. Making it cross in phase 0 means the
`sapp-jsutils` bridge that all of phase 3 rests on is proven by the first
deployment rather than first exercised under a WebRTC handshake. Native builds
have no page to read, so they report the short git hash baked in at compile
time, or `dev` outside a checkout.

---

## 2026-08-30 — wasm undefined symbols are imports, and must be said so

The first Pages deploy failed to link:

    rust-lld: error: undefined symbol: fl_build_hash
    rust-lld: error: undefined symbol: js_string_length

on a build that linked cleanly on the machine it was written on. The
difference was the toolchain: rustc 1.95 locally, 1.98 on `dtolnay/rust-
toolchain@stable`. `fl_build_hash` and sapp-jsutils' `js_*` externs are
supplied by the page at instantiation time and there is nothing for the linker
to resolve them against; 1.95's lld inferred that they were imports, 1.98's
does not. Reproduced locally with `cargo +1.98.0` before fixing, rather than
guessing at a red CI run.

The fix is `.cargo/config.toml` telling lld what the older toolchain assumed:

    [target.wasm32-unknown-unknown]
    rustflags = ["-C", "link-arg=--import-undefined"]

Verified on both toolchains, and the wasm still declares all six imports.
Notably this is *not* a workflow fix: the workflow was right and the build was
wrong, and it would have been wrong on every developer machine with a current
stable. CI is deliberately left on `stable` rather than pinned to a version
that agrees with any one laptop — running ahead is how it caught this, and a
pin would only have deferred the same failure to the first person who upgraded.

---

## 2026-08-30 — Overflow checks stay on in release

Rust checks integer overflow in debug builds and wraps in release ones. That
default is fine for a program running once; it is a determinism bug for this
one. A headless `bot` peer is usually a debug native build and a browser peer
is always a release wasm build, so an overflow anywhere in `sim` would panic on
one and quietly wrap on the other. That is not a crash — it is a desync, and
the hardest kind, because the two peers have stopped agreeing about arithmetic
itself.

`overflow-checks = true` in `[profile.release]` makes a mistake the same loud
failure on every peer. The arithmetic is written not to overflow anyway — every
`Fx` multiply and divide goes through `i64` — so this is the belt to that
braces, and it costs a branch per operation. Phase 2 item 4 profiles `tick()`
against a 20 ms budget at 500 citizens with the flood running; if the branch
turns out to be the cost, that is the place to find out and this is the entry
to revisit.

---

## 2026-08-30 — Roads and bridges are buildings; a Dike is walkable

The plan's phase 1 item 4 lists Road and Bridge among the buildings, and they
are modelled as exactly that: 1 x 1 buildings that cost builder-ticks, occupy a
cell, and take damage. The alternative was a flag on the cell, which would have
been less code today and two rules by phase 2 — the flood breaks road cells
(design §6) and it breaks buildings (§3.4), and those would have been two
separate pieces of damage logic to keep in step. One kind of thing, one rule.

A **Dike does not block movement**, which the design does not say either way.
It is a raised bank of earth and stone, and the alternative is worse than
unrealistic: a player who rings their city with a dike to keep the water out
would have walled their own citizens in, would lose them to it, and would learn
not to build dikes — the exact opposite of the lesson design §5 exists to
teach. It carries no traffic bonus either; it is walkable, not a road.

Roads and bridges also mean construction has to go through `World` rather than
through `Building` directly: the tick a bridge is finished is the tick the map
becomes passable somewhere it was not, and every cached flow field is then
wrong. `Building::build`, `deliver` and `damage` are crate-private and
`World::build_at`, `deliver_to` and `damage_building` are the door, so bumping
the navigation generation is impossible to forget.

---

## 2026-08-30 — Flow fields are a cache, and live outside `World`

Design §3.2 wants one Dijkstra field per destination rather than five hundred
A* searches. Each field is two vectors of sixteen thousand cells, so a dozen of
them would be more state than the entire rest of the game — and if they lived
in `World` they would be in the snapshot a late joiner receives (§8 budgets
50–150 KB for that) and in the checksum computed every tick.

So `Nav` is passed to `World::tick` rather than owned by it. `World` stays the
authoritative state; the fields are derived from it and rebuilt identically on
every peer, which is a claim `a_fresh_nav_cache_navigates_like_a_warm_one`
tests directly: one world ticked with a warm cache and its clone ticked with a
brand new cache every single tick must agree, checksum and all. That is exactly
the situation a late joiner is in.

What *is* in `World` is `nav_generation`, a counter bumped whenever a footprint
appears or disappears. It is cheap, it tells the cache when it has gone stale,
and it is checksummed because two peers with different generations have placed
different numbers of buildings — worth catching directly rather than inferring
from the wreckage two minutes later.

---

## 2026-08-30 — Starving is a condition, not a state

`State` was written with a `Starving` variant, and the walking tests found what
that costs: `tick_needs` set it, which overwrote `Walking`, so the moment a
citizen ran out of food it stopped moving — including on its way to the granary
that would have saved it. The bug is not the assignment, it is the modelling.

`State` is now only what somebody is doing — Idle, Walking, Working, Eating,
Sleeping, Dead — and hunger and exhaustion are separate fields with
`starving()` and `tired()` reading them. The two are independent, which is what
lets a starving citizen run for food and a tired one keep walking at half
speed. Death is the one condition that *is* a state, because it ends every
activity, and `Citizen::die` clears the destination and job that only make
sense for the living.

---

## 2026-08-30 — The Hearth holds no food, and hunger does not veto sleep

Two bugs the city test found, both worth writing down because both were a
plausible-looking design choice rather than a slip.

**The Hearth had a food capacity**, invented when its stores were written and
not taken from anywhere. Design §3.3 gives the Hearth no larder and gives the
Granary "food stockpile; citizens eat here". With a food capacity, the Hearth
sat nearer the farm than the granary on most layouts, so haulers filled it
instead, citizens ate at the fire, and the granary stayed empty for ever. The
city did not starve, which is why only a test that asked specifically about the
granary caught it. The Hearth now holds wood and stone — the starting
"stockpile of wood" of design §4 — and nothing else.

**Hunger vetoed sleep.** The priority list was written as "eat, else if not
hungry then sleep, else work", which reads correctly and is wrong: a city that
runs out of food has citizens who are permanently hungry, so they never slept
again, never claimed a bed, and worked at half speed until they starved — the
failure spiralling exactly when the player could least afford it. A hunger that
cannot be answered must not veto the sleep that can, so the check is now "if
hungry *and there is food*, eat; otherwise if tired, sleep".

---

## 2026-08-30 — A producer is not a store

`Kind::stores` originally meant "has capacity for", which put a Farm's output
buffer in the same category as a Granary. A hauler emptying a farm then looked
for the nearest place to put food and found the farm it was standing in, so the
harvest never moved. `is_store` (Hearth, Granary, Stockpile) and `produces`
(Farm) are now separate questions, and `stores` means "will take delivery",
which is the thing a hauler actually needs to know.

---

## 2026-08-30 — `Road`, `AcceptRoad`, `Trade` and `AcceptTrade` wait for item 8

The plan's item 7 says "`Command` (design §7, minus `Lend`)", which reads as
all nine remaining variants at once. Four of them are not there yet: the two
road commands and the two trade commands arrive with their mechanics in item 8,
which is the item that defines `RoadId`, `TradeId` and what a joined road is.

The alternative was to define them now with handlers that return an error or do
nothing, and that is worse than leaving them out. Item 10 asks the determinism
test for "a scripted command stream covering every `Command` variant", and a
variant that parses and does nothing would satisfy that requirement while
testing precisely nothing — the covering assertion would go green and stay
green when the real handler landed. A variant that does not exist cannot be
covered by accident.

Two variants exist that design §7 does not list. `RaiseDike` is the command for
design §3.3's dikes growing a level at a time, which the §7 sketch has no way
to express. `Demolish` is in §7 and also does the clearing of rubble, since
rubble keeps its footprint until somebody moves it.

---

## 2026-08-30 — Commands are all-or-nothing

`MoveTo` naming eight of your citizens and one of mine does nothing at all,
rather than moving the eight. Every check a command needs happens before any of
its effects.

The reason is the lockstep rather than fairness. A partly-applied command is a
command whose outcome depends on the order the checks ran in, and two peers
that disagree about how much of a rejected command took effect have diverged in
a way the checksum will catch a tick later and nobody will be able to explain.
`a_rejected_command_leaves_the_world_byte_identical` is the test, and it
compares checksums rather than fields so it cannot miss a corner of the world
that a hand-written comparison would forget.

---

## 2026-08-30 — Roads are laid as sites, and a road is not a link until it is whole

`Command::Road` marks out the cheapest path and places a construction *site* on
every cell of it, rather than conjuring a finished road. Design §6 says
"builders from the ordering city construct it", and this is what that means:
laying a road across the map is a route drawn on the ground and then several
days of somebody's builders and haulers, which is what makes the decision to
lay one cost something.

Road planning does not use `nav`'s flow fields, and the two cost functions are
deliberately different. A field answers "how does a citizen walk to X", and a
citizen cannot cross shallows; a road can, by becoming a bridge. The road
planner is also four-connected where the walking one is eight — a diagonal road
is two cells touching at a corner, and a hauler can no more walk that than it
can squeeze between two buildings.

`Road::intact` requires every cell to be a standing road or bridge, and
`linked` requires intact *and* joined. So one cell broken by a surge stops the
trade while leaving the agreement standing, and rebuilding that cell restores
it without anybody having to negotiate again. That is design §6's "the flood
breaks road cells, which is what makes rebuilding the link after an age a
decision", and it is a test rather than a hope.

---

## 2026-08-30 — A caravan is people walking, not a transfer

A day's trade could have been two numbers moving between two treasuries. It is
instead an errand given to real haulers who walk the road with it, because
design §6 says "a hauler that drowns loses the cargo" and there is no way to
lose cargo that was never on anybody's back. It also means a trade during a
flood is a gamble, which is the interesting version.

Caravans take unassigned citizens only — a farmer is not pulled off the field
to walk to another city — and never take one that is already carrying
something, because pulling it off that errand would drop the load. One merely
on its way to *pick something up* has nothing to lose and is fair game. If a
city's haulers are all busy, that day's trade is short, which is a reasonable
thing for a city to be bad at rather than a bug to fix.

---

## 2026-08-30 — A day is 1200 ticks, and the balance moved with it

Design §4 says an age is "6 days, about 12 real minutes at 10 ticks/s with 200
ticks per day", and those three numbers cannot all be true: six days of two
hundred ticks is twelve hundred ticks, which at ten a second is two minutes.
Design §11 flags age length as an open guess, so this was asked rather than
decided, and the answer was to honour the prose — twelve minutes an age, so
`TICKS_PER_DAY` is 1200 and an MVP run of three ages is a little over half an
hour.

It also settles a second collision that item 9 walked straight into. §5 wants
the surge to pour for about thirty seconds, which is three hundred ticks —
longer than a two-hundred-tick day, so the flood literally could not fit inside
its own impact day. At twelve hundred it has room to spread, pool behind a dike
and drain, which is most of what makes the flood readable.

A day six times longer makes every number keyed to it mean something different,
so they moved together rather than being left to rot:

* Food decays 1 a tick rather than 4, so a citizen empties in a thousand ticks
  — a little under a day, deliberately not exactly one, or the whole city would
  queue at the granary at the same hour.
* Rest decays 1 point every 2 ticks. An *interval* rather than a smaller
  number, because there is none: design §3.2 fixes needs at 0..=1000, so the
  only way to say "slower than one point a tick" is to skip ticks. It empties
  in two thousand, so hunger and sleep drift out of phase instead of always
  arriving together.
* A farmer takes 32 ticks a unit rather than 8, so one still feeds about three
  people and a three-slot farm still feeds nine. Left at 8 it would have fed
  thirty-six and made farms a formality.
* Sleeping recovers 2 a tick rather than 20, so a night is about a third of a
  day rather than four seconds.

Phase 5's playtesting is where these get answered with a stopwatch instead of
arithmetic. They are all in `balance.rs` for exactly that reason.

---

## 2026-08-30 — Why a run ended is recorded, not inferred

The score counts *completed* ages, and "completed" is not something the clock
can answer on its own. A city that drowned half way through age three survived
two ages; a city still standing when age three ran out survived three. Both end
in age three, at the same tick, and the first version scored them the same —
`age - 1`, which is right for the collapse and short by one for the survivor.

Nothing caught it, because item 9's tests only ever ran a run into the ground.
It was found by looking at the output of a scripted game rather than at an
assertion, which is the argument for having something to look at.

`Ending` is now stored when the run stops, and the score reads it. It is also
better on the screen: "the last city fell" and "you outlasted the ages" are
different endings and should not print the same line.

---

## 2026-08-30 — The flood: three things the design implies but does not say

Design §3.4's automaton is a page long and the numbers around it are not
self-consistent. Three had to be settled by measurement.

**Water is kept in sixteenths of a unit of terrain height.** Depth and terrain
must be comparable — a surge of height 12 has to mean something against a hill
of height 12 — but at terrain's own resolution the automaton does not work.
Splitting a cell's outflow between four equally-lower neighbours means dividing
by four, and in whole terrain units the answer is usually zero: a two-deep
puddle on flat ground never moved at all. Worse, the obvious fix — give the
remainder to the last neighbour so the sum comes out exact — makes the puddle
spread lopsidedly, because "last" is whichever way the loop happens to run, and
the plan asks specifically for a puddle that "spreads symmetrically". In
sixteenths the shares come out equal and the few that division loses simply
stay put, which conserves volume just as well.

**The sea rises during a surge.** The source corner touches two edges of the
map, and off-map was a bottomless drain at surface zero, so an age-one flood
poured in and ran straight back out beside itself: eight hundred thousand
sixteenths off the edge against forty thousand ever on the map, and a front
that stalled thirty cells in. A storm surge *is* the sea being high, so while
one is running the sea outside the map is at the surge's own level and the
water has nowhere to go but inland. When it stops, the sea falls and design
§5's step 6 — "water drains off the edges over the next day" — happens by
itself.

**The source is a pump, not a puddle.** §5 says the source "gives them flow
pointing toward the map centre", and writing that into the flow field achieves
nothing: the automaton recomputes flow from the height field every tick, so an
injected direction is overwritten before anything reads it. Held at a depth and
left to diffuse, the surge covered five per cent of the map and stopped — once
its neighbours are as deep as it is there is no gradient left to drive it. The
source now puts water down one cell inland as well as in itself, every tick,
which is the volume and the direction §5 asks for.

And the pump has to scale with the age's height, which is the difference
between design §4's escalation table meaning something and being decoration:
with a fixed pump, an age-one surge of twelve and an age-four surge of
twenty-four flooded *identically*. Scaled, peak volume runs 1.8M, 3.5M, 5.3M
across the three heights.

---

## 2026-08-30 — The front does not reach the middle of the map, and should not

The plan's test for the surge is "front reaches the map centre within N ticks".
The design's own physics will not do that and must not. Water finds its level:
a surge twelve deep cannot climb ground that is sixty higher, and §5 depends on
it not being able to — "anyone who reaches high ground or a rooftop survives"
means nothing if high ground gets wet, and "build on the high corner" is the
first counter-play the design offers.

So the test is written the other way round. `the_surge_takes_the_low_country`
asserts an age-one flood covers at least eight per cent of the map with several
hundred cells properly deep, `the_high_corner_stays_dry` asserts it never
reaches the far corner, and `a_bigger_surge_is_a_bigger_flood` asserts the
escalation table escalates. That is the behaviour design §5 actually describes,
and it is what makes a city's distance from the low corner matter — which is
design §6's "the low corner is nearer to some than others".

Terrain relief was tried at 64 instead of 255 to make the flood spread further.
It moved coverage from nine per cent to fifteen and cost more than it bought:
sixty-four distinct heights make the quantile bands that keep every seed
playable much coarser. Reverted, and recorded here so it is not tried twice.

---

## 2026-08-30 — v2: no server anywhere, and the phases swap

The design and plan were replaced with their `v2-noserver` drafts. Sections 1–6
— the whole simulation — are unchanged, so everything phases 0 to 2 built
stands. What changed is §7 to §9, and it changes the rest of the project.

**There is no server.** Not matchbox, not Fly.io, nothing. Signalling goes
through trystero over public infrastructure that already exists (Nostr relays
by default), with a pasted offer/answer blob as the fallback for when relays
are down or blocked. Both paths end in the same object — one
`RTCPeerConnection` with a reliable and an unreliable channel — so `net-web`
cannot tell them apart. If GitHub Pages is up, the game is up.

This removes the one thing I had flagged as needing the author's own account.
There is nothing left to deploy but static files.

**Star, not mesh.** Every joiner connects only to the host and the host relays.
A joiner needs exactly one connection, which is what makes a single pasted
exchange enough; the host already owns the seed and the snapshot; and browsers
cap `RTCPeerConnection`s, which a mesh hits first. The host is a relay with a
clock, not an authority — it runs the same `sim` as everyone else.

**Phases 3 and 4 swap, and that is the best part.** Lockstep is now built and
tested on `net::Loopback` — N in-process peers in a star with configurable
latency and loss — before any browser is involved, and only then does
`quad_rtc.js` have to work. Under v1 the riskiest thing was also the first
thing, and a desync and a dropped data channel would have looked identical.
Now a networking regression and a lockstep regression cannot be confused.

**Crates removed: `net-native` and `bot`.** Both existed to reach a
`matchbox_socket` peer from a terminal. `net::Loopback` does that job better —
in-process, deterministic, and inside `cargo test` — and `bot`'s scripted play
becomes a loopback test. `make signal` and `make bot` go with them.

The earlier entry about matchbox deciding who offers by arrival order is now
history rather than guidance: trystero owns that handshake. It stays because it
records a real correction to design §9.2, and because the star still has to
decide who offers if the pasted-code path is ever generalised.

---

## 2026-08-30 — Building resistances were measured, not guessed

Plan item 2.3 asks that "wooden buildings take damage from flow, stone
resists". The first numbers for that were two and six units of flow, picked by
eye. A real surge turns out to produce flow speeds with a median around thirty,
a ninetieth percentile between two hundred and two hundred and fifty, and a
peak near three hundred and eighty at the front — so those thresholds sat below
even the *median*, and every building in the game, stone dikes included, would
have dissolved within seconds of the water arriving. The dike that design §5
builds its teaching moment around would never have survived to teach it.

Measured instead: wood gives way in any strong current, stone only where the
front itself is breaking, and a road — a hand's breadth of stone rather than a
bank of it — gets its own threshold between the two, because design §6 wants
"the flood breaks road cells it flows over" and at a wall's resistance it never
would.

A dike doing its job sees still water piled against it and almost no flow at
all, so it survives; a dike standing in the front does not. That seems right:
build your dike where the water will *arrive*, not where it comes out.

---

## 2026-08-30 — The automaton is nowhere near the budget, and `sim` is optimised under test

Plan item 2.4 asks for `tick()` under 20 ms at 500 citizens with the flood
running, and offers to run the automaton every other tick if it is the cost.
It is not: on native release, a dry tick is 0.04 ms and a flooding one 0.29 ms,
so the whole automaton costs a quarter of a millisecond against a twenty
millisecond budget. Seventy times the headroom, which is what the wasm build
will spend some of. Nothing is halved, and `tests/profile.rs` is the
measurement, kept ignored so it only runs when asked.

Unoptimised, though, it was slow enough to matter: the suite went past two
minutes once the flood tests existed, because sixteen thousand cells of integer
arithmetic per tick is exactly what a debug build is worst at.
`[profile.test.package.sim] opt-level = 2` brings it back to twelve seconds. A
determinism test nobody runs is worse than no determinism test, because it
looks like cover. `debug-assertions` and `overflow-checks` are independent of
`opt-level` and stay on, so the same panics still fire.

---

## 2026-08-30 — The surge is the sea rising, and terrain relief had to come down

Two corrections to the flood, arrived at together because the first exposed the
second.

**The source is capped, and the sea comes in over the edges.** Pumping water
inland without a limit made the flood deep rather than wide: three hundred
ticks of it piled water three hundred and seventy units deep beside a surge
whose stated height was twelve. Design §5 says the source "sets depth = H", and
a set is a cap. But capping it alone left a flood that spread a dozen cells and
stopped, because a source held at depth H can only raise its neighbours to H
and then it has nothing left to push with.

What was missing is that a storm surge is *the sea being high*, and a high sea
does not wait politely at the corner it was poured from — it comes over every
low edge it can reach. Water now flows in wherever a map edge sits below the
surge's level. It stops by itself, since nothing can rise above sea level, and
when the surge ends the sea falls and design §5's step 6 happens on its own.

**Terrain relief is 40, not 255.** With the sea model, the flood's reach is
arithmetic: surge height divided by the terrain's slope. At 255 units of relief
over 128 cells that is two units a cell, so a height-twelve surge reaches *six
cells* and an age-one flood covered two per cent of the map. The design fixes
what a dike is worth (three units a level) and what a surge is worth (twelve to
twenty-four) and never fixes the terrain's range — so the terrain is what was
wrong. A three-unit dike against a two-hundred-and-fifty-five-unit landscape
was never going to mean anything.

This was tried once before at a relief of 64, abandoned because it seemed to
buy little, and that judgement was made while the uncapped pump was masking the
effect. At 40 the flood is what design §5 describes: an age-one surge takes
fourteen to nineteen per cent of the map, an age-two twenty-three to thirty-two,
an age-four thirty-six to fifty-two. That is an escalation table you can feel.

The cost is paid in the map generator: forty distinct heights make the
histogram its quantile bands walk coarse, so the bands land within a few per
cent of where they are aimed rather than on it. `every_map_has_its_shallows_and_
its_rock` now asserts the thing that actually matters — every seed has a river
to bridge and rock to build around — instead of a percentage the discretisation
cannot honour.

---

## 2026-08-30 — Losing a building lets its people go, wherever it happens

The code that unhomed residents and unemployed workers when a building became
rubble lived inside the flood, because the flood is what prompted writing it.
So a cottage pulled down by its own owner left its residents homed to a hole in
the ground until something else happened to notice. It is a property of a
building ceasing to exist, not of the water, and it lives in `release_from` now,
called from both `damage_building` and `demolish`.

---

## 2026-08-30 — Four things the lockstep had to learn, and one bug it found

Phase 3 built the star from design §8 against `net::Loopback`. Four corrections
came out of it, all of them from tests that could never have been written
against a browser.

**A `Turn`'s checksum is tagged with the tick it describes.** Design §8 says
`checksum_prev` is "a 64-bit FNV of `World` after tick T − 1", and that cannot
be true: a turn for tick T is sent `DELAY` ticks early, so the sender does not
yet know what the world will look like after T − 1. It reports the last tick it
has actually finished, and says which. Untagged, a late joiner priming its
pipeline sends four turns carrying the same checksum, the host reads them as
claims about four different ticks, and throws an innocent player out for a
desync on the tick they arrived.

**Nothing happens until the host presses Start.** Design §5 puts the button in
the lobby and it is not decoration. Without it the host simulates alone from the
moment it exists and is fifty ticks ahead before anyone finishes connecting, so
every joiner arrives into a game in progress and spends the run catching up.

**The host waits only for players who are actually there.** A world is generated
with a fixed number of cities and a joiner takes one of them; an unclaimed seat
is a city standing there with nobody commanding it. Waiting for a turn from it
stops the game before it starts.

**Giving up on a silent player cannot go through the ordinary queue.** The
`Drop` goes straight into the host's own turn for the tick being bundled, and
the player stops being waited for at once. Queued as a normal command it
deadlocks: commands are flushed while sending turns, turns are sent while the
simulation moves, and the simulation is stopped waiting for the very player the
command exists to give up on.

And the bug, which was in `sim` rather than in `net`: **a hauler that had
leftovers forgot it was holding them.** It delivered what a site wanted, kept
the rest, then looked for new work, found every store empty — because the wood
was in its own arms — and stood still for the rest of the game. Three of them
ended up holding sixty wood in front of a granary that needed ten, and the city
starved beside a farm it had built. `find_haul` now looks at what it is
carrying before it looks for anything to fetch.

That one is worth dwelling on: it is a plain single-player bug, in code with its
own passing tests, and it took a three-player networked game running for two
in-game ages to surface it. The city tests founded cities by fiat, with the
materials already delivered; only a game that had to build one from a hearth
ever produced an odd-sized load.

---

## 2026-08-30 — `var register_plugin;` before macroquad's bundle

Every page load threw `ReferenceError: register_plugin is not defined` before
any of our own scripts ran. It is macroquad's, not ours: `mq_js_bundle.js`
carries an inlined websocket plugin that does `register_plugin = function (e)
{ … }` against a global it never declares, and the bundle runs under
`"use strict"`, where assigning to an undeclared name throws.

Nothing visibly broke — the game does not use macroquad's websockets — and it
went unnoticed until a headless browser was pointed at the page and asked what
was in its console. That is the cost of it: an uncaught error on every single
load, sitting there ready to be mistaken for the cause of whatever goes wrong
next, in a phase whose whole job is going to be debugging a WebRTC handshake.
Declaring the global before the bundle loads is one line and buys a console
where anything that appears is worth reading.

---

## 2026-08-30 — Phase 3's done-condition, and the GUI it needed

The plan wants the native GUI running a two-player loopback game, and offers "a
blank canvas that logs ticks" as enough. It got the real renderer instead —
terrain shaded by height, water whose opacity is its depth, buildings as
rectangles with a glyph, citizens as a circle with two legs, and the side panel
— because phase 5 has to build it anyway and a blank canvas would have proved
only that the lockstep does not crash.

It proved more than that. A screenshot of the browser build shows two cities on
a generated map with the shallows in the low corner, both eight strong, and the
panel reading `peers at [230, 229]` — the host one tick ahead of the joiner,
which is exactly what a star with a wire in it should look like and is not
something a passing test would have shown anybody.

---

## 2026-08-30 — The letterbox: logical pixels in, framebuffer pixels out

The deployed game drew itself into the bottom-left quarter of the browser
window with the rest black. `screen_width()` and `mouse_position()` are in
logical pixels — what CSS calls a pixel — and `Camera2D::viewport` is in
framebuffer pixels, which at a device pixel ratio of two are twice as many. The
viewport was computed from logical sizes and handed to GL unconverted, so it
covered half the width and half the height of the real framebuffer: a quarter
of the area, in the bottom-left, because that is where GL's viewport origin is.

`Viewport` now carries `dpi` and applies it in exactly one place — the
`Camera2D` rect — while `x`, `y` and `scale` stay logical, because that is what
input needs. `Viewport::mouse` deliberately does *not* apply it; doing so would
put the cursor twice as far from the corner as it really is, which is the same
bug wearing the other shoe and will be easy to introduce when phase 5 adds
selection.

Worth saying how it was missed: the browser build had been checked, and it was
checked at a device pixel ratio of one, where the two coordinate systems are
the same size and the bug does not exist. It is invisible from a desktop and
unmissable on a laptop. Anything touching the letterbox gets looked at under
both from now on, which `packaging/` cannot enforce but a person can.

---

## 2026-08-30 — The handshake, written down before the plugin

The plan (phase 4 item 5) and `HANDOFF.md` both say to write the exact sequence
of events down before writing `web/quad_rtc.js`, and to implement to what was
written. This is that. Where reality later disagreed with it the paragraph says
so rather than being quietly edited, because the disagreement is the useful
part.

Notation: `H` is the host, `A` and `B` joiners. "our channels" are the two
`RTCDataChannel`s the plugin owns, distinct from anything Trystero opens for
its own use.

### Both paths share the channels, and they are negotiated

Each connection carries exactly two channels, created **out of band** —
`{negotiated: true, id: 40, ordered: true}` and `{negotiated: true, id: 41,
ordered: false, maxRetransmits: 0}` — by *both* ends independently.

Out-of-band is not a detail. The in-band alternative is one side calling
`createDataChannel` and the other waiting for `ondatachannel`, and on the
Trystero path there is a race in it that cannot be closed: the plugin does not
get its hands on the `RTCPeerConnection` until `onPeerJoin` fires, both ends'
`onPeerJoin` fire at about the same moment, and if the host creates its channel
before the joiner has attached a listener the event is gone. Negotiated
channels have no event and therefore no race — both ends name the same stream
ids and the channels simply open. It also makes the two paths identical from
the channel down, so the only thing the Trystero path adds is where the
`RTCPeerConnection` came from.

40 and 41 rather than 0 and 1 because Trystero opens its own in-band channel
(`createDataChannel("data")`, seen in the vendored bundle) and in-band ids are
allocated from 0 upwards; twenty channels would have to open before anything
reached 40. Trystero also assigns `pc.ondatachannel` directly rather than
adding a listener, which is a second reason not to want that event.

### The role frame, and why the star is enforced rather than assumed

Trystero rooms are meshes: every peer meets every other. The star wants joiners
to talk only to the host. Design §9.2 proposes that "joiners accept only the
first peer (the host) and ignore others", and first-is-the-host is a guess —
two joiners arriving together may well meet each other before either meets `H`.

So the first message on the reliable channel, sent by both ends the moment it
opens, is a single byte: `0x48` (`H`) from a host, `0x4A` (`J`) from a joiner.
The plugin consumes it; Rust never sees it. A host that reads `J` has a joiner
and reports it. A joiner that reads `H` has found the host. A joiner that reads
`J` has met another joiner, closes its two channels and reports nothing —
leaving Trystero's own connection alone, because that one is Trystero's to
manage. One byte per connection buys an invariant instead of a hope.

### Trystero path, in order

1. `rtc_host(room, 0)`. The plugin dynamically imports the vendored bundle
   named by `config.js`, so a player who only ever uses pasted codes never
   downloads it, and a failed import is a message that says "try Join by code"
   rather than a page that does not work.
2. `joinRoom({appId, password, rtcConfig}, "<build_hash>-<room>")`. Nothing is
   on the wire until a relay socket opens. The build hash is in the room name
   (design §9.4) so a stale tab cannot find a newer build's game.
3. `A` does the same. Trystero introduces them over the relays and does the
   SDP exchange itself; the plugin never sees an offer on this path.
4. `room.onPeerJoin(id)` fires at both ends. It means Trystero's own channel is
   open, which means the SCTP association exists, which is exactly the
   precondition for opening a negotiated channel with no renegotiation.
   The plugin takes `room.getPeers()[id]` — a real `RTCPeerConnection`, and
   documented API, not an internal — and creates channels 40 and 41 on it.
5. Both channels open. Each end sends its role byte on 40.
6. `H` reads `J` and emits `peer(1)` to Rust; `A` reads `H` and emits
   `peer(1)`. The two ends number their peers independently and neither cares
   what the other calls anybody: `PeerId` is local to a transport, which is
   why `net::PeerId` and `sim::PlayerId` were kept apart in phase 3.
7. Bytes flow. Channel 40 arrives as `reliable = 1`, channel 41 as `0`.
8. `B` joins. `H` emits `peer(2)`. `A` and `B` also meet, exchange `J` for `J`,
   close their channels and tell Rust nothing. This is the step that would
   otherwise have gone wrong quietly and only with three players.
9. `A` closes its tab. `room.onPeerLeave(A)` at `H` → `left(1)`. `B` had no id
   for `A` and emits nothing.
10. `H` closes its tab. `A` gets `onPeerLeave` → `left(1)`, and the lockstep
    already knows what to do with that: "the host left the game".

### Pasted-code path, in order

1. `rtc_host(room, 1)`. The plugin builds its own `RTCPeerConnection` from
   `config.js`'s ICE servers, creates channels 40 and 41, `createOffer`,
   `setLocalDescription`, and then **waits for `iceGatheringState` to reach
   `complete`**. No trickle: there is no second channel to deliver a late
   candidate on, so the blob has to be the whole thing.
2. `rtc_code_local()` returns `null` until gathering finishes and the
   compressed blob after. The lobby shows it; the player sends it over whatever
   chat they are already using.
3. `A`: `rtc_join(room, 1)` and then `rtc_code_remote(blob)` when the player
   pastes. The plugin decompresses, builds its connection, creates channels 40
   and 41, `setRemoteDescription(offer)`, `createAnswer`,
   `setLocalDescription`, waits for gathering, and `rtc_code_local()` then
   returns the answer blob.
4. `H`: `rtc_code_remote(answer)` → `setRemoteDescription`. DTLS and SCTP come
   up, both channels open, role bytes cross, both ends emit `peer`.
5. `H` immediately starts gathering a *fresh* offer for the next joiner, so
   `rtc_code_local()` goes non-null again with a different blob. One paste per
   joiner and no mesh to arrange: that is what the star bought.
6. Leaving has no signalling channel to announce itself on, so it is read from
   the connection: `connectionstatechange` reaching `failed` or `closed`, or
   either channel closing, is a `left`. Only for peers that were reported as
   `peer` in the first place — otherwise closing our own channels on a
   joiner-to-joiner link would report a departure that never happened.

### What Rust is told

`rtc_poll()` returns one event or null:

```
{k: 0, id}                        a peer is usable
{k: 1, id}                        it is gone
{k: 2, id, reliable, bytes}       bytes arrived
{k: 3, text}                      something a person should read
```

Design §9.2 writes `kind` as a string (`{kind:"peer", id}`). It is a small
integer here because the field is read on every event, sixty times a second per
peer, and reading a string across `sapp-jsutils` costs a UTF-8 conversion and
an allocation on each side. Nothing else about the shape changed.

`rtc_send(peer, reliable, bytes)` takes the bytes as a `JsObject` buffer rather
than design §9.2's `(ptr, len)`. Reading the wasm heap directly from the plugin
would save one copy of a few hundred bytes and would need the plugin to reach
for miniquad's `wasm_memory` global; the copy is not worth the coupling.

---

## 2026-08-30 — Vendoring Trystero without npm, and why Nostr and BitTorrent

`web/vendor/` holds two files, pinned by name and sha256 in
`web/vendor/README.md` and checked by `packaging/package-web.sh` before it will
package anything:

| file | version | sha256 |
|---|---|---|
| `trystero-nostr-0.25.4.js` | 0.25.4 | `6bfce15d…202906` |
| `trystero-torrent-0.25.4.js` | 0.25.4 | `93ed42a5…2558f0` |

**Getting a browser-ready file at all took a decision.** Trystero on npm is
source, not a bundle: 0.25's `trystero` package is a shim that re-exports
`@trystero-p2p/<strategy>`, and every strategy imports bare specifiers
(`@trystero-p2p/core`, `@noble/secp256k1`, `mqtt`) that no browser resolves. A
`<script>` tag needs one self-contained file, and the rule is no npm at build
time — which leaves fetching a pre-bundled build once, by hand, and committing
it. jsDelivr's `/+esm` was the obvious source and is not usable: it splits into
chunks that import each other by absolute CDN path, so the vendored copy would
still phone home at load time and the game would stop working the day jsDelivr
did. esm.sh's `…/es2022/<name>.bundle.mjs` is a single file with no imports at
all, which is the property that matters. Both files were checked for that —
`grep -c 'from *"'` is zero on each — and hashed.

**Nostr is the default and BitTorrent is the alternative; MQTT is not shipped.**
The plan says to try MQTT if Nostr is slow. Its bundle is 418 KB against
Nostr's 61 KB and BitTorrent's 52 KB — most of it an MQTT client this game
would use for one handshake — and 418 KB is most of the wasm again for a
fallback that may never be used. BitTorrent covers the same failure (Nostr
relays unreachable or slow from one end) at a tenth the size, and the real
last-resort fallback is the pasted code, which needs nobody's relays. If both
strategies turn out to be unreachable from a real network the answer is a line
in `config.js`, not a rebuild: `strategy` names one of the two files, and
`web/` is copied wholesale into `dist/web/`.

**They are loaded on demand, not with a script tag.** Design §9.1 says a plain
script tag. `import()` from inside the plugin instead, because it makes the
choice of strategy a config value rather than a build-time constant, keeps a
player who only uses pasted codes from downloading 61 KB they will not run, and
turns "the signalling library did not load" into a caught rejection the lobby
can put on screen — which is exactly the failure phase 6 wants to have a
sentence for.

---

## 2026-08-30 — What phase 4 measured, and the two places reality disagreed

Everything below is a number a browser produced, not one that looked
reasonable. `make browser-test` re-runs all of it.

**Join times, headless Chromium, two and three tabs.** Nostr introduced a
joiner to a host in **0.3–0.8 s**; BitTorrent trackers in **1.0–1.1 s**. Both
are far inside anything a lobby needs, so Nostr stays the default on nothing
stronger than being first in the design — if a real pair of home networks
disagrees, `config.js` names the other file and neither end rebuilds.

**The pasted blob is 292–369 characters.** Design §9.2 asked for under 600 and
suggested stripping inferable SDP lines *and* deflating. Stripping first turned
out to matter more than the compression: a full data-channel offer is about
1 400 characters, of which the eight kinds of line that say anything are about
600, and `deflate-raw` plus base64url takes those to ~350. Dropping Chrome's
TCP host candidates is worth about a third of the candidate lines on its own,
and they are no use without a TCP relay at the other end. It fits in a chat
message with room to spare, which was the actual requirement.

**Round trip reads as ~50 ms between two tabs on one machine, and that number
is the harness.** `echo.html` drains its queue on a 50 ms interval, so a reply
waits up to 50 ms at each end before anyone looks at it. The wire is
sub-millisecond here. Recorded because a future session will otherwise read
"50 ms on loopback" as a problem.

**Two places where writing the sequence down first was not enough.**

*A closed tab took sixteen seconds to notice.* The pasted-code path has no
signalling channel to announce a departure on, so the first version left it to
`connectionstatechange`, and measured 16.1 s — ICE consent freshness expiring.
That is inside the lockstep's thirty-second patience and outside the ten
seconds the plan asks for. A `pagehide` listener that closes the connections
puts an SCTP shutdown on the wire and the other end sees it in **0.3 s**. The
slow path is still there and is still the one that matters for a crash, a
killed process or a pulled cable — none of which get to run any code.

*Closing Trystero's connections is rude, and it says so.* The same `pagehide`
handler called `pc.close()` on every link, including the ones Trystero owns,
and the *other* tab's console filled with `Trystero peer error: OperationError:
User-Initiated Abort`. A real error message about nothing, on the page of the
peer that did nothing, in the phase whose whole job is reading consoles.
A link now records whether the plugin built the connection or was handed one,
and only closes what it made; the two channels are always ours to close and
`room.leave()` does the rest. Zero console errors on either side now, which is
the state phase 4 has to be able to trust.

**And one place it was exactly enough.** The star. Design §9.2 says a joiner
should "accept only the first peer (the host) and ignore others"; the sequence
written down before the code said that was a guess and used a role byte
instead. In the three-tab run, the second joiner's host is peer **id 2** — it
met the other joiner first, ignored it, and took the host when it arrived.
First-is-the-host would have wired two joiners together and left the host
waiting, and it would have happened only with three players and only sometimes.

---

## 2026-08-30 — macroquad's font is ASCII, and it does not say so

Three em dashes and two ellipses shipped in the lobby as hollow boxes, one of
them in the middle of the sentence that tells a player what to do when the
relays are down. macroquad's built-in font has no glyph for them and no
fallback: it draws the box and carries on.

It was invisible in review because the source is right — `"connected — waiting
for the host"` is what anybody would write — and it is only wrong at the
moment of drawing. It was invisible in the tests because nothing asserts on
letters. It was found by looking closely at a screenshot taken for another
reason entirely, which is not a process.

So `gui` lints its own sources: every string literal outside a comment must be
ASCII. It is a heuristic rather than a Rust parser and it errs towards
complaining, which is the right direction for a lint about typography. Strings
that reach the screen from `net` and `sim` — `Refusal::to_message` and
`RuleError::to_message` — are checked in their own crates' tests instead, since
the lint reads only `gui`'s files, and the plugin's messages were fixed by
hand. The prose in comments and in this file keeps its typography; nobody draws
a comment.

---

## 2026-08-30 — A relay that has not answered yet is not the end of the game

`Event::Error` set `Status::Ended`, which is right once a run is going — the
transport has failed and there is no game any more — and wrong in a lobby,
where most of what a transport has to say is advice. "No signalling relay
answered in fifteen seconds, try a pasted code" would have thrown the host out
of the room they were waiting in, discarding a room code they had already sent
to somebody, on the strength of a warning that is often just slowness.

`Lockstep::trouble` carries it instead while the status is `Lobby`, and the
lobby prints it and offers the other path as a button. A button rather than the
plan's "offers *by code* automatically", because switching automatically would
silently invalidate the code the host is at that moment reading down a phone.

The failure was reproduced rather than imagined: a Playwright context that
aborts every `wss://` request and names a signalling bundle that does not
exist, which is what a blocked network looks like from inside the page.

---

## 2026-08-30 — Design step 7, and the four things playing it found

The plan's phase 5 ends with "playtest the flood until it is fun", and the
prompt for this session says to run at least three full games and to say
plainly if it is not fun yet. Here is what was done, what it found, and what is
still unanswered.

**Nobody has played FLOODLINE with their hands.** That has to be said first,
because everything below is a measurement and a measurement cannot tell you
whether a game is fun. What it *can* tell you is whether the decisions a player
is being asked to make have different outcomes, and a game where the careful
player and the idle one end up in the same place is not unbalanced — it is not
a game, and no amount of judgement about how it feels will fix that.

So `crates/sim/tests/playtest.rs` plays five strategies through full three-age
runs on three seeds, entirely through `World::apply`:

* **idle** — found a city, do nothing.
* **grow** — a farm, a granary, two cottages, a second farm, one a day, and
  farmers assigned as each farm goes up.
* **dike** — grow, and a wall along the shore between the city and the water.
* **flee** — grow, and everybody uphill on the impact day.
* **both**.

Run it with `cargo test -p sim --release --test playtest -- --ignored
--nocapture`. The first run of it said this:

```
  seed        play   ages  alive by age
  31          idle      0   [0]
  31          grow      0   [0]
  31          dike      0   [0]
  31          flee      0   [0]
```

Every strategy, every seed, dead before the end of age one with no water
anywhere near them. Four things came out of chasing that.

### 1. Nothing in a city ever built anything

A city that places a farm sees its haulers carry the wood and the stone to the
site and then stop. `Job::Builder` existed, `Assign` on a site produced one,
and nothing else in the game ever did — so unless the player knew to select
citizens and right-click the hole in the ground, the site sat there full and
finished-looking for ever. Because a citizen can only eat at a granary and a
granary is a building, the city then starved on day four with the materials
lying on the floor.

An unassigned citizen with nothing to carry now picks up a shovel. Assignment
still does what it did — an assigned builder goes to *its* site and stays, and
`BUILDER_SLOTS` caps how many work on one at once — so what this costs is
nothing and what it buys is that an unattended city builds slowly instead of
dying. It does *not* extend to farming: a farm with nobody in it still produces
nothing, and `nobody_takes_a_job_that_was_not_given_to_them` says so, because
who works where is the whole of what a player does with their citizens and a
city that staffed itself would leave them nothing to manage. Placing a building
is an order already given; carrying it out is not a choice.

Two tests in `tests/city.rs` now hold both halves of that line.

### 2. "Get uphill" did not work, and it is the order the game is about

Design §3.2 calls it "the one order that matters during a flood". A citizen
that walked to where it was sent arrived with no errand, `find_work` handed it
one, and it turned round and walked back to the farm it had been told to leave
— inside a tick, a day before the water came. `Citizen::held` is what a move
order sets and what `Unassign` (the panel's "back to hauling") clears.

### 3. Every citizen starts inside a building, and could not be ordered out of it

A Hearth blocks movement, the founding party spawns on its site, and a flow
field has no step for somebody standing in a wall. So "select everybody, go
uphill" on the first morning left whoever had not wandered off yet standing in
the fire, permanently unorderable, for the rest of the run. The unit test that
covered this asserted the *old* behaviour — "stopping is the honest answer" —
and that reasoning is right when there is a path from where you stand and
useless when there is not. A citizen on impassable ground now walks out of it
first.

### 4. The map decided the game, and it should not have

This is the big one and it has its own arithmetic in `balance::SHORE_DISTANCE`.
Hearth sites sat on a ring around the map centre, and the centre of a
128-cell map is 128 Manhattan cells from its corner while an age-one flood
stops at about 115. Measured across the three seeds: one city sat 65 cells from
the water and lost five of its eight people to the first flood before it had a
granary; another sat 148 cells away and never got wet in three ages. Nothing a
player did moved the outcome as much as which spot the ring's random rotation
handed them.

The ring cannot be fixed by moving it — a circle about a point is not
equidistant from a corner, and one of radius 54 already spans 108 of the map's
128 cells. So the sites go on the **shore parallel**: the line at a fixed
distance from the corner the water comes out of, spread along it, with a couple
of cells of jitter for design §6's "comparable (not identical)". Ninety-six
cells out, which the flood-reach measurement says is where an age-one flood
wades through the streets and an age-three flood is properly dangerous.

It costs something and the cost is written down rather than hidden: six cities
forty cells apart need two hundred cells of shore and there are a hundred and
thirteen, so `MIN_SITE_SPACING` falls from 40 to 17 and five- and six-player
maps are cramped. Design §11 already lists map size as an open question and now
has a second reason to. Given the choice between neighbours who can see each
other and whole cities standing outside the flood for a three-age run, the
flood wins: it is the game.

### What the runs say now

```
  seed        play   ages  alive by age    at the hearth
  31          idle      0   [0]            [0]
  31          grow      2   [8, 6, 0]      [19, 86, 187]
  31          dike      2   [8, 8, 0]      [6, 72, 179]
  31          flee      2   [8, 7, 0]      [19, 86, 3]
  1000003     grow      2   [8, 8, 0]      [0, 58, 287]
  1000003     dike      2   [8, 8, 0]      [0, 54, 285]
  4043362590  grow      3   [8, 8, 1]      [0, 17, 116]
  4043362590  dike      3   [8, 8, 5]      [0, 13, 102]
```

Idling still kills you, and now it kills you by starvation rather than by a
bug. Growing gets you through ages one and two with a bloody nose. A dike keeps
everybody through ages one and two on every seed and five of eight through age
three on one of them — it is the best thing to do, which is what design §5
wants it to be. Running uphill saves the people the water would have taken and
costs a day's farming, and age three's second corner half a day behind the
first is what catches a city that comes home too early.

### What is still not answered

**Whether it is fun.** Nobody has played it. Three things are visible from the
measurements that a person should be asked about:

* **Age three kills everybody on two of three seeds** whatever is done. It is
  the last age, so losing there is losing at the end rather than being denied a
  choice — but a run that always ends the same way is a run with no ending
  worth reaching.
* ~~**Stone runs out and there is no Quarry.**~~ Answered, in the same
  session. The measurement said a wall that changes the outcome is about
  thirty-four cells, which at the old price of forty stone a level was 2 720
  stone against a purse of 120 — one twentieth of one wall. A dike now costs
  ten a level and a city starts with 720, so a player gets **one good wall in a
  run** and has to choose where to put it and how high to build it. The probe's
  `dike` strategy now *orders* its wall through `Command::Place` and lets the
  same eight people haul and build it out of the same stone: thirty-three cells
  for four hundred stone, up before the age-one flood, and the builders on it
  are builders who are not farming. That trade is the decision, and it now
  exists. Nothing still produces stone, which is what makes the one wall a
  choice rather than a chore.
* **A day is two minutes and a run is thirty-six.** §11 already suspects that
  is too long for an evening. Nothing here can tell.

They are three specific questions rather than "is it fun", which is the most
useful shape the answer could have had without a player.

---

## 2026-08-30 — A dike costs ten a level, and a city starts with 720 stone

Measured, and the measurement is in `balance::STARTING_STONE`. The short
version: a wall that changes the outcome of a run is about thirty-four cells
long, the old price put that at 2 720 stone at two levels, and a city started
with 120. The flood answer the whole design turns on could not be afforded at
one twentieth scale, and nobody had checked because every test that involves a
dike delivers its stone by fiat — including the one that proves dikes work.

Ten a level and a purse of 720 buys seventy-two dike-levels: thirty-six cells
at two levels, or eighteen at four. Nothing in the MVP produces stone, so that
is the whole run's worth, and the probe now builds its wall through
`Command::Place` like a player would — thirty-three cells for four hundred
stone, finished before the age-one flood, with the builders on it not farming
while they do it. Both halves of that show up in the runs: the dike strategy
keeps more people through ages one and two than growing does, and the strategy
that builds a wall *and* runs uphill sometimes does worse than either, because
it spent the same eight people on both.

The tests that asserted "forty" now ask `Kind::Dike.cost()` instead. What they
were for is that a dike pays per level and starts holding water back only when
it stands, and neither of those is a fact about the number.

---

## 2026-08-30 — Three ways a room stopped working, and one gesture that did nothing

Found by playing, not by testing: a real evening in which two people could not
get into the same game and a single-player village starved before the flood.
Every one of them was reproduced from scratch before it was touched, and every
one now has a test.

### The room that would not let anybody in

Joining an abandoned room looked exactly like joining a room that does not
exist. Logging the bytes on the channels settled it in a minute:

```
in   1 byte  [72] = 'H'    the host's role byte
out  1 byte  [74] = 'J'    ours
out 16 bytes              Hello, with the build hash
                          <- nothing. ever.
```

WebRTC had connected. The host had received a `Hello` and answered with
silence. Three separate faults, all of them fatal on their own.

**The plugin outlived the game.** Leaving the lobby set `session = None`, which
dropped the Rust `Session` — and `WebPeer` had no `Drop`, so nothing ever told
the browser. The tab stayed in the Trystero room for as long as it was open,
exchanging role bytes with anybody who arrived and queueing their `Hello`s for
a game that no longer existed. `WebPeer` now closes the room when it is
dropped.

That needs a session generation, and the generation is not belt and braces: a
new session is built *before* the old one is dropped, so hosting a second game
opened a room and then had the previous session's teardown close it half a
frame later. `rtc_host` and `rtc_join` hand back which session they made and
`rtc_close` takes it, so a stale handle closes nothing.

**A seat was never given back.** Player ids came from a counter that only went
up, so a two-seat game accepted exactly one joiner *for the host's whole life*.
The first one takes player 1; they close the tab; every `Hello` after that is
answered "this game is full". Two people who fumbled their first attempt could
never play at all. Seats are now the lowest free one — and freed **only in the
lobby**, because a player who drops mid-run leaves a city standing and handing
their seat on would hand over the city with it.

**Any departure was the host leaving.** `peer_left` did not look at *which*
peer had gone, so a joiner that met another joiner and let it go announced "the
host left the game". In a Trystero room, where everybody meets everybody, that
is not an edge case. It now checks, and a joiner that loses a host which never
answered goes back to waiting and greets whoever turns up next — which is the
way out of the failure above, if one is ever left behind again.

**And the screen said the same thing either way.** A joiner's `connected()` was
always 1, because `peer_of` is a host's bookkeeping, so the lobby could not
tell a finished handshake from a dead relay: "looking for the host on the
public relays" whether it had found one or not. `Roster` used to carry
`world.players` — every seat on the map, occupied or not — and joiners ignored
it. It now carries who is actually connected, joiners keep it, and the lobby
says which of the three things is happening. A joiner that has been connected
for five hundred frames with no `Welcome` says so, and suggests a fresh room
code.

### The gesture that did nothing

Choose the whole city, right-click the farm. That is the most natural thing a
player does and it was refused *whole*: a farm has three job slots, eight is
more than three, and a command is all-or-nothing (which is right, and stays).
So nobody farmed, the farm stood empty, and the city starved on day four — with
a red line under the map that faded in three seconds as the only sign. The
flood, which is the entire game, was never reached at all.

`World::will_take` and `World::will_house` answer "how many of these would you
actually take", using the same arithmetic `assign` and `SetHome` use and living
next to them, so the two cannot drift. The mouse asks first and sends what
fits: three go to work, and the game says *"3 of 8 - that is all the room there
is"*.

Three things went with it, because the failure was as much about not being told
as about not working. The panel has the warning line phase 5 asked for and it
names the one thing that will kill this city next — "no granary: your people
have nowhere to eat" is the sentence that would have saved the evening. The
panel says what is under the cursor, so a farm's three slots are visible before
the click rather than as a refusal after it. And a refusal is drawn on a plate
and held at full strength before it fades, instead of dissolving into the
terrain from the first frame.

### And one number that was wrong because of another

`STARTING_STONE` went to 720 last session and the Hearth's capacity stayed at
500. A hauler that carried twenty stone to a site wanting ten had nowhere to
put the other ten — the Hearth was over its own capacity, so `has_room_for`
refused it — and the leftovers stayed in its arms for the rest of the run. A
hundred and forty stone, a fifth of the game's entire supply, quietly gone.
`the_hearth_can_hold_what_a_city_starts_with` now makes that a rule rather than
a coincidence.

---

## 2026-08-30 — The simulation gets a clock

`main` advanced the world one tick per rendered frame. Design §3.1 says ten
ticks a second; measured on the deployed build it was **24.3** in a headless
browser (tick 248 at ten seconds, tick 977 at forty) and about sixty on an
ordinary display. So a day was twenty to fifty seconds instead of two minutes
and a whole run six to fifteen minutes instead of thirty-six, and the speed of
the game was a property of the machine it was watched on.

Everything counted in ticks went with it. `DROP_AFTER_TICKS` is three hundred,
which design §8 calls thirty seconds of silence; at sixty frames a second it
was five. `WAIT_WARN_TICKS` was under a second. Neither was wrong in the code —
they were counting the right number of the wrong thing.

It also decided the pace of a *shared* game by one machine's frame rate. The
host emits one bundle per call and nobody advances without one, so a host on a
120 Hz display ran everybody's game at twice the speed of a host on a 60 Hz
one, and neither at the speed the balance was measured for.

An accumulator in the frame loop, and the numbers mean what they say again:
measured at exactly 10.0 ticks a second afterwards (tick 101 at ten seconds,
tick 401 at forty).

Two things about its shape. In the lobby it advances once a frame instead —
nothing is being simulated there, and a handshake should not wait on a clock.
And a frame may make up at most eight ticks: a tab that was backgrounded for a
minute comes back owing thousands, and simulating them all freezes the page for
seconds and then does it again. Dropping that backlog is right for the peer
that fell behind, because lockstep will not let it run ahead of anybody
regardless — the host is waiting on its turns either way.

---

## 2026-08-30 — Wood and stone have a source, and rock is what the quarry is for

Design §3.3 lists a Forester's hut and a Quarry among the buildings that
"produce food / wood / stone"; the plan deferred both, and the result was that
*nothing in the game made wood or stone at all*. A city started with two
hundred wood — a farm, a granary and two cottages, near enough exactly — and
that was the whole run. The first question a player asked was "how do I get
more wood", and the answer was that there was none.

**Forester's hut**, thirty wood, two slots, sixty-four worker-ticks a unit.
**Quarry**, forty wood, two slots, ninety-six. Both cost wood and no stone, so
a city that has spent its stone on dikes can still dig itself out — and a
forester's hut has to be affordable beside a farm and a granary out of the
founding two hundred, or the one building that ends the wood shortage is the
one the shortage stops you building.

Measured rather than guessed, in `a_forester_and_a_quarry_pay_for_a_building_in_a_day`:
a day of two foresters is **37 wood** — a cottage and a bit — and a day of two
quarriers is **24 stone**, two and a half dike levels. Six days of an age is
then roughly two buildings or fifteen dike cells from one hut and one quarry,
with two of your eight standing at each. The shortage is real, the answer to it
is real, and manning both is most of a city.

**A quarry needs rock beside it**, and that is the only rule in the game that
asks what is *next to* a footprint rather than under it. Rock was decoration
before: every map has some, none of it is buildable, none of it is passable,
and nothing wanted it. Now the building that ends the stone shortage has to go
somewhere particular, which is a decision about the map rather than another
slot on the build menu.

`Job` gained `Forester` and `Quarrier` rather than reusing `Farmer`, because
design §3.2 names them separately and the panel says which one somebody is.
`Kind::ticks_per_unit` replaced the single `FARM_TICKS_PER_UNIT` at the one
place production happens.

---

## 2026-08-30 — Copying happens in the click, not in the frame after it

The Copy button on the pasted-code screen did nothing. Not "sometimes" — never,
and silently, on the one screen whose entire content is a string you have to
get to somebody else.

`navigator.clipboard.writeText` is only allowed while a user gesture is live.
macroquad reads a click in the *animation frame after* the browser delivered
it, by which time it is not, so the write was refused — and it is refused by
returning a rejected promise, which `try`/`catch` cannot see. So the code
`return`ed as though it had worked and the `execCommand` fallback beneath it
never ran. Both halves of that are worth remembering for anything else that has
to happen "when the player clicks".

The plugin now copies inside the canvas's own `click` listener, where the
gesture is live. Rust arms it each frame with the text *and the button's
rectangle in page pixels*, and the listener checks the click against that
rectangle. Hovering was tried as the signal first and is too slow: a click can
arrive before a frame has been drawn with the cursor over the button, which is
exactly what a fast click is.

`page::copy` still hands the string to miniquad as well, because ctrl-C goes
through the page's own `copy` event — a real gesture, always allowed — so the
keyboard works even where the button is refused. The screen says so.

Tested with clipboard *writing denied*, which is the case the fallback is for.

---

## 2026-08-30 — "Try Join by code" was the wrong advice for half the failures

Trystero's `onJoinError` fires for three different things and the lobby
answered all of them with "one of you may be behind a strict NAT. Try Join by
code." Two of those are wrong.

A pasted code replaces the *introduction*. It does not replace the connection:
both paths end in the same `RTCPeerConnection` and the same ICE negotiation. So
when two peers have already found each other and cannot open a direct link —
which is what a strict NAT, or a router keeping its own clients apart, actually
looks like — a pasted code fails in exactly the same place. Sending somebody
there is sending them round a loop, and it did.

`Event::Error` now carries `try_a_code`, the plugin sets it per failure, and
the lobby only offers the button when a different introduction would help. The
messages say which of the three happened. And the button keeps what the player
was doing: it used to turn a *joiner* into the host of a brand new empty room,
which is not a way out of anything.

---

## 2026-08-30 — Citizens take up room

Two rules, both about what a player sees. Eight people standing at a hearth
were one circle with a number of them inside it, and anybody walking to a
granary walked through whatever was in the way.

`crowd.rs` runs **last in the tick**, after everything that moves anybody:
walking, the flood carrying bodies about, a citizen stepping out of a building
it started inside. Doing it once at the end rather than inside each of those is
what makes "nobody is standing in a wall, and nobody is standing in anybody
else" true whatever put them there.

**A cell index, not pairwise.** Sixteen thousand `u16`s, cleared and refilled
each tick, so "who is near me" is nine lookups. The pairwise version is a
quarter of a million distance checks a tick at five hundred citizens. Measured
in `tests/profile.rs`: a tick with the flood running went from 0.36 ms to
**0.46 ms** against a twenty-millisecond budget.

**Everything is integer and every loop has a fixed order**, for the reason
every loop in `sim` does: two peers that resolved a crowd in different orders
would push the same two people in different directions and the game would come
apart inside a second. The index is filled in descending id order so each
cell's list comes out ascending; citizens are visited by id; the push is whole
1/256ths; and two people on the same spot are separated by *the lower id
stepping aside*, which is the same choice on every machine.
`the_crowd_settles_the_same_way_on_two_peers` is the check.

**One fixed-point trap, worth writing down.** The first version pushed two
co-located citizens apart along `V2::new(-Fx(1), Fx(0))` — a vector one
256th of a cell long — and normalised it with `with_len`. `with_len` divides by
a length computed in the same 1/256ths, and the squared length of 1/256 is
*zero*, so it normalised to nothing and a knot of eight people stayed a knot
for ever. Anything shorter than about a sixteenth of a cell has no usable
direction in this representation; the fallback is a whole-cell unit vector now,
and the threshold is explicit.

**And they no longer start inside the fire.** The founding party was scattered
within two cells of the hearth's middle, and a Hearth is three by three and
blocks movement — so everybody began the game standing in a wall. `spawn_ring`
walks outward from the hearth in a fixed order and hands each of them a cell
somebody can actually stand on. That also removes the last reason for
`step_off_a_building`, which stays anyway: a citizen can still end up inside
something the flood dropped a building on.

---

## 2026-08-30 — The tutorial is one line that always knows what to do next

Not a scripted sequence with a Next button. `tutorial::next_thing` reads the
world and names the single most urgent thing this city needs, and the panel
draws it. A player who does things out of order is never told to do something
they have already done, and one who knows the game sees an empty line — which
is the property a step counter cannot have.

It is ordered by what kills you soonest, and the order is arithmetic rather
than taste: a citizen empties at tick 1000 and dies 3600 later, so a city with
nowhere to eat has until **day four**, and the water does not come until **day
six**. Food first, always; then the two things that run out and used to have no
answer at all; then the flood.

There is also a card on the first run — three controls, two lists, and the two
dates that matter — dismissed by any click or key and never shown again. It is
modal, which is deliberate: a click that dismisses it must not also put a
building down behind it.

Every sentence in here is one somebody needed and did not have. The village
that started this starved on day four with a farm standing in it, because
"choose everybody and right-click the farm" was refused whole and nothing said
so. The panel now says *"drag to choose your people, then right-click the
farm"* until somebody is farming, and *"3 of 8 - that is all the room there
is"* when they do.

The line is wrapped to two rows and both rows are reserved whether there is
anything in them or not. The panel is 330 pixels of usable width — about
fifty-six characters — and a sentence that says what to do next does not fit in
one; reserving the space means the buttons below do not move as a city's
situation changes, which they did twice while this was being built.

---

## 2026-08-30 — Each producer is bought with what the other one makes

A forester's hut costs forty stone. It cost thirty wood, and so did the quarry,
which meant the wood shortage funded its own cure: the one building that ends
it was bought with the thing there was none of. It also left the seven hundred
stone a city starts with nowhere to go but dikes.

A city begins holding the stone and wanting the wood. So stone buys the hut
that cuts timber, and the wood it cuts buys the quarry that cuts stone back.
Forty is a fifth of what is in the Hearth on day one, so the hut is never the
building the shortage stops you building, and
`the_two_producers_are_bought_with_what_the_other_one_makes` holds both halves:
neither producer may be bought with its own output, and a granary, a farm and a
hut must all still fit inside the opening stock with room left for the quarry.

First of the seven milestones agreed for the river-and-gold plan.

---

## 2026-08-30 — A camera over the map, and a second conversion that stays home

M2 of the plan. The map is 128 cells square drawn at eight pixels a cell and a
city is about twelve cells across, so the game was being played on a postage
stamp. `screen::MapView` is zoom and pan: wheel toward the cursor, arrows or
middle-drag to move, `0` to frame the whole map again.

**The rule that keeps this safe is the one that already existed.** The
letterbox has been wrong twice, both times because two places did the same
arithmetic and disagreed, and both times it was invisible at a device pixel
ratio of one. So the camera is not a multiplication sprinkled through the
drawing code: `MapView` is the only thing that converts between the logical
canvas and the map, `draw` and `input` ask it, and `draw::map_rect` and
`draw::cell_at` are gone rather than left as a second opinion. Map space is
`cell * CELL` with its origin at the map's corner — the same units everything
was already drawn in, so no building's position changed.

**One real trap, found by writing it down rather than by a bug.**
`Viewport::camera` passes the letterbox's *top* margin as the GL viewport's y,
and GL wants the distance from the *bottom*. It has always been correct because
the letterbox is centred, so its top and bottom margins are the same number.
The map window is not centred, so `MapView::camera` computes
`fb_h - top - h` — and if the letterbox ever stops being centred, the older one
will need the same treatment.

The map gets its own `Camera2D` with a viewport, which is how macroquad
scissors: terrain cannot draw over the side panel at any zoom, and the test
checks that at both pixel ratios. Culling came free with it — the ground pass
is sixteen thousand rectangles at the fit and a few hundred up close.

`packaging/browser/view.py` is the same discipline on the test side: one copy
of the letterbox-and-camera arithmetic, imported by every script that clicks
the map, instead of four scripts each doing the sum.
