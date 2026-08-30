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
- The riskiest work goes first, but it is now phase 4, not phase 3: lockstep
  is built and tested on `net::Loopback` with no browser at all, and only then
  does `web/quad_rtc.js` have to work. A networking regression and a lockstep
  regression can then never be confused for one another.
- Every commit passes `make test`. Commit messages: imperative subject, body
  says why. No "WIP" commits on `main`.
- Reference repos are cloned into `reference/` (gitignored). Borrow
  conventions and small pieces (`Rng`, letterbox math, Makefile, packaging
  script, panic hook, cache stamp). Do not import their code wholesale.
- Toolchain: stable Rust ≥ 1.88, `wasm32-unknown-unknown` target, macroquad
  0.4, and a vendored `trystero` browser bundle for signalling. **Nothing of
  ours runs anywhere but GitHub Pages.** No server, no crate that implies one.
  No other framework without a written decision.
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
- `make` — run the native GUI, which plays against `net::Loopback`.
- `make web` / `make serve` — browser build and local server.
- Push to `main` deploys to GitHub Pages via `.github/workflows/pages.yml`.
  There is nothing else to deploy: if Pages is up, the game is up.
