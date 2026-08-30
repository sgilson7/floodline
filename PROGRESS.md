# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 1 — 2026-08-29

**Phase 0 — repository and pipeline: done.** Phase 1 started.

### Phase 0 checklist

- [x] Workspace with design §7's crate layout, all six crates compiling empty.
      `rust-version = "1.88"`, resolver 2, the workbench's `[profile.release]`
      and `[profile.test]`.
- [x] `Makefile`: `make`, `test`, `check`, `web`, `serve`, `publish`, `signal`,
      `bot ROOM=x`, `help`.
- [x] `packaging/package-web.sh`: builds `gui` for wasm32, copies
      `mq_js_bundle.js` and `sapp_jsutils.js` from the Cargo.lock-pinned
      registry sources, copies `web/quad_rtc.js` and `web/config.js`, stamps
      the wasm sha256 into `index.html` and exposes it as
      `window.FLOODLINE_BUILD`.
- [x] `.github/workflows/pages.yml`: push to `main` → `make test` → `make web`
      → `actions/deploy-pages` on `dist/web`.
- [x] `CLAUDE.md`, `DECISIONS.md`, `README.md`, `.gitignore`.
- [x] Reference repos cloned into `reference/`, the named files read, the
      DECISIONS.md entries written.

**Done when:** `make test` green with zero tests ✓, `make web` produces a page
showing a macroquad canvas with the build hash in the corner ✓, the Pages
workflow deploys it — see below.

### Phase 1 checklist

- [ ] 1. `Fx`, `Rng`, `checksum()`
- [ ] 2. Map generation from a seed, Hearth sites
- [ ] 3. `Citizen` and needs
- [ ] 4–11. not started

### Decided this session

Six entries in `DECISIONS.md`. The two that change what someone else would
have written:

* **matchbox decides who offers by arrival order, not by peer id.** Design
  §9.2 says lower-id-offers; `matchbox_socket` actually offers on `NewPeer`
  and accepts on an unsolicited `Signal`. The two rules disagree half the
  time, and `net-web` has to connect to `net-native`, so `quad_rtc.js` will
  implement arrival order. **This is a correction to the design document.**
* **The build hash crosses the JS boundary in phase 0**, through
  `quad_rtc.js` and `sapp-jsutils`, rather than being drawn by JS. §8 needs
  it inside `Hello` anyway, and doing it now means the bridge phase 3 rests
  on is proven by the first deployment.

Also: `sim` depends on serde *and* postcard (the plan contradicts itself;
`checksum()` settles it), package names are unprefixed so the plan's literal
`cargo test -p sim` works, Pages deploys `dist/web` while `make publish`
survives for `docs/`, and citizens carry a `name: u16` index into a static
table — the answer to the design §11 question that was asked.

### Blocked

Nothing.

### Next action

Phase 1 item 1: `Fx` fixed point in `crates/sim/src/fx.rs`, with its tests.
