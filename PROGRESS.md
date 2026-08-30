# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 1 — 2026-08-29/30

**Phase 0 done. Phase 1 done.** 164 tests, green, about 8 seconds.

Live at **https://sgilson7.github.io/floodline/** — a black canvas, the word
FLOODLINE and the build hash. Every push to `main` has deployed green.

### Phase 0 checklist — all ticked

Workspace, Makefile, `packaging/package-web.sh`, `.github/workflows/pages.yml`,
`CLAUDE.md`, `DECISIONS.md`, `README.md`, `.gitignore`, reference repos read.
**Not verified: the canvas rendering in a browser** — see Blocked.

### Phase 1 checklist — all ticked

- [x] 1. `Fx` fixed point, `Rng`, `checksum()`
- [x] 2. Map generation from a seed; Hearth sites, 2–6 players, ≥40 apart
- [x] 3. `Citizen`, needs, death by starvation
- [x] 4. Buildings: Hearth, Cottage, Farm, Granary, Stockpile, Dike, Road,
         Bridge. Placement rules, materials hauled, builder-ticks applied.
- [x] 5. Flow fields per destination; walking; roads double speed
- [x] 6. Jobs: Hauler, Farmer, Builder. Farms → granary via haulers; eating at
         a granary, sleeping in a cottage
- [x] 7. `Command` and `World::apply(player, cmd)` with ownership checks;
         `World::tick(nav, &[(PlayerId, Command)])`
- [x] 8. Roads between cities, `AcceptRoad`, joined roads; `Trade` /
         `AcceptTrade` with haulers walking the goods
- [x] 9. Ages, day counter, age timer, the disaster draw (Flood only), the
         day's warning, the score
- [x] 10. `tests/determinism.rs` — 10 000 ticks; a scripted stream covering all
         fourteen `Command` variants; a cold nav cache matching a warm one
- [x] 11. `tests/boundary.rs` — `sim` depends on exactly serde and postcard

**Done when:** `cargo test -p sim --test scenario` founds two cities, builds a
road and trades food for wood for three days ✓, and the determinism test
passes ✓.

### Decided this session

Sixteen entries in `DECISIONS.md`. The ones that change what somebody else
would have written:

* **matchbox decides who offers by arrival order, not by peer id.** Design
  §9.2 says lower-id-offers. `matchbox_socket` offers on `NewPeer` and accepts
  on an unsolicited `Signal`. The rules disagree half the time and `net-web`
  has to talk to `net-native`. **A correction to the design document.**
* **A day is 1200 ticks.** §4's three numbers contradict each other and §5's
  thirty-second surge would not have fitted inside a two-hundred-tick day.
  Asked, per CLAUDE.md; the prose won. Every number keyed to a day moved with
  it — see the entry, they are all in `balance.rs`.
* **A city starts with stone as well as wood.** The plan's building list has no
  Quarry and its job list no Quarrier, so nothing produces stone, and a Dike
  costs stone, and surviving behind a Dike is the whole vertical slice.
* **Flow fields live outside `World`.** Thirty-two thousand cells each; in
  `World` they would be in every snapshot and every checksum. A cold cache is
  tested to navigate identically to a warm one, which is a late joiner's exact
  situation.
* **A Dike is walkable.** Blocking would let a player wall their own citizens
  in with the one structure design §5 exists to teach.

Four bugs the tests found rather than a player: `State::Starving` stopped a
citizen walking to the granary that would have saved it; hunger vetoed sleep,
so a city that ran out of food never slept again; the Hearth had a food
capacity I invented, so everybody ate at the fire and the granary stayed empty;
a Farm counted as a store, so haulers put the harvest back where they found it.

### Blocked

Nothing blocking. One thing unverified: **the canvas has not been seen
rendering.** Page, wasm and all four JS files serve from Pages; the wasm's
imports are exactly what `quad_rtc.js` supplies; `quad_rtc_crate_version` is
exported so miniquad would complain if plugin and Rust drifted. But this
machine has no headless browser, `screencapture` has no permission and Safari
refuses scripted queries without "Allow JavaScript from Apple Events".
Somebody should open the URL and confirm.

Phase 3 needs a browser it can drive anyway — two tabs exchanging bytes is its
definition of done — so installing Playwright is on the path there.

### Next action

Phase 2 item 1: the shallow-water automaton in `crates/sim/src/water.rs` —
`depth`, `flow`, the transfer rule with a per-tick cap, draining off the map
edges. Its tests are named in the plan: volume conserved except at the edges, a
puddle on flat ground spreading symmetrically, and water behind a level-2 dike
staying there for a height-12 surge.
