# Progress

Where the last session ended. Short enough to read in a minute.

---

## Session 1 — 2026-08-29/30

**Phases 0, 1, 2 and 3 done. Phase 5's renderer done early.** 204 tests, no
warnings, `make test` in about twelve seconds. Every push to `main` has
deployed green.

Live and playable at **https://sgilson7.github.io/floodline/** — a generated
map, two cities, a running lockstep game. Press space to start it.

**`HANDOFF.md` is the document to read next**, and `NEXT-SESSION-PROMPT.md` is
the prompt to start the next session with. This file is only the summary.

### Checklist

- [x] **Phase 0** — workspace, Makefile, `package-web.sh`, Pages workflow
- [x] **Phase 1** — `Fx`, `Rng`, `checksum`, map, citizens, buildings, flow
      fields, jobs, `Command`, roads and trade, ages and score, determinism and
      boundary tests
- [x] **Phase 2** — the shallow-water automaton, the surge, bodies in the
      flood, building damage, profiling
- [x] **Phase 3** — `net::Peer`, `net::Loopback`, the wire format, the star
      lockstep, and all of phase 3's checklist in `cargo test`
- [ ] **Phase 4** — `quad_rtc.js`, trystero, the pasted-code path, `net-web`.
      Not started. `net-web` is an empty crate.
- [~] **Phase 5** — map, panel and score screen done. **No input at all**:
      no selection, no build menu, no road tool, no trade dialog, no lobby.
- [ ] **Phase 6** — hardening. Not started.

### The v2 pivot

Mid-session the design and plan were replaced with their `v2-noserver` drafts.
Sections 1–6 — the whole simulation — are unchanged, so phases 0 to 2 stood.
What changed: no server anywhere (trystero over public relays, plus a pasted
code, instead of matchbox on Fly.io), a star instead of a full mesh, and
phases 3 and 4 swapped so the lockstep is proven on an in-process loopback
before any browser is involved. `net-native` and `bot` were deleted.

That swap earned its keep immediately: four corrections to design §8 and one
plain single-player bug in `sim` came out of phase 3, and every one of them
would otherwise have been debugged through a WebRTC connection.

### Decided this session

Thirty-one entries in `DECISIONS.md`. The ones that change what somebody else
would have written are indexed under "Things that will bite you" in
`HANDOFF.md`.

### Blocked

Nothing. The one thing that needed an account the author has and the agent does
not — a Fly.io signalling server — stopped existing when the project moved to
v2.

### Next action

Phase 4, first item: write the message sequence for a host and one joiner into
`DECISIONS.md`, for both the trystero path and the pasted-code path, before
writing any of `web/quad_rtc.js`.
