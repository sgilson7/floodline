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

---

## 2026-08-30 — Which way a building runs

M3 of the plan, first of three commits, and deliberately on its own: it is a
wide mechanical edit and it is much easier to review without a pressure model
tangled into it.

A dike is three cells long and one deep, so for the first time a footprint has
an orientation. `Building::facing` is a new field and `Kind::size` takes a
`Facing`, which ripples through `footprint`, `fits_on_map`, `ground_suits`,
`neighbours_suit`, `can_place` and `place`. `Command::Place` carries a facing
too, and every kind carries one even though only the dike can use it — a wire
format with a field that is present for one variant and absent for the others
is a wire format with two shapes, and the transcript would need two spellings
of `place`. It is `place dike 40 12 ns` now, for everything.

**`Facing` is named for the axis the long side lies along, not the direction a
wall faces.** A wall running east–west faces north and south, so "facing" is
ambiguous exactly where it matters. `EastWest` is three across and one deep,
and the doc comment says so, because the first person to guess will guess
wrong.

**A square building forgets which way it was placed.** `Building::site`
normalises the field to `EastWest` for any kind that does not turn, and
`Kind::turns` is read off `size` rather than kept as a second table. Without
that, a cottage placed "north–south" and the same cottage placed "east–west"
would be different bytes, and two peers would checksum a distinction the game
does not make. `a_square_building_forgets_which_way_it_was_placed` holds it.

**The GUI still places dikes east–west only, for one commit.** The facing a
player chooses comes from the drag they draw the wall with, and that is the
next commit; a rotate key would be dead the day it landed. Everything else on
the menu is square and could not tell the difference.

Two tests were the test encoding an old world rather than the code being
wrong, and both were rewritten rather than patched. `a_one_by_one_centres_on_
itself` now uses a road, which is the 1 × 1 it meant; the dike got its own test
for three cells and a transpose. And `water.rs`'s two walls are stacks of
east–west segments, so they are `DIKE_LENGTH` cells thick — "behind the wall"
now means past the far side of it rather than one cell in, which is what those
sums were quietly measuring.

`playtest.rs`'s `order_a_wall` walks a diagonal a cell at a time and now leaves
gaps where segments overlap. It is a measurement, not an assertion, and it
becomes a single `DikeLine` in the next commit, so it is left alone rather than
half-fixed twice.

---

## 2026-08-30 — A wall is drawn, and it is drawn straight

M3's second commit. `Command::DikeLine { from, to }` lives beside
`Command::Road` and deliberately does not reuse it: a road takes the cheapest
path between two cells and a wall stays on the line you drew, so they would
share a name and nothing else.

**Three rules, and each of them is a refusal to be clever.** A run snaps to
whichever axis it is longer along and keeps the cell it started from, because a
wall that wandered would be a wall you could not aim. It snaps to a whole
number of segments and may overshoot the cursor by up to two cells, because a
wall with a one-cell hole in it is not a wall — and the ghost shows the run
that will actually be built, so the overshoot is visible before the mouse comes
up. And a segment the ground or another building refuses is skipped rather than
failing the line, because the alternative is a tool that rejects a
forty-cell wall over one segment clipping the corner of a farm, which is a tool
a player stops using.

**`plan_dike_line` and `lay_dike_line` are one arithmetic, not two.** The drag
tool draws the ghost and totals the price from `plan_dike_line`; `lay_dike_line`
lays exactly what it returns. The letterbox has been wrong twice in this repo
because two places did one sum, and being shown one wall and sold another is
the same bug wearing a different hat. `the_ghost_and_the_wall_are_the_same_
arithmetic` holds it.

**The dike leaves the build tool for a drag tool.** Press where the wall
starts, drag, let go where it ends; the ghost and a running cost follow the
cursor. Pressing and releasing on the same dike still raises it, which is
design §3.3's "dikes grow" and the only way to spell `RaiseDike` from a mouse.
The anchor is tracked through the frame rather than read back out of
`self.tool`, because a click fast enough to go down and up inside one rendered
frame arrives with `clicked` and `released` both set — and the browser check
that presses `7` and clicks does exactly that.

**A segment is priced per cell, and the playtest is why.** A dike segment is
three cells, and leaving `cost` at ten stone would have made a wall a third of
its measured price and a third of its measured build time overnight. Nobody
asked for that and no assertion would have caught it: the five-strategy
playtest did, by reporting a dike strategy that no longer had to choose where
to spend. Stone and builder-ticks now both scale with `DIKE_LENGTH`.

**What the shape cost, measured rather than guessed.** `playtest.rs` drew its
wall as a diagonal of single cells, which against a four-neighbour water
automaton is a perfect seal for one cell of stone per cell of front. Straight
segments cannot draw a diagonal, so it now draws a staircase — a tread across
the water's path and a riser back down it — and half of every staircase runs
parallel to the flow rather than across it. Same seeds, same stone:

|                    | wall | stone | survivors: dike | both |
|--------------------|------|-------|-----------------|------|
| diagonal of cells  | 33   | 410   | 2               | 4    |
| staircase, mispriced | 33 | 190   | 0               | 0    |
| staircase, priced per cell | 33 | 420 | 1            | 1    |

The wall is worse per cell than it was, and that is a true fact about walls
drawn straight rather than a bug to be tuned away here. M4 replaces the corner
flood with a river, where a wall along a bank *is* a straight line and the
staircase does not arise; M5 re-derives all of these numbers against it. Fixing
it now would be tuning a barrier against a flood that is about to be replaced.

The L in `scenario.rs` learned the same lesson in miniature: three-cell
segments do not tile a corner, so the second arm skips the segment that would
have overlapped the first and leaves one cell either side. The test patches the
seam with a short run across it, which is what a player watching the ghost skip
would do, and then asserts there is no gap — an assertion the old cell-by-cell
wall never needed and never made.

---

## 2026-08-30 — A wall is leaned on, not battered — and the plan's formula was wrong

M3's third commit, and the one the milestone exists for. `Building::stress` is
a `u32` that the water adds to and time takes away; past a limit set by the
dike's level, the segment is rubble. Dikes leave `batter_buildings` entirely,
so a wall has one model and not two disagreeing about the same building.

**The plan said pressure is `depth * speed`, and measuring it said that is
zero exactly where a wall earns its keep.** Water a dike has stopped is water
that has stopped moving. `dike_pressure_on_flat_ground` found fifty-one
sixteenths piled against a level-one wall travelling at a speed of *two*: the
product is four hundred, which divided by any sane scale is nothing at all, and
across eight hundred ticks of a held surge every wall in the game accrued
exactly zero stress. A dam is loaded by the depth it holds; flow is what makes
the leading edge worse than the pool behind it. So pressure is
`depth * (STILL_PUSH + speed)` — speed is a term that adds rather than a factor
that gates — and `balance::STILL_PUSH` carries that paragraph in its doc
comment. This is a departure from the agreed plan, taken because the
measurement is unambiguous and the alternative is a feature that does nothing.

**The wet side is whichever side is wet.** A segment offers `sides()` — the row
or column beside each of its long faces — and the flood takes the greater of
the two pushes. A wall does not know which way round it was built, and asking
the water is one rule rather than a facing convention nobody would remember.

**Relief runs outside the water's guard.** `flood_bodies` returns early when
there is no water on the map, and the first version of `press_dikes` sat inside
that — so a wall shed stress only while it was being loaded, which is to say
never. The half of this that matters between floods is the half that runs when
the map is dry.

**The numbers, measured on flat ground against a real surge.** A wall the water
does not top takes about 26 000 scaled pressure-ticks from an age-one surge and
about 35 000 from an age-three one. With limits of 6 000 / 30 000 / 45 000 /
60 000 by level:

| surge | level | peak | broke | segments left of 42 |
|---|---|---|---|---|
| age 1 (12) | 1 | 6 006 | tick 1064 | 8 |
| age 1 (12) | 2 | 26 188 | no | 42 |
| age 3 (20) | 1 | 6 007 | tick 920 | 5 |
| age 3 (20) | 2 | 30 004 | tick 3019 | 26 |
| age 3 (20) | 3 | 35 289 | no | 42 |

A level one gives way to the first flood, a level two holds it and is in
trouble by the third, and a level three holds everything the MVP can throw.
**These are provisional and M5 owns them**: the plan's target is a fraction of
each level broken, measured across seeds against the river, and flat ground is
not a map.

`STRESS_RELIEF` is two a tick, which is the number that decides whether "a dike
that survives one surge is weaker for the next" means anything. The gap between
floods is an age — 7 200 ticks — so a wall sheds 14 400 in it: a level one is
clear again well before the next flood, and a level two that came through at
nine-tenths of its limit meets the next surge still carrying two fifths of the
last.

**A strained dike darkens.** `Building::strain` is nought to a hundred and
`palette::strained` multiplies the owner's colour down by it. Without something
on the screen a wall that gives way has, from where the player sits, given way
at random — which is the failure `batter_buildings` already had and the reason
this milestone is three changes and not one.

**Two tests were about an old world.** `wood_gives_way_before_stone` used a
dike as its stone exemplar, and a dike is no longer battered at all; the only
other stone thing a player can place is a road, which design §6 wants the flood
to break. The comparison is now made where it lives (`RESIST_STONE` against
`RESIST_WOOD`) with the water still saying the half a placeable building can
answer, and `only_a_dike_is_pressed` holds the new rule next door.

The five-strategy playtest is unchanged in outcome — one survivor for `dike`
and one for `both`, the same as before the pressure model — while the stone
left standing at the flood drops from 420 to 210–330, which is segments being
lost. A model that changes what a wall costs without changing whether a wall
works is the right thing to have at this point in the plan.

---

## 2026-08-30 — A river through the middle of it

M4, and the largest change since the MVP. The terrain is still a corner-to-
corner ramp with noise on it — that is what keeps "high ground is safe" true —
and a channel is now cut down it from the high side to the low, entering and
leaving on opposite edges of the map. Cities go on its banks. The flood comes
down it.

**A river is water because it is a river, not because it is low.** The plan
said to carve the channel before the ground bands are computed so it "counts as
the shallows it is", and measuring that says it does not: the bands are
percentiles of the height field and a channel running down a ramp is above the
waterline for most of its length however deep it is cut. `what_the_river_costs`
found three seeds in eight with two fifths of their channel reading as dry
land. So the floor is painted `Shallows` outright — the one place besides
`level_pad` where ground is not a function of height, and deliberate in both.

**The bed is cut from a smoothed profile, and that took the arrival probe to
find.** A running minimum on the raw terrain is a trap: the land along a
meander is noisy (`NOISE_AMPLITUDE` is 16 against a relief of 40), so one
hollow drags the bed down and — because a minimum never comes back up — every
reach below it is cut to that depth. Seed 31's channel crossed a hollow early
and spent the rest of its length as a canyon at height zero beside a city
standing at thirty. The surge filled the canyon and the city never got its feet
wet. That is not a flood, it is a moat. The bed is now cut from the terrain
averaged over `RIVER_SMOOTH` cells and then made monotone.

**`Ground::Ford`, and `nav::passable` learns its fourth rule.** Every map
guarantees one reach shallow enough to wade: passable, half speed, six times
the pathing cost of open ground, unbuildable except by a bridge — crossable
without one, better with one, exactly the relationship that was asked for. And
it closes when the water comes: `passable` asks the water's depth at a ford and
nowhere else, at the same depth a citizen starts wading at, so "you cannot path
across it" and "you would be swept off it" arrive together.

The ford covers the whole width of the cut and not only the channel floor. The
bank taper is cut low too and on a low-lying reach the bands make those cells
shallows as well, so a ford the width of the floor stops two cells short of dry
land on each side — which is to say it is not a crossing. That was worth ten
minutes and a flow field that reached a third of the map.

**Sites are chosen farthest-point from a band, not offset from a line.** Two
earlier versions offset a site perpendicular to the channel and both were wrong
for the same reason: a meander means the line a site was placed from is not the
nearest water to it, and a site fourteen cells out along one reach was one cell
from the next bend. The band is every cell within `SITE_JITTER_BAND` of
`SHORE_DISTANCE` from the river; players take turns picking the cell in it
furthest from everybody already placed, on alternating banks. Spacing by
construction rather than by hope.

**Three things the band has to know about, each found by a test failing:**

* **Rock.** Rock is the top eight percent of the height field, in one mass at
  the high corner, and a river bank is low country — so 46% of two-player
  sites had no rock within forty cells and the worst was a hundred away. A
  quarry has to be cut out of rock and a quarry is the only source of the stone
  a dike costs, so that is a city that cannot defend itself, decided by the
  generator. Filtering on rock cost far too much spacing (two cities one cell
  apart at six players), so the choice *ranks* — spacing until it is enough,
  then rock, then more spacing — and the generator plants a small outcrop
  within reach of any city that still has none. That fires for roughly half the
  cities, which is a lot; the better fix is low-country outcrops in the terrain
  model itself, and it is written down here rather than done.
* **Being walled in.** A city can sit in a pocket of the map that its own
  hearth, farm, granary and cottage then seal off, and
  `two_cities_found_a_road_and_trade_for_three_days` found one: no road could
  be laid between two cities that were both, technically, reachable. Candidates
  must now be in the map's largest non-rock region *and* have
  `SITE_ELBOW_PERCENT` of the ground around them open.
* **Headroom.** The flood fills the channel to about `Disaster::height` above
  the bed and spills from there, so how high a city stands above its own reach
  of river is what decides whether the water reaches it at all — the job
  `SHORE_DISTANCE` used to do alone when distance was the only thing that
  mattered. Sites ranged from fourteen below the bed to seventeen above, and
  `when_the_water_arrives` found one city in six that never got its feet wet in
  three ages. Bounded to −4..+12 against an age-one surge of 12, swept together
  with `SITE_JITTER_BAND` because the two of them decide how much there is to
  choose from.

**The surge comes out of the channel's upstream mouth, and the sea is at the
other end.** `Disaster::sources` stopped being corners: there is one mouth, so
what varies between ages is how many pulses come down it and when. The first
`SURGE_REACH` cells of the cut are held at the age's height and the next
`SURGE_REACH` at half — the same volume-and-shove the corner version used, with
a channel to go down.

`sea_surface` is measured at the river's *outfall* and not at its source, and
getting that wrong was instructive: with the sea taken from the high end
nothing could drain anywhere, and the flood sheeted out over fifteen thousand
cells of sixteen thousand at eleven sixteenths a cell. A damp map, not a flood.

`SURGE_REACH` is forty, measured:

| reach | age 1 peak | age 3 peak | wades at | dry again |
|---|---|---|---|---|
| 20 | 25–43 | 35–71 | 115–800, sometimes never | 465–2530 |
| 40 | 72–82 | 105–115 | 71–137 | 1380–3079 |
| 80 | 79–125 | 125–171 | 55–135 | 1622–3835 |

Wading starts at 32 and swimming at 96. At twenty the flood is a damp patch; at
eighty an age-one flood is already over your head and the escalation has
nowhere to go. Forty is where age one wets you and age three drowns you, which
is design §4's table read back out of the water.

### What this leaves for M5, said plainly

**The river flood is too gentle, and the numbers say so.** The five-strategy
playtest went from one survivor to sixteen for `grow`: two seeds in three now
survive to age three doing nothing defensive at all. `dike` scores thirteen —
*worse* than doing nothing — because a wall costs 450 builder-ticks a segment
and the tallest anybody finished before the age-one flood was one segment. A
game where the wall is not worth building is the thing M5 exists to fix, and it
now has three probes to fix it with: `when_the_water_arrives`,
`how_far_the_water_reaches` (re-pointed at the bank rather than a corner) and
`dike_pressure_on_flat_ground`.

One city in twelve still never wades. Everything else about the placement is
guaranteed; that one is a tuning number.

### Tests that were about an old world

Seven, and each says so in its own comment: the wet ground is at the low end
*excluding the channel*, the cities are comparably near *the river*, the flood
comes *down the river*, the building in the front of the surge stands *beside
the river mouth*, the crowd test *bunches its own citizens* rather than leaning
on where a founding party happens to land, the water tests put their walls in a
*clear column*, and the road tests *search for* a pair of ends a road can
actually join instead of aiming two cells east of a hearth.

---

## 2026-08-30 — Which dikes break, and what a wall is allowed to cost

M5. `dikes::which_dikes_break` walls both banks of the river at three distances
across ten seeds, alternating level one and level two, and reports what the
flood takes. `DIKE_STRESS_LIMIT` is set from it and the table lives beside the
constant.

At `[15_000, 48_000, 90_000, 145_000]` an age-one flood takes 71% of a
level-one wall and leaves 79% of a level-two one standing — both in the middle
of the plan's target — and by age three it is 82% and 61%. The gradient with
distance is the part worth looking at: 82% of a level-one wall on the bank goes
and 59% of one twenty cells back, which is the choice the drag tool exists for.

**Seven seeds in ten hit both bands; the plan asked for eight, and the residual
is not the number.** `[20_000, 55_000, …]`, `[16_000, 52_000, …]` and
`[14_000, 50_000, …]` all measured seven as well. The three that miss are maps
whose flood is unusually weak or strong, not walls behaving oddly. Two attempts
at narrowing that are recorded here because the next person should not repeat
them:

* **Holding the surge's surface rather than its depth.** Kept — it is the
  better model, since a river in flood rises to its banks and over rather than
  to a fixed number of sixteenths wherever it happens to be. It raised the mean
  and did not narrow the spread.
* **Capping the water on the ground.** Dropped. It looked right — the map that
  *poured* the least held the most water and vice versa, because a source that
  holds a depth stops asking once its neighbours are full — but capping the
  volume moved every seed together and left the same two outliers. Machinery
  that does not earn its place does not stay.

**A dike had to be given a footing.** A hard threshold sitting in the middle of
the load distribution is exactly where the fraction broken is most sensitive to
the load, which is why the same rule gave 67% of a level-one wall gone on one
seed and 93% on another. `FOOTING_SPREAD` draws each segment's toughness within
25% of the book figure when it is placed, and keeps it on the building so two
peers cannot disagree about which stretch was the weak one. No two banks are
alike, and without this they were.

**And a wall had to be made affordable, which is the finding that matters.**
The playtest measured the old price and the answer was the whole run: a wall
long enough to shield a city is about forty cells, at 150 builder-ticks a cell
that is six thousand builder-ticks, half the city stands on the bank for two
days a long walk from the granary, and **every `dike` run died before the water
arrived**. So:

* `Kind::Dike.build_ticks()` is fifty a cell, not a hundred and fifty. A bank
  of earth is not a house — a cottage is two hundred for four cells.
* `DIKE_RAISE_PERCENT` makes raising a level half the work of building one,
  because adding a course to a bank is adding to something already there.
  Design §3.3 has dikes grow; this is what growing costs.
* `playtest.rs`'s dike strategy now *mans* its wall, raises it to level two and
  no higher, and orders it only once the farm and the granary are standing —
  three things its own comments already claimed and its code did not do.

**On one seed in three, a diked city now visibly outlives an undiked one** —
`both` finishes three ages with all eight alive against `grow`'s six and
`flee`'s none. That is the first time that has ever been true in this repo.

### What is left, and why it is parked

On the other two seeds building the wall still costs the city the run. The
cause is measured and is not a bug: the labour has to come from somewhere, and
those maps have no slack in the first age. Fixing it is a question about the
*food* economy rather than about dikes — how much a farm yields, how many
hands a city of eight can spare — and the honest way to answer it is to watch
two people play, which is M10.

So M5 stops here. Nothing in M6 through M9 touches the flood: gold and mules,
levels and moving, job icons and workers indoors, and families are all
independent of it, and the plan says so of M6 and M7 in as many words.
**Finishing the balance belongs immediately before M10**, where a real run is
the instrument, and it is listed there rather than left implicit.

---

## 2026-08-30 — Gold, a trading post, and a cart that is not a person

M6. `Good::Gold` went in first and on its own, before anything depended on it,
which is what the plan asked and was right: `covers`, `total` and the GUI's
`cost_line` now walk `Good::ALL` instead of naming three fields, so a fifth
good cannot be silently left off a price tag.

**Gold is a departure from design §6's "there is no market, no price and no
currency in version one", taken deliberately.** Barter is untouched —
`Command::Trade` is still a standing daily exchange two players agree on and
haulers walk — and gold is what a post's mules earn, which is a different thing
in a different place.

**Gold is minted by the exchange, not moved between players.** The other city
does not pay out of a purse it has not got: nothing else in the game makes
gold, so a first trade would be impossible if trade only moved it. A mule hands
over ten wood and comes home with five gold that did not exist before. The wood
is real and leaves the seller's store; the coin comes from outside the map, the
way a coin does.

**Gold is not hauled.** `Good::hauled` says so and the barter dialog will not
offer it. A city whose haulers spent the day moving coins between the hearth
and the stockpile would be doing nothing useful very busily; gold is kept where
the mule left it and spent from there.

**A mule is its own entity and not a citizen wearing a hat**, as the plan asks.
A citizen has hunger, rest, a home, a job, a crowd around it and an errand it
can abandon; a mule has a position, a destination, a load and one bit for which
way round the trip it is on. Making it a citizen would have meant six rules
that do not apply and one that does. What it shares is the half that matters —
the same flow fields, so it finds its way round the river like everybody else,
and road speed on a road, which is the first thing in this game that makes
laying a road between two cities pay for itself.

**Three things this cost that were not obvious:**

* **A mule is spawned inside its own post, and a post blocks movement.** The
  cart stood in the yard for ever and the whole feature did nothing, silently.
  It spawns in the yard now and falls back to `step_off_a_building` — the same
  escape a citizen born inside a hearth uses.
* **`Building::deliver` is for construction sites only.** A mule handing wood
  over at another city's hearth is not building it anything, so the delivery
  returned zero and the load went home again. `Building::stow` is the
  counterpart, written once and used by both ends of the trip.
* **`Job::produces` was answering the wrong question.** A trader makes nothing
  — the gold is earned on the road — but it *stands at* its post and holds one
  of its slots, and the arm that puts a worker on a roster was guarded by
  `produces`. So a trader arriving at its post fell through to "there is no job
  here", had its workplace cleared, and left a mule on the road belonging to
  nobody. `Job::stationed` is the question that was meant.

**A cart with nowhere to go says so.** `Leg::Stuck` is a state rather than a
mule standing still: it is ringed in red on the map and the panel reads "a mule
has nowhere to take its load: no other city it can reach". With a river between
the players this stops being a corner case — it is what a player sees before
they have a bridge or found the ford.

`MULE_PAY` is provisional and says so. What a round trip is worth cannot be
settled until M7 has priced an upgrade, because gold buys levels and a level is
one more pair of hands.

---

## 2026-08-30 — A level is one more pair of hands, and a building can be moved

M7, and it is two rules rather than two features.

**A level is one more citizen the building can hold.** One sentence, no
per-kind arithmetic: a farm goes three hands to four, a cottage four beds to
five, a post two traders to three and so two mules on the road to three.
`Building::slots_for` and `Building::beds` add `level - 1` to whatever the kind
says, and every reader was moved off `Kind` and onto the building — `assign`,
`will_take`, `will_house`, `SetHome`, the roster in `jobs.rs`, and the panel's
hover line.

**The plan's table has a row that cannot be honoured as written**, and this is
the departure: "granary · stockpile · hearth — one more hauler based there". A
hauler in this codebase is based *nowhere*. `slots_for(Job::Hauler)` has no
limit and `assign` deliberately gives a hauler no workplace, because a hauler
goes where the work is. A level on a store would therefore buy nothing a player
could see, and a level a player pays for and cannot see is worse than a level
they cannot buy. So `Kind::upgradable` sells levels only where hands actually
go — farm, forester, quarry, cottage, trading post — which keeps the
one-sentence rule true rather than nearly true. The hearth is out because the
plan says so, and a dike's levels are height bought with stone: the flood
currency stays separate from the trade one.

**An upgrade does not put the building back to a site.** Raising a dike does,
because a dike is being made taller; a farm is being given another pair of
hands, and a farm that stopped feeding anybody while its fourth farmer was
hired would be a strange thing to sell. It is paid at once out of the city's
stores, because gold is not hauled.

**A move keeps the id, and that is the whole trick.** Everybody who worked or
lived there is still pointing at the same `BuildingId` and simply re-paths to
the new address — no new machinery at all. It keeps its store and its level,
and it arrives as a construction site with its materials already delivered, so
the move costs builder-ticks and no materials. Being a site while it moves is
the price: it shelters nobody and produces nothing until it is finished, which
is what makes moving the granary the day before the water comes a decision
rather than a free tidy-up.

`can_move` is split from `move_building` the way `can_place` is split from
`place`, so the ghost under the cursor can ask without issuing a command that
will be refused — and it ignores the building's own cells, or a building would
refuse to shuffle one step because it is already standing in the way.

Dikes are movable. The plan's table does not say either way, and a wall in the
wrong place is the most expensive mistake in this game: being able to shift it
is worth more than the tidiness of a shorter rule.

`UPGRADE_GOLD` and `MULE_PAY` are one number really — how many round trips a
pair of hands is worth — and both say they are provisional. Ten against five
means two round trips buys the first level and four buys the second, which is
about an age of trading for a farm that feeds one more mouth. Nobody has played
it.

---

## 2026-08-31 — Six silhouettes, and a way into the building

M8. Two changes, and the second one is not only cosmetic.

**The outline carries the job, not the colour.** The colour is already spoken
for — it is whose city this is — and a second meaning on it would make a
two-player map unreadable. So each of the six is a different *shape* over the
head: a scythe leans, a saw is level, a pick is a wedge, a hammer is a block, a
trader has a purse, and somebody unassigned carries nothing. Different shapes
rather than different lengths of one, because they have to be told apart at
eight pixels a cell as well as up close.

**`nav::passable` gains its exception, and it is one rule: you may go inside
the place you are going to.** The plan asked for "somebody whose workplace is
that building", which a shared flow field cannot express — one field serves
everybody walking to a granary, and a hauler with a delivery is not employed
there but should still walk in at the door. `FlowField::build_into` takes the
destination and opens that building's own cells; every other field still finds
it solid, which `a_field_is_seeded_at_the_middle_and_reaches_the_whole_footprint`
holds by checking a field aimed elsewhere cannot get through.

**The field is seeded at the middle rather than over the whole footprint**, and
that was the actual bug behind three farmers being one circle. Seeding every
cell made the field *flat* across the building: the first cell anybody touched
was as good as any other, so they all stopped on whichever corner the field
reached first. One goal in the middle gives the inside a gradient.

**Arriving is still at the building and not in it, and getting that wrong cost
an hour.** The first version made a worker arrive only once it was on the
footprint, which reads exactly like what the plan asked for and starved two
cities: a farmer that has not "arrived" is still `Walking`, and a city of people
permanently on their way somewhere eats and sleeps at the wrong times. What
puts a worker inside its farm is the crowd — a one-step drift toward the middle
for anybody `Working` at a standing building they are not standing on, in the
pass where every other question about position is answered. The elbow-room push
then spreads them through the inside, which is the part that stops three
farmers being one circle.

**The crowd's wall rule gained the exception rather than losing the rule.**
`nobody_ends_a_tick_standing_in_a_wall` still holds; `indoors_here` is what it
asks now, and it says yes to the inside of the building you work at or are
walking to and no to everything else.

**And a bug from M4 fell out of it.** `lay_road` bridged `Ground::Shallows`
exactly, and a ford is water too — so a road routed over the ford laid road
cells on it, every one was refused for standing on water, and the road came out
with a hole in it that nothing reported. `Road::intact` then said the cities
were not linked and the trade moved nothing. It bridges anything `watery` now.
`two_cities_found_a_road_and_trade_for_three_days` also had to be wound back to
the second day of the age: founding two cities and laying a road across a river
takes four days of simulated time now, and the test's third day of trading had
quietly become the impact day, so the flood took the road and the failure read
as "trade moved nothing" rather than "the water broke the road".

---

## 2026-08-31 — Families, children, and the one thing that adds a citizen

M9, and the largest addition to `sim` since the MVP.

**Being fed is the gate, and it is a gate rather than a brake.** A hungry day
sets a household's progress back to nought rather than pausing it, because a
gate that only slows you down is not a gate. `families::how_a_city_grows` is
the measurement: a fed city of eight is ten by the first flood and twelve by
the second, and a hungry one never leaves eight. That is what makes the granary
decide the *size* of a village and not only whether it survives.

**No nursery, no children.** A child is born into one and takes a place there,
so it is a building a player chooses to put up rather than a rule that happens
to them — and a full nursery is a city that has decided how big it wants to be.
A level buys a nursery one more place, which is the same one-sentence rule M7
sells everywhere else: a level is one more citizen the building can hold, and a
child is a citizen.

**The pairing has to be a function of the world.** Two adults sharing a cottage
become a household, and a cottage with five beds and five people in it is not
two and a half families: it is one household and three lodgers, and *which two*
is decided in id order on every machine. Two peers that disagreed about who
married whom would desync on the next child.

**Appending only, and it is the only place a citizen is ever added.** Ids are
indices into `World::citizens` in half a dozen places and the crowd, the flood
and every roster iterate it, so `bear_a_child` pushes and nothing anywhere
reorders or reuses. `two_peers_raise_the_same_children` runs ten thousand ticks
with births in them and compares checksums every five hundred.

**A child is a citizen in every way but work.** It does not haul, farm or
build — `assign` refuses it with "too young to work" and `find_work` sends it
to its nursery — and it eats, it can be ordered uphill, and the flood does not
care how old anybody is. It comes of age on a tick it was born knowing, two
ages later, so a child born in the first age is working by the third and one
born just before the last flood never works at all.

**The households tab is the first thing in this game that connects a list to
the world.** One chip per household with the two names, how many children and
how close the next one is; hovering it rings those people on the map in a
wider, warmer ring than a selection's — a different question ("where are these
people") deserving a different mark. It is only useful because M2 put a camera
over the map to see them at.

---

## 2026-08-31 — M10 is played at ten ticks a second, on the deployed build

`HANDOFF-M10.md` leaves this open and asks for it to be chosen deliberately: a
run is three ages of six days of 1 200 ticks, which at `TICKS_PER_SECOND` is
thirty-six minutes of wall clock, and the alternative is a test-only multiplier
on `Clock::ticks_due`.

**There is no multiplier.** The choice closes itself once both halves of the
milestone are written down together. The done-condition names *the deployed
build*; a multiplier lives in `crates/gui/src/main.rs` and must never reach a
shipped build. Both cannot hold — to multiply the clock on the deployed page
you have to deploy the multiplier. The escape is to run both peers on a
matching local build, which the build-hash guard permits, but then the one
artefact nobody has ever played is still the page a player opens, and that is
the whole milestone.

And it would compress the variable under test. A day is two minutes of thinking
time. At four times the clock it is thirty seconds, and "was the wall worth
building" is partly a question about whether a city can afford the *attention*
and not only the labour — which is exactly what M5 parked for a run to answer.
A multiplier would not make the agents decide faster; it would only give them
less time to.

The cost is accepted rather than worked around: thirty-six minutes for the run
and about twelve for the rehearsal, both of them spent watching. The polling
cadence that follows from it is about one look every twenty-five seconds
through a quiet day, tightening to five or ten seconds on day six of each age
— roughly ninety looks each over a run.

---

## 2026-08-31 — Two browsers, because thirty seconds of not rendering is a drop

The handoff asks for two browser *contexts* rather than two pages in one, so
that two agents do not share a clipboard, a `localStorage` or a permission
grant. `two_agents.py` uses two whole browsers instead, for a reason that only
appears over a long run and that nothing in the repo had written down.

`Lockstep::DROP_AFTER_TICKS` is 300 ticks — thirty seconds — and it is counted
in the *waiting* peer's own ticks, so it is thirty seconds of wall clock at ten
ticks a second. `Clock::MOST_PER_FRAME` is 8, so a page must render at least
1.25 frames a second to hold the rate at all, and a backlog past eight ticks is
dropped rather than caught up. Chromium throttles animation frames in
backgrounded and occluded pages, and two pages in one browser cannot both be in
front. A run would therefore be decided by which tab Chromium thought was
visible, and the symptom — one peer dropped twenty minutes in — would look
exactly like a network fault.

So: one browser each, launched with `--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` and
`--disable-renderer-backgrounding`. Playwright passes all three itself today;
they are named in our launch anyway, because the reason we need them is ours
and not Playwright's to keep.

`two_agents.py` also checks something `game_two_tabs.py` does not: that both
tabs are still ticking three seconds after they arrive. A page that stopped
rendering draws the same map as one that is playing, and looks correct right up
until it is dropped.

---

## 2026-08-31 — An agent's hands are one-shot commands, and it is told what a player knows

Two things about the shape of the playtest, decided before it starts because
both of them are easier to get right than to fix halfway through a run.

**The hands.** An agent's turns are separate processes and it cannot hold a
`sync_playwright()` session open across them. So the browsers are launched once
with their own `--remote-debugging-port`, and every action an agent takes is a
short `connect_over_cdp`, act, screenshot, disconnect. The state lives in the
browser, which is where it already lives. Each agent is given exactly one port
and never learns the other's — which turns "neither may read the other's page"
from a promise into a property of the setup rather than a rule somebody has to
keep.

**The briefing.** An agent is told what a player could know: the controls, the
goods, the buildings, the deadline, the river and the ford — the first-run card
and the manual, written down. It is told nothing from `crates/sim`: no balance
constants, no source, no probe tables, no idea which dikes break. Otherwise the
run measures an agent's reading of `balance.rs` rather than the game, and design
step 7's question — whether this is *fun* — cannot be answered by somebody who
has read the answer key.

The referee is the exception and is not a player: it reads both panels on a
schedule, issues no input, and detects the desync banner as red in the status
row, the way `assign.py::alarm_band` detects a refusal. It is an instrument.

---

## 2026-08-31 — What "peers at" was, and the two things left beside it

Setting up M10.1 meant reading the panel's bottom rows in a real browser for
the first time, and the row the handoff calls the instrument to watch — "`peers
at` is the two peers' tick counts side by side" — showed `peers at [74]`. One
number, its own, identical to the `tick` row directly above it.

It was true of the native build and only of the native build. `Session::ticks`
maps over `Local::steps`, and natively every peer's `Lockstep` is in this
process, so `make` really does show several numbers. In the browser a page has
exactly one `Lockstep` and the arm read `vec![w.step.tick()]`. Every browser
that has ever run this game has drawn a row that says nothing, and it went
unnoticed because the two tabs that have played together were both watched by
somebody who knew what the number ought to be.

`Lockstep::peer_ticks` replaces it. The host keeps `seen_at`: the last tick each
player reported a checksum for, which arrives on `Turn` and is already being
recorded a few lines away for the desync check. It shows `peers at [74, 71]` —
its own simulated tick and everybody's last reported one. The gap is the
pipeline, not a fault: what a peer reports is a round trip old and `DELAY` ticks
behind besides. A steady gap is health, a growing one is a peer falling behind,
and one that stops moving is a peer that has stopped. A joiner still shows one
number, because it is genuinely told nothing else.

**Two things were found with it and deliberately left alone.**

**Only the host can notice a desync.** A checksum rides on `Turn`, which every
peer sends to the host and to nobody else, and `check_agreement` is host-only —
"the host is the only peer that sees them all, which is why it is the one that
notices", as the comment there says. A joiner's status can therefore never
become `Desync`, and `a_peer_whose_world_differs_is_caught_and_the_game_stops`
only ever asserts on `steps[0]`. So M10's done-condition — "neither client ever
showed a desync banner" — is half a statement: the joiner's silence proves
nothing, and the referee must read the *host's* status row to know.

**And on a desync the joiner freezes without being told.** The host sets
`Desync`, `is_stopped` makes `advance` return early, bundles stop, and the
joiner sits on "playing" for ever. Two people playing, one is shown the fault
and the other's game simply stops.

Both are real and neither is fixed here. The second is a shipped failure path
that has never fired in production, and changing what it does is a decision
about the game rather than about the playtest; making it during the setup for a
run, on evidence from reading rather than from playing, is the kind of change
this project has twice found reasons to regret. They are written down so the
account has them, and M10.8 is where they are answered — with a run behind them.

---

## 2026-08-31 — The soak, and why it can only be eight minutes long

M10.4's ten-minute soak found that a ten-minute soak is not possible, and the
reason is the game rather than the harness.

The measurement, on the deployed build, two browsers, nobody playing:

    10:07:00  watching 2 peers for 900s, a look every 5s
    10:08:50  city 0 turned a day (now 1); city 1 turned a day (now 1)
    10:10:51  city 0 turned a day (now 2); city 1 turned a day (now 2)
    10:12:51  city 0 turned a day (now 3); city 1 turned a day (now 3)
    10:14:37  city 0 IS OVER - the score screen is up; city 1 IS OVER
    10:14:37  city 0: 3 days in 7.6 min, 0 samples with no tick, 0 red, ended
    10:14:37  city 1: 3 days in 7.6 min, 0 samples with no tick, 0 red, ended
    10:14:37  CLEAN

**The clock is exact.** A day is nominally two minutes and the three turns came
121 and 120 seconds apart. Nothing throttled, nothing drifted, neither peer
ever drew no tick, and the status row never went red — for the whole life of
the game, on two browsers with the flags of the previous entry.

**And the whole life of an unattended game is about eight minutes.** Nobody
feeds anybody, so both cities starve on day four and the run ends. A longer
soak cannot be run without playing, and playing is the rehearsal, not a soak.
That is the honest limit and it is written here rather than worked around: the
harness is proven for as long as there is a game to prove it against, and
M10.5 extends the evidence by keeping the cities alive.

It matters for M10.6 too. Thirty-six minutes is three ages, and an unattended
city does not last one. Both agents have to be feeding their people inside the
first four days or the run ends on its own, and a run that ends that way is a
finding about the food economy rather than a failed setup.

**The first soak reported twenty-six failures for a game that had finished
normally.** An ended game and a stopped page are identical at the tick row —
which is the only place the difference matters and the only place it cannot be
seen. `still_playing` tells them apart at the **tab row**: `main.rs` draws the
score screen *instead of* `panel_layer` when the run is over, so the tabs go
with it. Measured at 847 lit pixels live and none once it has ended.

The tab row and not the build menu, which was the first attempt: a player
merely *looking at the households tab* has no build menu either, and reporting
"the game has ended" halfway through a run because somebody checked on their
families would have been worse than the fault it was meant to catch.

**One more thing that would have made the referee useless.** `Status::Desync`
is drawn in `palette::ALARM` (230, 84, 71) and `Status::WaitingOn` in
`palette::WARNING` (240, 184, 71) — in the same row. `assign.py::alarm_band`'s
"is it reddish" test matches both, and it is right to, because it is looking at
a row where only a refusal is ever coloured. Here it would report a desync
every few seconds, since waiting on the other peer is what lockstep does all
day. `alarm_pixels` separates them on green.

---

## 2026-08-31 — The panel names the clock, not just the mechanism

The M10.5 rehearsal killed both cities in age one and neither death was the
water. The plan puts the run's findings in M10.8, *after* the run; this one was
pulled forward because it is the reason a run to age three could not happen.

The amber line is the most trusted thing on screen — both agents said so
unprompted, one calling it "the best thing in this game" — and it said "the
granary is empty - give the farm a moment", unchanged, for two days while eight
people starved beside a farm staffed three-of-three. It was correct. It named
the mechanism and never the clock, so neither player could tell "a day too
slow" from "the food is not moving at all", and both of them separately asked
for the same number afterwards.

    1 food left, and 8 mouths eat 96 a day - under a day
    the granary is empty. 8 mouths eat 96 a day - more farmers, or fewer
    hands carrying stone

**Twelve units a citizen a day is derived, not chosen.** `FOOD_A_DAY` is
`TICKS_PER_DAY * FOOD_DECAY / FOOD_PER_UNIT`: a need falls a point a tick and
one stored unit fills a hundred of it. The comment on `FARM_TICKS_PER_UNIT` has
said "about twelve units a day" since phase 1; this writes it down as a number
the game can do arithmetic with rather than one only the author knew.

`days_of_food` returns `None` for a city with nobody left in it, which is not
the same answer as nought days and must not read like it — telling a player
"0 days left" about people who are already gone would be worse than the silence
the panel keeps now.

**`larder` is its own function so it can be tested at the width a big city
makes of it.** `draw::panel` wraps the line at 52 columns and takes two rows;
a third is dropped without a word. The food lines are the only ones here
carrying numbers, so they are the only ones that grow when a city does, and a
city of ninety eating four digits a day is what the test checks.

**A working building now says what is standing on it** — `farm: 3 of 3
working, 1 food waiting`. That row used to read identically for two opposite
situations: a farm just emptied by a hauler, and a farm nobody is carrying
from. One player spent two days unable to tell which it was watching, and it
was the question that killed its city.

Nothing about the balance changed. No constant was retuned, no yield altered,
no rule about how food is made or moved. This is the panel saying out loud a
number the simulation already had — which is what M5 meant when it said the
food economy needed a person to look at it, rather than a probe.

---

## 2026-08-31 — The run, and the row it could not read

M10.6 finished: three ages, eighteen days, 35.4 minutes, "The map stood", both
cities alive and mauled, **no desync on either client at any point**. The clock
held to 119.8 seconds a day against a nominal 120 across the whole run.

**The referee said NOT CLEAN and was wrong, which found a real bug.** It
reported city 0 drawing no tick 116 times while days kept turning on schedule —
two things that cannot both be true. The cause: `Input::offers` and the
level/move row grow *past the foot of the panel* and overdraw `tick`,
`peers at` and `build`/`seed`. The referee spent twelve minutes reading
`city 1: 20 food for your 20 stone` where the tick count should have been.

Those three rows are the ones a player is told to look at when something is
wrong, and the ones M10 nominated as the desync instrument. **Part of this is
mine**: moving the level/move row below the offers put up to forty-eight more
pixels on a stack that already overflowed by design — the offers alone start at
y≈880 and the foot begins at 890. Recorded here rather than fixed, because it
is a panel-layout decision and the panel has moved six times; whoever fixes it
should decide whether the foot is drawn last, whether the variable stack gets a
hard ceiling, or whether the panel is simply out of room.

**A raise that works looks exactly like nothing happening.** `raise_dike` adds
a level and returns the segment to a site, so the hover row stops saying "level
1 of 4" and starts saying "being built" — a player who clicks to check whether
their click landed sees strictly less than before. A refused raise says "it is
not built yet" in red and `NOTICE_SECONDS` removes it after four and a half
seconds. Between the two, city 1 played a whole run believing the raise was
broken, and its verdict on walls is therefore "a level-one wall is not worth
building" rather than the wider claim. **Nobody has ever tested a level-three
wall in a played game**, because the game makes levelling undiscoverable.

**`day 7 of 6` on the final frame.** `day_of_age` is
`(tick - age_start_tick) / TICKS_PER_DAY + 1`; at the last tick of the last age
that is seven, and the world has finished so the age never rolls over.

**The finding that matters is not a number.** Both players, independently,
spent their stone by guessing and both said the same thing afterwards: there is
no way to ask how high a cell is or how far the water came last time, in a game
entirely about water height. City 0 planned its third age by screenshotting the
second flood at its peak and noting which pixels stayed green — "that is
reading the renderer, not playing the game". **The wall is not underpowered; it
is unreadable.** M5's balance work stands; what M10 found is that the decision
it balanced is one the player is asked to make blind.

---

## 2026-08-31 — The clock doubles, and two constants stop being written in seconds

M11.1. `TICKS_PER_SECOND` is 20. A day is a minute, an age six minutes, and a
three-age run eighteen rather than thirty-six.

**It is a wall-clock knob and nothing else** — it appears in no rule in `sim`.
Everything the simulation balances is counted in ticks against
`TICKS_PER_DAY`, so at twice the rate the same game is watched twice as fast
and nothing about the game changes. That is measured, not asserted:
`three_full_runs_of_each_strategy` is identical either side of the change.

```text
                 survivors                        tallest wall by flood 1
  10/s   idle 0  grow 8  dike 8  flee 4  both 0          60 stone
  20/s   idle 0  grow 8  dike 8  flee 4  both 0          60 stone
```

**Two constants had to be pinned first, and finding them is the whole of this
entry.** `SURGE_TICKS` was `30 * TICKS_PER_SECOND` and `DROWN_TICKS` was
`5 * TICKS_PER_SECOND`. Both *read* like wall clock and neither is: what
decides a flood is how much of the **day** the source pours for, and a day is
`TICKS_PER_DAY` whatever the clock does. Left alone, doubling the rate would
have poured for six hundred ticks instead of three hundred — half a day rather
than a quarter, twice the water — while looking like a change to nothing but
the frame rate. They are 300 and 50 now, which is exactly what they meant at
ten a second, so the pinning itself changed nothing on the day it was made.

They had to be pinned *together*. A body lasting a sixth of a surge is the
relationship that matters, and scaling one without the other would have moved
it quietly. `ages::the_clock_can_change_without_changing_the_game` asserts both,
and asserts the ratio, so the next person to tie one back to the clock finds
out from a test rather than from a playtest.

**Three others are genuinely counted in seconds a person waits and still scale
with the clock**: `DROP_AFTER_TICKS` is thirty real seconds, `WAIT_WARN_TICKS`
five, `PING_LIFETIME` three. `lockstep::the_timeouts_are_counted_in_seconds_not_ticks`
is the other half of the same statement, in the crate that owns them.

**And one number in `gui` moved with it.** `Clock::MOST_PER_FRAME` is 16 rather
than 8. It is counted in ticks but what it caps is *wall clock* — how much a
stalled frame may make up — so it doubles with the rate. The floor it implies
is the number that matters and is unchanged: a page must render at least 1.25
frames a second to hold the tick rate, which is the figure the "two browsers"
entry above quotes as the reason an agent gets a browser to itself.

**Numbers in earlier entries that have moved.** `DROP_AFTER_TICKS` is 600 ticks
now, not 300; it is still thirty seconds, which is what those entries were
actually claiming. `referee.py` gained a `DAY_SECONDS` constant because it
computed expected day-turns as `minutes / 2` and would have called every
healthy run late. `AGENT-BRIEF.md`'s "a day is two minutes, a run is thirty-six
minutes" and its polling cadence moved too — an agent told to look every
twenty-five seconds is looking half as often once a day is half as long.

**What was not done, and why it is written down.** Doubling `WALK_SPEED` is the
obvious way to make people move faster. It is safe as far as 128 — a road
doubles it and 256 is a whole cell a tick, which is where a citizen starts
stepping over walls that `nav::passable` never gets asked about — and all 280
tests passed at it. But the same five-strategy table takes the tallest wall a
city can raise before the first flood from 60 stone to **540**, because haulers
carry stone nine times better, and drops `flee` from four survivors to one.
That is not a frame-rate change, it is M5 undone, and the measurement is here so
nobody has to rediscover it.
