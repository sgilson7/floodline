# Starting the next Claude Code session on FLOODLINE

`CLAUDE.md` is already in the repo root and is loaded every session. The
message below is what to paste as the first prompt.

---

## The prompt

```
You are continuing FLOODLINE, a browser multiplayer medieval city builder in
Rust and WASM. It is deployed and playable at
https://sgilson7.github.io/floodline/ and phases 0 to 3 of the plan are done.

Read HANDOFF.md first — it is the shortcut through everything else and it says
what will bite you. Then CLAUDE.md, floodline-design.md and
floodline-mvp-plan.md. DECISIONS.md is thirty-one entries of why things are the
way they are; read the ones HANDOFF.md points you at, and the rest when you
disagree with something.

Your task is phases 4, 5 and 6, in that order, finishing the MVP. You know you
are done when two people on two machines open the Pages link, join the same
game, each build a small town, get flooded, and see a score screen — with
nothing running anywhere except GitHub Pages.

Phase 4 first, and do not start phase 5 until it is done. It is the riskiest
work in the project: quad_rtc.js, trystero, the pasted-code path, and net-web.
Before you write a line of the plugin, write down in DECISIONS.md the exact
message sequence you expect for a host and one joiner, in order, for both paths
— read design §9.1–9.2 and the trystero source you vendor, and implement to
what you wrote. Vendor trystero pinned by filename and sha256 into web/vendor/
and record both. No npm at build time.

Phase 4 is done when two browser tabs on the Pages build join the same room,
exchange bytes on both channels, and a closed tab produces Left on the other —
by trystero and by pasted code — and then when net::Loopback is swapped for
net-web under the phase-3 lockstep and two tabs play the phase-1 scenario.
You will need a headless browser to check any of that; HANDOFF.md has the
recipe the last session used. Drive it at a device pixel ratio of 2 as well as
1, and read the page's console errors — that is how the two worst bugs of the
last session were found and neither was visible any other way.

Phase 5 is the rest of gui: input, the build menu, the road tool, the trade
dialog, the lobby and the score screen wiring. The renderer, the panel and the
score screen already exist. Read the phase 5 checklist in the plan before you
touch screen.rs; the letterbox has two coordinate systems in it and the deployed
game has already been broken once by confusing them.

The rules that are not negotiable, all of them already in CLAUDE.md: sim
depends on serde and postcard and nothing else; no floats and no HashMap
anywhere in sim; commands are the only way the world changes; make test passes
at every commit; every sim feature ships with its test in the same commit. If
tests/boundary.rs or tests/determinism.rs ever fails, fix it before anything
else.

Two things about how the last session worked that are worth keeping. When a
number is a judgement — how far a flood spreads, how much damage water does,
how much terrain relief there is — measure it with a throwaway probe and write
the measurement into the comment, rather than picking a value that looks
reasonable. Three separate flood constants were wrong in ways no test caught
until they were measured. And when a test fails, find out whether the test or
the code is wrong before changing either; roughly half the failures last
session were the test being wrong about what the design actually promised, and
saying so in the comment is worth more than the fix.

Anything design §11 leaves open: ask. Anything else: decide, write a paragraph
in DECISIONS.md, and continue.

Report back with: what was committed, whether two tabs actually talked to each
other and by which path, the DECISIONS.md entries, and the single next action.
```

---

## Prompts for the sessions after that

Keep them short; the plan and HANDOFF carry the detail.

```
Continue FLOODLINE. Read PROGRESS.md and HANDOFF.md first. This session:
finish phase N items X–Y per floodline-mvp-plan.md. Same rules as CLAUDE.md.
Report as before.
```

For phase 5's last session, add:

```
Before you call phase 5 done, play it. Design step 7 is "playtest the flood
until it is fun", and it is the only step in the whole plan that cannot be
discharged by a test passing. Run at least three full games, write in
DECISIONS.md what you tuned and why, and say plainly if it is not fun yet.
```
