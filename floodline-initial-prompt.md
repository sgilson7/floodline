# Starting Claude Code on FLOODLINE

Two files here. The first is `CLAUDE.md` — put it at the root of the new repo
before the first session so it is loaded every time. The second is the message
to paste as the first prompt.

Before starting: create the empty GitHub repo (`floodline`), enable Pages with
source "GitHub Actions", and have a Fly.io account ready for phase 3. Put
`floodline-design.md` and `floodline-mvp-plan.md` in the repo root.

---

## `CLAUDE.md`

```markdown
# FLOODLINE — rules for working in this repo

Read `floodline-design.md` (what we are building) and `floodline-mvp-plan.md`
(the order and the definition of done) before doing anything. `DECISIONS.md`
records choices already made; `PROGRESS.md` says where the last session ended.

## Non-negotiable
- `crates/sim` depends on `serde` and `postcard` only. No `f32`/`f64`, no
  `HashMap`/`HashSet`, no `std::time`, no `rand`, no `Instant`. One `Rng`
  lives in `World`. `tests/boundary.rs` and `tests/determinism.rs` enforce
  this; if either fails, fix it before anything else.
- The world changes only through `World::apply(player, Command)`. `gui` and
  `net` never mutate `World` directly.
- The riskiest work goes first: `web/quad_rtc.js` (phase 3) is proven before
  phase 4 or 5 begins.
- Every commit passes `make test`. Commit messages: imperative subject, body
  says why. No "WIP" commits on `main`.
- Reference repos are cloned into `reference/` (gitignored). Borrow
  conventions and small pieces (`Rng`, letterbox math, Makefile, packaging
  script, panic hook, cache stamp). Do not import their code wholesale.
- Toolchain: stable Rust ≥ 1.88, `wasm32-unknown-unknown` target, macroquad
  0.4, matchbox 0.14 (server deployed, `matchbox_socket` native-only). No
  other framework without a written decision.
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
- `make test` — whole suite, no window.
- `make` — run the native GUI. `make bot ROOM=x` — headless peer.
- `make signal` — local matchbox_server on :3536.
- `make web` / `make serve` — browser build and local server.
- Push to `main` deploys to GitHub Pages via `.github/workflows/pages.yml`.
```

---

## First prompt

```
You are building FLOODLINE, a browser-based multiplayer medieval city builder
in Rust + WASM. Read CLAUDE.md, floodline-design.md and floodline-mvp-plan.md
completely before writing any code. They are the spec; when they disagree,
the plan wins on order and the design wins on content, and you tell me.

Your task for this session is Phase 0 of the plan, and then start Phase 1.

Phase 0, concretely:
1. Clone sgilson7/gear-master, sgilson7/perturbation-workbench,
   sgilson7/pdf-redactor and johanhelsing/matchbox into reference/ (add it to
   .gitignore). Read gear-master's Cargo.toml, Makefile,
   packaging/package-web.sh, crates/engine/src/rng.rs and crates/console/src/verb.rs;
   the workbench's Cargo.toml, packaging/package-web.sh and crates/wasm/src/lib.rs;
   and matchbox_protocol/src/lib.rs plus matchbox_socket/src/webrtc_socket/wasm.rs.
   Write two paragraphs in DECISIONS.md on what you are taking from each and
   what you are deliberately not.
2. Create the workspace from design §7 with all crates compiling empty:
   sim, net, net-web, net-native, gui, bot. Same [profile.release] and
   [profile.test] as the workbench, rust-version 1.88, resolver 2.
3. Makefile with the targets listed in CLAUDE.md and a `make help`.
4. packaging/package-web.sh adapted from gear-master: build gui for
   wasm32-unknown-unknown, copy mq_js_bundle.js and sapp_jsutils.js from the
   pinned crate versions in Cargo.lock, copy web/quad_rtc.js (a stub for now)
   and web/config.js, stamp the wasm sha256 into index.html and expose it as
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
floats, no HashMap, one Rng in World, commands are the only door. If you
catch yourself reaching for f32 or a physics crate, stop and re-read design
§3.4.

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

For phase 3 specifically, add:

```
This is the riskiest phase. Before writing quad_rtc.js, write down in
DECISIONS.md the exact message sequence you expect for two peers joining
(from matchbox_protocol and the wasm.rs socket), then implement to that.
Test against a local matchbox_server first, then against Fly.io over wss.
Do not proceed to Phase 4 until two browser tabs and one bot exchange bytes
on both channels and a closed tab produces a Left event.
```
