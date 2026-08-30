# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 1 — 2026-08-29/30

**Phase 0 done. Phase 1 items 1–3 done, plus items 10 and 11 pulled forward.**

Live at **https://sgilson7.github.io/floodline/** — a black canvas, the word
FLOODLINE, and the build hash. `make test` is 58 tests in about 6 seconds.

### Phase 0 checklist — all ticked

- [x] Workspace, design §7's six crates, `rust-version = "1.88"`, resolver 2,
      the workbench's `[profile.release]` and `[profile.test]`.
- [x] `Makefile`: `make`, `test`, `check`, `build`, `web`, `serve`, `publish`,
      `signal`, `bot ROOM=x`, `clean`, `help`.
- [x] `packaging/package-web.sh`: builds `gui` for wasm32, copies
      `mq_js_bundle.js` and `sapp_jsutils.js` from the Cargo.lock-pinned
      registry sources, copies `web/quad_rtc.js` and `web/config.js`, stamps
      the wasm sha256 into `index.html` as `window.FLOODLINE_BUILD`, and fails
      the build if the stamp did not land.
- [x] `.github/workflows/pages.yml`, deploying `dist/web` after `make test`.
- [x] `CLAUDE.md`, `DECISIONS.md`, `README.md`, `.gitignore`.
- [x] Reference repos read, DECISIONS.md entries written.
- [x] Pages deployment succeeds. **Not verified: the canvas rendering in a
      browser** — see Blocked.

### Phase 1 checklist

- [x] 1. `Fx` fixed point, `Rng`, `checksum()`
- [x] 2. Map generation from a seed, one Hearth site per player, 2–6 players,
         ≥40 cells apart
- [x] 3. `Citizen` with needs, decay, death by starvation
- [ ] 4. Buildings and construction
- [ ] 5. Flow fields and walking
- [ ] 6. Jobs
- [ ] 7. `Command` and `World::apply`
- [ ] 8. Roads and trade
- [ ] 9. Ages and the disaster draw
- [x] 10. `tests/determinism.rs` — 10 000 ticks, four seeds, two player counts.
         Grows with `Command` at item 7.
- [x] 11. `tests/boundary.rs` — `sim`'s dependencies are exactly serde and
         postcard, asserted by parsing `Cargo.toml`.

10 and 11 were pulled forward from the end of the phase: a rule enforced from
the first commit costs nothing, and a rule enforced after eight more items is
one that has already been broken once.

### Decided this session

Nine entries in `DECISIONS.md`. The four worth knowing without reading it:

* **matchbox decides who offers by arrival order, not by peer id.** Design
  §9.2 says lower-id-offers; `matchbox_socket` offers on `NewPeer` and accepts
  on an unsolicited `Signal`. The rules disagree half the time and `net-web`
  has to talk to `net-native`, so `quad_rtc.js` will do arrival order.
  **This is a correction to the design document.**
* **Ground bands are cut by quantile, not by fixed height.** Fixed thresholds
  made map quality a lottery — seed 0 had 30 shallow cells, seed 7 had no rock
  at all. Every seed is now a playable map.
* **The noise amplitude was chosen by measurement.** `map::probe::
  sweep_noise_amplitude` counts, over 300 seeds, how often the high corner
  comes out wetter than the low one. 110 is the largest amplitude that never
  does; 170 got it wrong 5 times in 300. That failure would put the flood's
  source at the wrong end of the map.
* **`overflow-checks = true` in release.** A debug native bot and a release
  wasm browser must not disagree about arithmetic.

Also: `sim` is serde + postcard (the plan contradicts itself; `checksum()`
settles it), citizens carry a `name: u16` into a 256-entry table, package names
are unprefixed so `cargo test -p sim` works as the plan writes it, and Pages
deploys `dist/web` while `make publish` survives for `docs/`.

### Blocked

Nothing blocking. One thing unverified: **the canvas has not been seen
rendering.** The page, wasm and all four JS files serve from Pages; the wasm's
imports are exactly what `quad_rtc.js` supplies; `quad_rtc_crate_version` is
exported so miniquad would complain if the plugin and the Rust drifted. But
this machine has no headless browser, `screencapture` has no permission and
Safari refuses scripted queries without "Allow JavaScript from Apple Events".
Somebody should open the URL and confirm they see FLOODLINE and a build hash.

Phase 3 needs a browser it can drive anyway — two tabs exchanging bytes is its
definition of done — so installing Playwright is on the path there, not a
detour.

### Next action

Phase 1 item 4: buildings — Hearth, Cottage, Farm, Granary, Stockpile, Dike,
Road, Bridge — with placement rules and construction from hauled materials, in
`crates/sim/src/building.rs`, with its tests.
