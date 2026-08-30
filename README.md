# FLOODLINE

A browser multiplayer medieval city builder. Two to six players share one map,
each founding a city on it, linked by roads they lay between them and by barter
that haulers actually walk. At the end of every age a wall of water comes out
of one corner and takes whatever was not behind a dike. The run ends when the
last city is gone; the score is how many ages the map stood.

Rust and WASM, drawn with rectangles and lines. Deterministic lockstep over
WebRTC — only commands go over the wire, so six browsers can share five hundred
citizens and a flood on a few kilobytes a second.

**This project runs no servers.** The whole game is static files on GitHub
Pages; the host's own browser is the hub, and two players are introduced either
by public relays nobody here operates or by a code they paste to each other.

## Play

**<https://sgilson7.github.io/floodline/>**

One of you hosts and the other joins, two ways:

* **By room code.** The host presses *Host a game*, gets something like
  `brisk-otter-42`, and sends the link beside it. The other opens the link and
  presses *Join a game*. Public relays — Nostr by default — introduce the two
  browsers to each other and are never touched again; the game itself goes
  directly between you.
* **By pasted code**, when the relays are blocked or slow. The host presses
  *Host by pasted code* and gets about three hundred characters to send over
  whatever chat you are already using. The other pastes it, gets a reply of
  about the same size, and sends that back. One round trip and you are
  connected, with no third party involved at all.

Then the host presses **Start**. Both work from one machine to another across
the internet, and both work with nothing running anywhere.

### When it will not connect

* *"no signalling relay answered"* — the relays are unreachable from your
  network. The lobby offers the pasted-code path, which needs none of them.
* *"one of you may be behind a strict NAT"* — this is the one case with no free
  answer. WebRTC needs a public path between the two of you; STUN finds one for
  most home connections, and when it cannot, only a TURN relay can carry the
  traffic. `web/config.js` has `rtcConfig.iceServers` with public STUN servers
  and a commented example of a TURN entry: adding one is a single edit and no
  rebuild. **It is the only thing in this game that can cost money**, and it is
  the uncommon case, not the common one.
* *"different builds"* — one of you has an old page cached. Reload. A room's
  name carries the build's hash precisely so that two versions of the game
  cannot half-join each other.

## Run it locally

    make            # native build: a two-player loopback game in one window
    make test       # the whole suite, no window, about twelve seconds
    make web        # browser build into dist/web/
    make serve      # ...and serve it at localhost:8080
    make browser-test   # the checks that need a real browser
    make help       # every command

Needs stable Rust >= 1.88 and the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`). `make browser-test` also builds a
virtualenv with Playwright and a copy of Chromium the first time it is run; see
`packaging/browser/README.md` for what each check answers.

Note that a locally built page and the deployed one **cannot join each other**,
and that is the guard working: the build hash is the sha256 of the wasm, CI's
compiler is not yours, and mismatched builds are refused. Test two tabs of the
same build.

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

    crates/sim/          the game. serde + postcard, nothing else, no floats
    crates/net/          Peer trait, Loopback, wire format, the star lockstep
    crates/net-web/      Peer over web/quad_rtc.js                    (wasm32)
    crates/gui/          macroquad client: map, panel, lobby, input
    web/                 the page, the config, the WebRTC plugin
    web/vendor/          Trystero, pinned by name and sha256
    packaging/           the web build and the browser checks

`floodline-design.md` is what is being built, `floodline-mvp-plan.md` is the
order and the definition of done, `DECISIONS.md` is why things are the way they
are, `PROGRESS.md` is where the last session stopped, and `CLAUDE.md` is the
rules for working in here.
