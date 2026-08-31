"""Two agents, two browsers, one room: the ground M10 is played on.

`game_two_tabs.py` reaches the same world with two pages in one context, which
is right for a transport check and wrong for a playtest. Two agents must not
share a clipboard, a `localStorage` or a permission grant — and, over
a whole run, must not share a renderer scheduler either:

    Lockstep::DROP_AFTER_TICKS is thirty seconds, counted in the waiting
    peer's own ticks, and Clock::MOST_PER_FRAME is a whole second's worth of
    catch-up. So a page has to render at least 1.25 frames a second to hold the
    tick rate — both numbers moved when the clock doubled and that floor did
    not — and a page that stops for thirty seconds is dropped by the other
    peer.

The dance itself is `table.py`, which is also what starts a real run. This is
the check that it still works.

    two_agents.py [url] [dpr]
"""
import sys, time
from PIL import Image
import io
from playwright.sync_api import sync_playwright
from view import View, LOGICAL_H
import panel as P
import table

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

V = View(table.W, table.H, DPR)
errors = []

# The panel's tick row, from `panel.py` rather than as a literal of its own.
# The lobby coordinates in `table.py` are literals on purpose — if the lobby
# moves, a check should notice — but the panel has one copy of its running
# totals and this is a reader of it, not a tripwire for it. It was a literal
# until M11.2 moved the foot up by twenty-three pixels and this quietly began
# sampling blank panel, reporting that neither peer was ticking.
TICK_ROW = (P.LEFT, P.TICK - 16.0, P.LEFT + 180.0, P.TICK + 6.0)


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


def shot(page, name):
    img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
    img.save(f"/tmp/floodline-{name}.png")
    return img


def crop(img, box):
    x0, y0 = V.css(box[0], box[1])
    x1, y1 = V.css(box[2], box[3])
    return img.crop((int(x0 * DPR), int(y0 * DPR),
                     int(x1 * DPR), int(y1 * DPR))).tobytes()


def painted(img):
    """Is the map drawn? Only a tab that left the lobby draws one."""
    px, (w, h) = img.load(), img.size
    return sum(px[int(w * 0.3), int(h * 0.5)]) > 80


with sync_playwright() as p:
    browsers, pages, room = table.sit_down(p, URL, DPR, on_error=errors.append)
    print(f"  room {room}")
    print(f"  {URL}")
    check(room is not None, "the two browsers found each other in the room")
    if room is None:
        for pg, tag in zip(pages, ("host", "join")):
            shot(pg, f"m10-{tag}-stuck")
        for b in browsers:
            b.close()
        raise SystemExit(1)

    first = [shot(pg, f"m10-{tag}") for pg, tag in zip(pages, ("host", "join"))]
    # A joiner leaves the lobby only when the host's first bundle arrives, so
    # a drawn map on both is a world both of them are in.
    check(painted(first[0]), "the host is drawing the map")
    check(painted(first[1]), "the joiner is drawing the map")

    # And it is running. A drawn map is not a ticking one: a page that arrived
    # and then stopped rendering looks exactly like this for thirty seconds and
    # is then dropped.
    was = [crop(img, TICK_ROW) for img in first]
    time.sleep(3.0)
    now = [crop(shot(pg, f"m10-{tag}-later"), TICK_ROW)
           for pg, tag in zip(pages, ("host", "join"))]
    check(now[0] != was[0], "the host's tick count is advancing")
    check(now[1] != was[1], "the joiner's tick count is advancing")

    for b in browsers:
        b.close()

for e in errors:
    print("ERROR", e)
print("screenshots in /tmp/floodline-m10-*.png")
if errors:
    raise SystemExit(1)
print("OK")
