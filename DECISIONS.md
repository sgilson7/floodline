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
