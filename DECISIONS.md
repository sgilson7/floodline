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
