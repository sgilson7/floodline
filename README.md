# FLOODLINE

A browser multiplayer medieval city builder. Two to six players share one map,
each founding a city on it, linked by roads they lay between them and by barter
that haulers actually walk. At the end of every age a wall of water comes out
of one corner and takes whatever was not behind a dike. The run ends when the
last city is gone; the score is how many ages the map stood.

Rust and WASM, drawn with rectangles and lines. Deterministic lockstep over
WebRTC — only commands go over the wire, so six browsers can share five hundred
citizens and a flood on a few kilobytes a second.

**Status: phase 0 of `floodline-mvp-plan.md`.** The page builds and deploys;
there is no game in it yet. `PROGRESS.md` says exactly where things stand.

## Play

Not yet. When there is something to play, it will be at the Pages URL with a
`?room=` code you share with a friend.

## Run it locally

    make            # native build
    make test       # the whole suite, no window
    make web        # browser build into dist/web/
    make serve      # …and serve it at localhost:8080
    make help       # every command

Needs stable Rust ≥ 1.88 and the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`).

## Determinism, in one paragraph

Every peer runs the same simulation from the same seed and applies the same
commands on the same tick; nothing but commands is sent. That only works if the
simulation is bit-identical everywhere, so `crates/sim` depends on `serde` and
`postcard` and nothing else: no floating point, no `HashMap` (iteration order
is a decision, so it is always `Vec` or `BTreeMap`), no clocks, one `Rng` living
in `World`. Every tick each peer sends a 64-bit checksum of the world as it was,
and a mismatch stops the game with a banner naming the peer and the tick rather
than letting two games quietly become different. `tests/boundary.rs` reads
`sim`'s own `Cargo.toml` to enforce the dependency rule, and
`tests/determinism.rs` runs two worlds for 10 000 ticks and compares them.

## Layout

    crates/sim/          the game. serde + postcard, nothing else
    crates/net/          Peer trait, lockstep, wire format
    crates/net-web/      Peer over web/quad_rtc.js       (wasm32)
    crates/net-native/   Peer over matchbox_socket       (native)
    crates/gui/          macroquad client
    crates/bot/          headless peer, for testing lockstep without six tabs
    web/                 the page, the config, the WebRTC plugin
    packaging/           the web build

`floodline-design.md` is what is being built, `floodline-mvp-plan.md` is the
order and the definition of done, `DECISIONS.md` is why things are the way they
are, and `CLAUDE.md` is the rules for working in here.
