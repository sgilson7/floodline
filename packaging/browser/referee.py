"""Watches both games and writes down whether they stayed together.

    referee.py <seconds> [outdir]

Not a player. It attaches to both browsers `table.py` left standing, issues no
input, touches no map, and reads only what is drawn — the same panel a player
reads. It is the only thing in M10 allowed to see both screens, which is why
the comparison belongs to it and not to either agent.

What it can answer without reading a single digit:

* **Has the status row gone red?** `Status::Desync` is drawn in `palette::ALARM`.
  `WaitingOn` is drawn in `palette::WARNING` in the same row and happens
  constantly, so this cannot be `assign.py::alarm_band`'s "is it reddish" — see
  `alarm_pixels`, which separates the two on green. Only the host can ever
  raise a desync — see `DECISIONS.md`; a joiner is sent nobody's checksum — so
  the host's row is the one that matters and the joiner's is a courtesy.
* **Has a page stopped?** The tick row changes every tick, so a sample that
  matches the one before it means that page drew no tick in the gap between
  them. Thirty seconds of that and the other peer gives up on it
  (`DROP_AFTER_TICKS`), which is the failure that ends a run without warning.
* **Is the clock right?** `day N of 6` turns over every `TICKS_PER_DAY` ticks,
  so its wall-clock length is `TICKS_PER_DAY / TICKS_PER_SECOND` — see
  `DAY_SECONDS`. Counting day changes over a soak measures the rate to within a
  few percent and needs no arithmetic on a screenshot.
* **Or has the run simply ended?** An ended game and a stopped page look
  identical at the tick row, and the first soak reported twenty-six failures
  for a game that had finished normally. `still_playing` tells them apart at
  the tab row, which `main.rs` draws only while there is something left to
  command.

The numbers themselves are left to be read off the filmstrip by whoever wants
them. Reading digits out of a screenshot is a way to be confidently wrong, and
none of the four questions above needs it.
"""
import io
import os
import sys
import time
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View
import panel as P
import table

SECONDS = float(sys.argv[1]) if len(sys.argv) > 1 else 600.0
OUT = sys.argv[2] if len(sys.argv) > 2 else "/tmp/floodline-referee"

SAMPLE = 5.0        # how often to look: well inside the thirty-second window
FILMSTRIP = 15.0    # how often to keep the picture

# How long an in-game day lasts, in seconds of wall clock.
#
# `balance::TICKS_PER_DAY / balance::TICKS_PER_SECOND`, and the one number here
# that has to move when the clock does. It was two minutes until M11.1 doubled
# the rate; a soak that still expected two would call every healthy run late.
DAY_SECONDS = 60.0


def rows_box(which):
    """The crop each question is asked of, in logical canvas coordinates."""
    return {
        "status": (P.LEFT - 6.0, P.status() - 18.0, P.RIGHT, P.status() + 6.0),
        "tick": (P.LEFT - 6.0, P.TICK - 16.0, P.LEFT + 180.0, P.TICK + 6.0),
        "peers": (P.LEFT - 6.0, P.PEERS - 16.0, P.RIGHT, P.PEERS + 6.0),
        "day": (P.LEFT - 6.0, P.AGE_DAY - 18.0, P.RIGHT, P.AGE_DAY + 6.0),
        # The tab row, which is how a game that has *ended* is told apart from
        # a page that has stopped drawing. They look identical at the tick row,
        # which is the only place the difference matters and the only place it
        # cannot be seen.
        #
        # The tab row and not the build menu: `main.rs` draws the score screen
        # instead of `panel_layer` when the run is over, so both tabs go with
        # it — but a player who is merely *looking at the households tab* has
        # no build menu either, and calling that "the game has ended" would be
        # worse than the fault it is meant to catch.
        "tabs": (P.LEFT - 6.0, P.fixed_ends() + 6.0, P.RIGHT, P.fixed_ends() + 42.0),
    }[which]


def crop(img, V, box):
    x0, y0 = V.css(box[0], box[1])
    x1, y1 = V.css(box[2], box[3])
    d = V.dpr
    return img.crop((int(x0 * d), int(y0 * d), int(x1 * d), int(y1 * d)))


def still_playing(im):
    """Is the tab row drawn? If not, this game is over.

    Measured rather than guessed: a live panel has 847 lit pixels in this row
    on either tab, and an ended one has none.
    """
    return sum(1 for p in im.convert("RGB").getdata() if sum(p) > 200) > 100


def alarm_pixels(im):
    """Pixels that are `palette::ALARM` — and not `palette::WARNING`.

    The two are (230, 84, 71) and (240, 184, 71), and telling them apart is the
    whole job: `WaitingOn` is drawn in WARNING and happens constantly in normal
    lockstep, so the obvious "is it reddish" test — `assign.py::alarm_band`'s,
    which is looking at a row where only a refusal is ever coloured — reports a
    desync every few seconds here. Green is what separates them.
    """
    n = 0
    for r, g, b in im.convert("RGB").getdata():
        if r > 150 and g < 130 and b < 130 and r > g + 90:
            n += 1
    return n


def main():
    os.makedirs(OUT, exist_ok=True)
    log = open(os.path.join(OUT, "referee.log"), "a")

    def say(line):
        stamp = time.strftime("%H:%M:%S")
        print(f"{stamp}  {line}", flush=True)
        log.write(f"{stamp}  {line}\n")
        log.flush()

    with sync_playwright() as p:
        pages, views = [], []
        for port in table.PORTS:
            b = p.chromium.connect_over_cdp(f"http://localhost:{port}")
            page = b.contexts[0].pages[0]
            w, h = page.evaluate("[window.innerWidth, window.innerHeight]")
            pages.append(page)
            views.append(View(w, h, page.evaluate("window.devicePixelRatio")))

        who = ("city 0", "city 1")
        last = [None, None]
        last_day = [None, None]
        days = [0, 0]
        stalls = [0, 0]
        alarms = [0, 0]
        say(f"watching {len(pages)} peers for {SECONDS:.0f}s, "
            f"a look every {SAMPLE:.0f}s")

        started = time.time()
        next_film = started
        over = [False, False]
        while time.time() - started < SECONDS:
            note = []
            for i, (page, V) in enumerate(zip(pages, views)):
                img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
                tick = crop(img, V, rows_box("tick")).tobytes()
                day = crop(img, V, rows_box("day")).tobytes()
                red = alarm_pixels(crop(img, V, rows_box("status")))

                # An ended game and a stopped page both stop the tick row, and
                # calling the first one a stall is how a soak reports twenty-six
                # failures for a run that finished normally.
                if not still_playing(crop(img, V, rows_box("tabs"))):
                    if not over[i]:
                        note.append(f"{who[i]} IS OVER - the score screen is up")
                        over[i] = True
                elif last[i] is not None and tick == last[i]:
                    stalls[i] += 1
                    note.append(f"{who[i]} DREW NO TICK")
                if last_day[i] is not None and day != last_day[i]:
                    days[i] += 1
                    note.append(f"{who[i]} turned a day (now {days[i]})")
                if red > 12:
                    alarms[i] += 1
                    note.append(f"{who[i]} STATUS ROW IS RED - {red} pixels")
                last[i], last_day[i] = tick, day

                if time.time() >= next_film:
                    # The whole panel, not just the foot. This is the record of
                    # the run and the account is written from it, so it wants
                    # the age, the day, the omen and the treasury as much as it
                    # wants the tick counts.
                    stamp = time.strftime("%H%M%S")
                    crop(img, V, (P.LEFT - 18.0, 20.0, P.RIGHT + 8.0,
                                  P.BUILD_SEED + 8.0)) \
                        .save(os.path.join(OUT, f"{stamp}-{i}.png"))

            if time.time() >= next_film:
                next_film += FILMSTRIP
            say("; ".join(note) if note else "both ticking, nothing red")
            if all(over):
                say("both games have ended; there is nothing left to watch")
                break
            time.sleep(SAMPLE)

        mins = (time.time() - started) / 60.0
        say("---")
        for i in range(len(pages)):
            say(f"{who[i]}: {days[i]} days in {mins:.1f} min, "
                f"{stalls[i]} samples with no tick, {alarms[i]} red"
                + (", ended" if over[i] else ""))
        # Measured against the time actually spent playing: a run that ended
        # halfway through cannot have turned days after it.
        want = mins * 60.0 / DAY_SECONDS
        near = all(abs(d - want) <= max(1.0, want * 0.3) for d in days)
        ok = not any(stalls) and not any(alarms) and (near or any(over))
        say(f"expected about {want:.1f} days each over {mins:.1f} min")
        if any(over) and not near:
            say("fewer days than that because the run ended before the watch did")
        say("CLEAN" if ok else "NOT CLEAN - read the lines above")
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
