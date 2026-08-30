# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 2 — 2026-08-30

**Phases 4, 5 and 6 done. The MVP is playable end to end.** 212 tests and
seven browser checks, no warnings, `make test` in about twelve seconds.

Live at **https://sgilson7.github.io/floodline/**. Two people on two machines
open the link, one hosts and shares a room code or a pasted invitation, the
other joins, and both play the same world — with nothing running anywhere
except GitHub Pages.

### Checklist

- [x] **Phase 0** — workspace, Makefile, `package-web.sh`, Pages workflow
- [x] **Phase 1** — `sim`: land, citizens, buildings, roads, trade, ages, score
- [x] **Phase 2** — water, the surge, bodies in the flood, building damage
- [x] **Phase 3** — `net::Peer`, `Loopback`, the wire format, the star lockstep
- [x] **Phase 4** — `quad_rtc.js`, vendored Trystero, the pasted-code path,
      `net-web`, `echo.html`. Both paths verified in a real browser, on the
      deployed build.
- [x] **Phase 5** — the lobby, selection, the build menu, the road tool, the
      trade dialog, the score screen, and design step 7's playtest.
- [x] **Phase 6** — failure messages that say what to do, the relay fallback,
      the build-hash guard end to end, the README.

### What was decided

Ten entries added to `DECISIONS.md`; the ones that would change what somebody
else wrote are indexed under "Things that will bite you" in `HANDOFF.md`. The
four that matter most:

* **The handshake was written down before the plugin** (as the plan asks) and
  three of its guesses were wrong in ways only a browser could say. The one it
  got right in advance — replacing design §9.2's "a joiner accepts the first
  peer it meets" with a role byte — would otherwise have wired two joiners
  together and only with three players and only sometimes.
* **Playing a full run found four bugs no test had.** Nothing in a city ever
  built anything unless a player knew to assign builders, so an unattended city
  starved on day four with the materials on the floor. "Get uphill" — design
  §3.2's one order that matters in a flood — was undone a tick after they
  arrived. Every citizen starts inside its own Hearth and could not be ordered
  out of it.
* **The map decided the game.** Hearth sites on a ring around the map centre
  sat anywhere from 65 to 148 cells from the water, and an age-one flood stops
  at about 115. They now sit on a line at a fixed distance from the corner the
  water comes out of. The spacing guarantee fell from 40 cells to 17 and
  five- and six-player maps are cramped; that cost is written down.
* **A dike cost twenty times what a city could pay.** A wall that changes the
  outcome is about thirty-four cells; at forty stone a level that is 2 720
  against a purse of 120. Ten a level and 720 to start buys one good wall in a
  run, and choosing where to put it is the decision.

### Blocked

Nothing.

### Not answered, and needing a person

Design step 7 says "playtest the flood until it is fun", and nobody has played
FLOODLINE with their hands. What was measured is that the decisions now have
different outcomes — idling starves, growing survives two ages bloodied, a dike
keeps everybody through ages one and two — which is the part a test can settle.
Three specific questions are left, all in `DECISIONS.md` under "Design step 7":

1. **Age three kills everybody on two of three seeds** whatever is done.
2. **Nothing produces stone**, so a player gets exactly one wall in a run. That
   is a clean decision or a straitjacket, and only playing will say which.
3. **A run is thirty-six minutes.** Design §11 already suspects that is long.

### Next action

Play it, with a person, three times, and answer those three questions. Nothing
else in the plan is unfinished.
