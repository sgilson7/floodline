# FLOODLINE — M12 second-order effects

Every change made in M12, and **what else it changes**. `DECISIONS.md` says
why a choice was made; this says what that choice did to things nobody was
looking at.

It exists because the M11 run was lost to exactly this class of thing: M11.6
added deaths with causes and they were invisible, not because the feature was
wrong but because it shared a message slot nobody had thought about. The
feature was reviewed. The slot was not.

**How to read an entry.** *The change* is what was done. *Follows from* is what
it necessarily drags with it. *Watch* is what would show it going wrong, and
where. A change with no second-order effects gets an entry saying so — that is
a claim, and a claim can be wrong.

---

## M12.1 — the lobby fault, reproduced

**The change.** Three tests, no production code:
`a_host_that_finished_a_game_tells_a_new_joiner_something`,
`a_joiner_that_greeted_a_ghost_still_greets_the_host_when_it_arrives`,
`a_joiner_in_a_room_with_churn_is_still_told_about_the_silence`.

**What the reproduction found**, which is not what the handover guessed. The
handover's first suspect was a stale peer in a reused room, and it was right
about the room and wrong about the mechanism. The handshake against a finished
host is *fine* — it answers `this game is full`, which is an answer. The fault
is one line up from the handshake: **`greet` was a `bool`.** A joiner said
`Hello` to the first peer it met and to nobody else, ever. A non-host that
receives a `Hello` does nothing with it — the handler is guarded `if self.host`
and falls through to `_ => {}` — so nothing on the wire ever said *"I am not
the one you want"*.

**Follows from.**

* The build-hash suspect is ruled out and stays ruled out: the room name
  carries the hash, so two builds never meet at all.
* `rejoin.py` being lobby-only was never the gap it looked like. The gap was
  that **no test anywhere put a joiner in a room with more than one peer in
  it.** Every existing test is a star with one edge, which is the shape the bug
  hides behind. That is the gap M12.3 closes, and it is a different gap from
  the one the handover named.

**Watch.** `nearly_over` arranges *where a world starts* and nothing else — the
ending is reached through `age.rs`'s own roll-over. If a future change makes
`finished` reachable some other way, that test stops covering what it says.

---

## M12.2 — greet everybody; a clock nothing resets

**The change.** `greet: bool` → `greeted: BTreeSet<PeerId>`; `host_peer` set
only by `Welcome`; `unanswered` → `waiting_since`, counted off `greeted` and
never reset by `peer_left`; the silence message widened to cover a room with
nobody hosting in it.

**Follows from.**

1. **A joiner now sends one `Hello` per peer in the room.** In a two-person
   game that is one extra message in the worst case. In a room with stale tabs
   it is one each, once. `Hello` is small and the cost is bounded by peers, not
   by time — it is a set, so a peer already asked is never asked twice.
2. **A non-host still ignores `Hello` silently, deliberately.** Answering it
   with `Bye` would have been the honest wire — but `Bye` is what *ends* a
   joiner's run, so a bystander replying would end the game of a joiner that
   has done nothing wrong. Greeting everybody makes the silence harmless and
   `waiting_since` makes it audible. Written up in `DECISIONS.md`; a new
   message type would have changed the wire format and therefore the build
   hash, for a sentence.
3. **`peer_left` no longer ends a joiner's game unless it was welcomed *and*
   the departing peer was the one that welcomed it.** Before, `host_peer` was
   set by greeting, so the peer whose departure ended your game was whoever you
   happened to meet first. This is strictly narrower and strictly more correct.
4. **A peer that drops and reconnects is greeted again** — `peer_left` removes
   it from `greeted`. That is deliberate: from inside a room, a host on a fresh
   connection and a stranger are the same event.
5. **Two existing tests were encoding the old world** and were changed, not the
   code: "Hello is said once, however many peers turn up" *is* the fault. Said
   in the commit message so nobody re-derives it.

**Watch.** `SILENCE_FRAMES` is 500 frames, about eight seconds. It is now a
floor on how long a joiner can be silent, where before it was unreachable in a
churning room — so a *slow* `Welcome` on a bad connection could now produce a
warning that is wrong. The `Welcome` snapshot is 102 KB; if it grows toward the
150 KB budget, check that it still arrives inside eight seconds on a real link
before trusting the message. This is a real regression risk and it is the price
of the message existing at all.

---

## M12.3 — the guard, in a browser

**The change.** `rejoin.py` gains a room where two joiners meet each other
before the host arrives.

**Follows from.** The plan asked for "a run played to its end, then two peers
into a lobby again". That is **not** the check that was needed, and building it
would have cost eighteen minutes a run for a case `cargo` settles in under a
second. The check that was needed is a room with more than one peer in it, and
it takes twenty seconds. Recorded here because the handover named the wrong
gap in good faith and somebody will otherwise go and build the expensive one.

**Watch.** Two seats, so exactly one of the two joiners gets a city and the
other is told the game is full. The assertion is `welcomed(x) or welcomed(y)`
on purpose. If the seat count in the lobby ever defaults to three, this check
gets weaker without failing — it would pass with both welcomed and would no
longer be testing the ordering. Pin it if that default moves.

---

## M12.A — a farm feeds a city rather than a household

**The change.** `FARM_TICKS_PER_UNIT` 32 → 11. A farmer makes 109 units a day
instead of 37; a three-slot farm keeps 26 people instead of nine.

**Asked for directly.** This is the first change in M12 that is not a fault
being fixed, and the distinction matters: the measurement below says what the
change *did*, it is not what chose it.

**The table, either side.** `three_full_runs_of_each_strategy`, three seeds,
five scripted strategies, survivors across all seeds:

    play    before   after
    idle       0        0
    grow       8        9
    dike       8        7
    flee       4       12
    both       0        8

**Follows from.**

1. **The ceiling on doing more than one thing is what lifted.** `dike` and
   `grow` — the two single-verb strategies — did not move: 8 → 7 and 8 → 9,
   which is noise. `both` went from **nought survivors to eight**, and on seed
   31 from two ages with everybody dead to three ages with all eight standing.
   That is the change, and it is exactly the complaint three playtests made:
   walling cost the days that feeding needed.
2. **`flee` tripled, 4 → 12.** Getting uphill means leaving your farm, and
   leaving your farm used to be fatal on its own. It is now a thing a city can
   afford to do. Note this lands on the same verb M12.9 is about — a flat map
   is why fleeing rarely helps — so **M12.9's measurement must be taken after
   this, not before.**
3. **The flood is still what kills you.** `idle` dies on every seed, and two
   of the three seeds still kill every walling strategy outright. Food stopped
   being the only clock; it did not stop being a clock.
4. **`grow` on seed 1000003 got worse**, [8, 4, 0] → [8, 1, 0], and on seed
   4043362590 [8, 8, 1] → [7, 5, 2]. The scripts are fixed, so more food means
   the same script spends its days differently. Not understood, and it is
   noted rather than explained. **M12.10 measures growth and must not read
   these two cells as a growth finding.**
5. **Nothing in the suite noticed.** All 289 tests passed unchanged across a
   3× production change, because not one asked what a farm feeds. That is a
   test-coverage fault of the same family as `how_a_city_grows`: the number was
   only ever checked by playing it. `a_farm_feeds_a_founding_party_several_times_over`
   is the guard now.
6. **`FARM_BUFFER` is the real cap until a granary is standing.** A Hearth
   holds no food, deliberately — design §3.3 gives it no larder. So with
   nowhere on the map to put a farm's output, the buffer fills at 60 and the
   farmers stop: measured without draining, **a farm makes exactly 60 units a
   day at any value of this constant.** Tripling the rate therefore does
   nothing at all for a city that has not built a granary, and a great deal for
   one that has. That is a sharp new edge on an existing decision and no
   readout anywhere says a farm has stopped because its buffer is full.
   **Candidate work, and it is not currently in the plan.**

**Watch.** `three_full_runs_of_each_strategy` is now the *new* baseline. Any
measurement in M12.8, M12.9 or M12.10 taken against the pre-M12.A table is
reading a game that no longer exists.

---
