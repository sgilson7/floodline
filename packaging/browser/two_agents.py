"""Two agents, two browsers, one room: the ground M10 is played on.

`game_two_tabs.py` proves the transport with two pages in one context, which
is right for a transport check and wrong for a playtest. Two agents must not
share a clipboard, a `localStorage` or a permission grant — and, over
thirty-six minutes, must not share a renderer scheduler either:

    Lockstep::DROP_AFTER_TICKS is 300 ticks, which is thirty seconds, and
    Clock::MOST_PER_FRAME is 8. So a page has to render at least 1.25 frames a
    second to hold ten ticks a second, and a page that stops for thirty seconds
    is dropped from the game by the other peer.

Chromium throttles animation frames in backgrounded and occluded pages, and two
pages in one browser cannot both be in front. So: two browsers. This is the
lobby dance the run itself will use, and nothing here decides what to build.

    two_agents.py [url] [dpr]
"""
import io, sys, time, random
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View, LOGICAL_H

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

W, H = 1400, 900
V = View(W, H, DPR)
errors = []

# Chromium's throttling, off. Playwright passes all three itself today; they
# are named here anyway, because the reason we need them is ours and not
# Playwright's to keep.
FLAGS = [
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
]

# Lobby geometry, from crates/gui/src/lobby.rs. Literals rather than read from
# anywhere, for the reason game_two_tabs.py gives: if the lobby moves, this
# should notice rather than click a gap and blame something else.
CX = 800.0
BY_ROOM = (CX - 170.0, 353.0)    # start_screen: the "by room code" button
ROOM_FLD = (CX + 20.0, 512.0)    # its room field, which only relay mode has
HOST_BTN = (CX - 170.0, 574.0)
JOIN_BTN = (CX + 170.0, 574.0)
START_BTN = (CX, 534.0)          # hosting_screen's running total, relay mode
CARD = (CX, 400.0)               # anywhere on the modal first-run card

# The panel's tick row: draw.rs puts its baseline at LOGICAL_H - 74 and the
# next row twenty-three below, so this brackets one line and no other.
TICK_ROW = (1252.0, LOGICAL_H - 90.0, 1420.0, LOGICAL_H - 68.0)


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


def wire(page, tag):
    page.on("pageerror", lambda e: errors.append(f"[{tag}] pageerror: {e}"))
    page.on("console", lambda m: errors.append(f"[{tag}] console.error: {m.text}")
            if m.type == "error" else None)


def click(page, lx, ly):
    page.mouse.click(*V.css(lx, ly))


def shot(page, name):
    img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
    img.save(f"/tmp/floodline-{name}.png")
    return img


def crop(img, box):
    x0, y0 = V.css(box[0], box[1])
    x1, y1 = V.css(box[2], box[3])
    return img.crop((int(x0 * DPR), int(y0 * DPR), int(x1 * DPR), int(y1 * DPR))).tobytes()


def painted(img):
    """Is the map drawn? Only a tab that left the lobby draws one."""
    px, (w, h) = img.load(), img.size
    return sum(px[int(w * 0.3), int(h * 0.5)]) > 80


def wait_until(fn, timeout=60):
    end = time.time() + timeout
    while time.time() < end:
        if fn():
            return True
        time.sleep(0.4)
    return False


def linked(page):
    return page.evaluate("window.FLOODLINE_RTC.debug()?.links.size || 0") > 0


with sync_playwright() as p:
    # Two browsers, not two contexts: see the note at the top.
    browsers = [p.chromium.launch(args=FLAGS) for _ in range(2)]
    pages = []
    for b, tag in zip(browsers, ("host", "join")):
        page = b.new_context(viewport={"width": W, "height": H},
                             device_scale_factor=DPR).new_page()
        wire(page, tag)
        pages.append(page)
    host, join = pages

    room = "fl-m10-" + str(random.randint(100000, 999999))
    print(f"  room {room}")
    print(f"  {URL}")

    for pg in pages:
        pg.goto(URL)
        pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    time.sleep(1.5)

    # The room-code path only. The pasted-code path is for players behind
    # strict NATs and has nothing to teach a playtest.
    for pg in pages:
        click(pg, *BY_ROOM)
        click(pg, *ROOM_FLD)
        pg.keyboard.type(room)
    # Seats are left at two, which is what `Lobby::new` starts them at and what
    # a two-agent game wants: a seat nobody takes is a city standing empty.
    click(host, *HOST_BTN)
    time.sleep(0.5)
    click(join, *JOIN_BTN)

    ok = wait_until(lambda: linked(host) and linked(join))
    check(ok, "the two browsers found each other in the room")
    if not ok:
        shot(host, "m10-host-stuck")
        shot(join, "m10-join-stuck")
        for b in browsers:
            b.close()
        raise SystemExit(1)

    click(host, *START_BTN)
    time.sleep(2.0)
    # The first-run card is modal and covers the map, on both.
    for pg in pages:
        click(pg, *CARD)
    time.sleep(1.5)

    first = [shot(host, "m10-host"), shot(join, "m10-join")]
    # A joiner leaves the lobby only when the host's first bundle arrives, so
    # a drawn map on both is a world both of them are in.
    check(painted(first[0]), "the host is drawing the map")
    check(painted(first[1]), "the joiner is drawing the map")

    # And it is running. A drawn map is not a ticking one: a page that arrived
    # and then stopped rendering looks exactly like this for thirty seconds and
    # is then dropped. Two samples of the tick row, three seconds apart.
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
