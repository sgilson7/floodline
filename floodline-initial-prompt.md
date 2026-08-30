# Starting Claude Code on FLOODLINE (v2-noserver)

Two files here. The first is `CLAUDE.md` — put it at the root of the new repo
before the first session so it is loaded every time. The second is the message
to paste as the first prompt.

Before starting: create the empty GitHub repo (`floodline`) and enable Pages
with source "GitHub Actions". That is the entire infrastructure. Put
`floodline-design-v2-noserver.md` and `floodline-mvp-plan-v2-noserver.md` in
the repo root.

---

## `CLAUDE.md`

```markdown
# FLOODLINE — rules for working in this repo

Read `floodline-design-v2-noserver.md` (what we are building) and
`floodline-mvp-plan-v2-noserver.md` (the order and the definition of done)
before doing anything. This project runs no servers: the host's browser is
the hub, peers reach it through Trystero's public signalling or a pasted
code, and the only deployment is static files on GitHub Pages. `DECISIONS.md`
records choices already made; `PROGRESS.md` says where the last session ended.

## Non-negotiable
- `crates/sim` depends on `serde` and `postcard` only. No `f32`/`f64`, no
  `HashMap`/`HashSet`, no `std::time`, no `rand`, no `Instant`. One `Rng`
  lives in `World`. `tests/boundary.rs` and `tests/determinism.rs` enforce
  this; if either fails, fix it before anything else.
- The world changes only through `World::apply(player, Command)`. `gui` and
  `net` never mutate `World` directly.
- Lockstep is proven on `net::Loopback` (phase 3) before the browser plugin
  exists; the plugin `web/quad_rtc.js` (phase 4) is proven on two real
  networks, in both trystero and pasted-code modes, before `gui` (phase 5)
  begins.
- Never add a server, a signalling service of ours, a database or an npm
  build step. If something seems to need one, stop and say so.
- Every commit passes `make test`. Commit messages: imperative subject, body
  says why. No "WIP" commits on `main`.
- Reference repos are cloned into `reference/` (gitignored). Borrow
  conventions and small pieces (`Rng`, letterbox math, Makefile, packaging
  script, panic hook, cache stamp). Do not import their code wholesale.
- Toolchain: stable Rust ≥ 1.88, `wasm32-unknown-unknown` target, macroquad
  0.4, and a pinned vendored copy of trystero's browser bundle in
  `web/vendor/`. No other framework without a written decision.
- Anything design §11 leaves open: ask. Anything else: decide, write one
  paragraph in `DECISIONS.md`, continue.

## Working style
- One phase at a time, in plan order. Finish the phase's checklist before
  starting the next; tick items off in `PROGRESS.md`.
- Prefer a test over a demo, a demo over a description. Every `sim` feature
  ships with its test in the same commit.
- Keep files small. `gui` is several modules from day one, not one file.
- Comments explain why, not what. Match the register of the reference repos:
  plain, specific, no hype.
- At session end, update `PROGRESS.md` with: phase, done/not done against the
  checklist, decisions, blockers, the single next action.

## Commands
- `make test` — whole suite, no window, no network (lockstep runs on
  `net::Loopback`).
- `make` — run the native GUI against loopback peers.
- `make web` / `make serve` — browser build and a local static file server.
- Push to `main` deploys to GitHub Pages via `.github/workflows/pages.yml`.
```

---

## First prompt

```
You are building FLOODLINE, a browser-based multiplayer medieval city builder
in Rust + WASM with no server of any kind. Read CLAUDE.md,
floodline-design-v2-noserver.md and floodline-mvp-plan-v2-noserver.md
completely before writing any code. They are the spec; when they disagree,
the plan wins on order and the design wins on content, and you tell me.

Your task for this session is Phase 0 of the plan, and then start Phase 1.

Phase 0, concretely:
1. Clone sgilson7/gear-master, sgilson7/perturbation-workbench,
   sgilson7/pdf-redactor and dmotz/trystero into reference/ (add it to
   .gitignore). Read gear-master's Cargo.toml, Makefile,
   packaging/package-web.sh, crates/engine/src/rng.rs and crates/console/src/verb.rs;
   the workbench's Cargo.toml, packaging/package-web.sh and crates/wasm/src/lib.rs;
   and trystero's README plus src/index.js and the nostr strategy file, noting
   how joinRoom exposes the underlying RTCPeerConnection.
   Write two paragraphs in DECISIONS.md on what you are taking from each and
   what you are deliberately not.
2. Create the workspace from design §7 with all crates compiling empty:
   sim, net, net-web, gui. Same [profile.release] and
   [profile.test] as the workbench, rust-version 1.88, resolver 2.
3. Makefile with the targets listed in CLAUDE.md and a `make help`.
4. packaging/package-web.sh adapted from gear-master: build gui for
   wasm32-unknown-unknown, copy mq_js_bundle.js and sapp_jsutils.js from the
   pinned crate versions in Cargo.lock, copy web/quad_rtc.js (a stub for now),
   web/config.js and web/vendor/trystero-<version>.js (download once, pin the
   filename and sha256 in the script), stamp the wasm sha256 into index.html and expose it as
   window.FLOODLINE_BUILD. The gui, for now, draws a black canvas with the
   words "FLOODLINE" and the build hash.
5. .github/workflows/pages.yml: on push to main, `make test` then `make web`
   then deploy dist/web with actions/deploy-pages.
6. PROGRESS.md, DECISIONS.md, README.md stub.

Verify locally: `make test` green, `make web` produces dist/web, and
`make serve` shows the canvas. Commit in small steps. Then push to main and
confirm the Pages deployment succeeds; if it fails, fix the workflow rather
than the code.

Then begin Phase 1 with items 1–3 of its list: Fx fixed point, Rng,
checksum(), map generation from a seed with per-player Hearth sites, and the
Citizen struct with needs. Each with tests. Stop when those pass or when you
hit something the design leaves open, and write PROGRESS.md.

Constraints to keep in mind the whole time: sim is serde+postcard only, no
floats, no HashMap, one Rng in World, commands are the only door, and there
is no server. If you catch yourself reaching for f32, a physics crate, a
signalling server or npm, stop and re-read design §3.4 and §9.

Report back with: what was committed, the Pages URL, the DECISIONS.md
entries, and the single next action.
```

---

## Prompts for later sessions

Keep them short; the plan carries the detail. Pattern:

```
Continue FLOODLINE. Read PROGRESS.md first. This session: finish Phase N
items X–Y per floodline-mvp-plan.md. Same rules as CLAUDE.md. Report as before.
```

For phase 3 add:

```
Everything in this phase runs on net::Loopback in cargo test. Do not touch
web/ or the browser. The phase is done when three loopback players finish
the scenario to age 2 with identical checksums under simulated latency, a
forced desync freezes all of them at the same tick, a silent player is
dropped deterministically, and a late joiner from a snapshot stays in sync.
```

For phase 4 add:

```
This is the riskiest phase. Before writing quad_rtc.js, write down in
DECISIONS.md the exact event sequence you expect for one host and two
joiners in trystero mode (from trystero's source) and in pasted-code mode
(from MDN's RTCPeerConnection docs, with trickle ICE disabled), then
implement to that. Test with the echo.html page on two machines on two
different home networks via the Pages build; nothing else may be running
anywhere. Do not proceed to Phase 5 until both modes connect, bytes flow on
both channels, a closed tab produces Left, and a mismatched build hash is
refused. Record measured join times per strategy in DECISIONS.md.
```
