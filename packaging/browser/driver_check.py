"""One of each of the driver's verbs, in a live game, before a run needs them.

`play.py` proves the mouse reaches the simulation. This proves the same thing
through `driver.py`, which is the only way an agent will ever touch the game —
a verb that is missing or wrong is a bug found late otherwise, halfway through
a run that cannot be repeated cheaply.

Every assertion is made by looking at the picture, because that is what an
agent has.
"""
import io, os, subprocess, sys, time
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View, LOGICAL_W, LOGICAL_H, PANEL_W, CELL
import panel as P
import table

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
HERE = os.path.dirname(os.path.abspath(__file__))
DRIVER = os.path.join(HERE, "driver.py")
PORT = table.PORTS[0]                      # the host's browser, and only that one

V = View(table.W, table.H)
errors = []


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


def drive(*args):
    """One verb, the way an agent runs it: a separate process every time."""
    r = subprocess.run([sys.executable, DRIVER, str(PORT)] + [str(a) for a in args],
                       capture_output=True, text=True)
    if r.returncode != 0:
        errors.append(f"driver {' '.join(str(a) for a in args)}: {r.stderr.strip()}")
    return r.stdout.strip()


with sync_playwright() as p:
    browsers, pages, room = table.sit_down(p, URL, ports=table.PORTS,
                                           on_error=errors.append)
    check(room is not None, "the table was set")
    if room is None:
        for b in browsers:
            b.close()
        raise SystemExit(1)
    host = pages[0]

    def shot():
        return Image.open(io.BytesIO(host.screenshot())).convert("RGB")

    def changed_cells(a, b, cx0, cy0, cx1, cy1):
        x0, y0 = V.logical_of_map(cx0 * CELL, cy0 * CELL)
        x1, y1 = V.logical_of_map(cx1 * CELL, cy1 * CELL)
        n = 0
        for ly in range(int(y0), int(y1), 2):
            for lx in range(int(x0), int(x1), 2):
                px, py = V.css(lx, ly)
                if a.getpixel((int(px), int(py))) != b.getpixel((int(px), int(py))):
                    n += 1
        return n

    def strip(img, y0, y1):
        x0, yy0 = V.css(P.LEFT - 10.0, y0)
        x1, yy1 = V.css(P.RIGHT + 10.0, y1)
        return img.crop((int(x0), int(yy0), int(x1), int(yy1))).tobytes()

    def becomes(look, was, seconds=5.0):
        """Wait for a row to stop being what it was.

        Sampled rather than asserted on one frame, because the page repaints
        when it feels like it and this whole file runs inside a suite that has
        a dozen other browsers on the machine. The two hover checks failed
        exactly this way: the baseline was captured before the cursor had
        finished moving off the cottage, so it was compared against itself.
        """
        end = time.time() + seconds
        while time.time() < end:
            if look() != was:
                return True
            host.wait_for_timeout(250)
        return False

    def hover_row(img):
        return strip(img, P.hover() - 18.0, P.hover() + 6.0)

    def chosen_row(img):
        return strip(img, P.chosen_count() - 18.0, P.chosen_count() + 6.0)

    def find_the_city():
        """Where this player's people are, from the colour of their hearth."""
        img = shot()
        px, pts = img.load(), []
        x0, y0 = V.css(0.0, 0.0)
        x1, y1 = V.css(LOGICAL_W - PANEL_W, LOGICAL_H)
        for y in range(int(y0), int(y1), 2):
            for x in range(int(x0), int(x1), 2):
                r, g, b = px[x, y]
                # palette::player(PlayerId(0)) - yellow since M11.8.
                if abs(r - 235) < 26 and abs(g - 217) < 26 and abs(b - 92) < 26:
                    pts.append((x, y))
        if not pts:
            return None
        mx = sum(q[0] for q in pts) / len(pts)
        my = sum(q[1] for q in pts) / len(pts)
        return V.map_cell((mx - V.ox) / V.scale, (my - V.oy) / V.scale)

    city = find_the_city()
    check(city is not None, f"found the city on screen at {city}")
    if city is None:
        for b in browsers:
            b.close()
        raise SystemExit(1)

    # --- the eyes ---------------------------------------------------------
    whole = drive("shot")
    check(os.path.exists(whole), f"shot wrote {whole}")
    only_panel = drive("panel")
    check(os.path.exists(only_panel), f"panel wrote {only_panel}")
    if os.path.exists(whole) and os.path.exists(only_panel):
        check(os.path.getsize(only_panel) < os.path.getsize(whole),
              "the panel crop is cheaper to read than the whole window")
    check(os.path.exists(drive("rows")), "rows wrote the status and the foot")

    # --- a key, and a click on the map -----------------------------------
    # The city sits somewhere on a bank; the middle of the map is the one cell
    # every seed has. A refusal costs nothing and the next spot is tried.
    before = shot()
    drive("key", "Digit1")
    placed = None
    for spot in ((64, 64), (60, 60), (68, 68), (56, 64), (72, 64)):
        drive("click-cell", *spot)
        host.wait_for_timeout(600)
        if changed_cells(before, shot(), spot[0] - 2, spot[1] - 2, spot[0] + 3, spot[1] + 3) > 4:
            placed = spot
            break
    check(placed is not None, f"key + click-cell put a cottage down at {placed}")

    # --- hovering fills the panel's hover row ------------------------------
    # The cursor is still on the cottage from the click that placed it, so the
    # row is already full. Park it on open ground first, or this compares a
    # filled row with a filled row and passes for the wrong reason.
    drive("key", "Escape")
    on_the_cottage = hover_row(shot())
    drive("hover-cell", 2, 2)
    check(becomes(lambda: hover_row(shot()), on_the_cottage),
          "hover-cell moved the cursor off the cottage and the row emptied")
    empty = hover_row(shot())
    if placed:
        drive("hover-cell", *placed)
        check(becomes(lambda: hover_row(shot()), empty),
              "hover-cell filled the panel's hover row")

    # --- a named panel button ---------------------------------------------
    was = chosen_row(shot())
    drive("button", "choose-all")
    check(becomes(lambda: chosen_row(shot()), was),
          "button choose-all changed the chosen row")

    # --- choosing by rectangle, and an order ------------------------------
    # `box-select` over open ground chooses nobody, which is the honest way to
    # tell it apart from `choose all`: the row has to go back to "nobody".
    was = chosen_row(shot())
    drive("box-select", 2, 2, 8, 8)
    check(becomes(lambda: chosen_row(shot()), was),
          "box-select over empty ground chose nobody, and the row said so")

    # Right-click, proved on the one answer it always gives at once: it puts
    # the current tool down. "click the ground. right-click to stop" is the
    # panel's own sentence for it, and the hint row changes on the same frame.
    #
    # Not "did anybody move": over three seconds a bunched city moves a few
    # pixels and the answer is a coin toss. Not "did the game say how many it
    # took" either — that wants a *standing* building, and `assign.py` already
    # waits out a farm's construction to prove exactly that. What is left for
    # this to prove is that the verb reaches the page at all.
    drive("button", "choose-all")
    host.wait_for_timeout(300)
    drive("key", "Digit1")
    host.wait_for_timeout(400)
    hint = lambda: strip(shot(), P.tool_hint() - 16.0, P.tool_hint() + 6.0)
    with_tool = hint()
    drive("right-click-cell", *city)
    check(becomes(hint, with_tool),
          "right-click-cell reached the game: it put the build tool down")

    # --- the one drag in the game -----------------------------------------
    # Retried along a few lines: a dike wants ground that will take it, and a
    # fixed pair of cells is a coin toss about the map rather than a test of
    # the gesture.
    drew = 0
    for row in (96, 100, 92, 104):
        drive("key", "Digit7")
        before = shot()
        drive("drag-cells", 30, row, 42, row)
        host.wait_for_timeout(700)
        drew = changed_cells(before, shot(), 30, row - 2, 44, row + 3)
        if drew > 8:
            print(f"  wall along row {row}")
            break
    check(drew > 8, f"drag-cells drew a wall: {drew} pixels changed")

    # --- the tabs, and the camera -----------------------------------------
    body = lambda: strip(shot(), P.body_top(), P.body_top() + 200.0)
    was_body = body()
    drive("button", "tab-households")
    check(becomes(body, was_body), "button tab-households changed the panel body")
    drive("button", "tab-build")
    host.wait_for_timeout(600)

    # `frame` must leave the camera where `view.py` assumes it: every *-cell
    # verb is wrong the moment it is not.
    drive("frame")
    host.wait_for_timeout(600)
    if placed:
        # The cursor is on a tab button, so the row is already empty; parking
        # it on open ground keeps it that way and gives a baseline to move off.
        drive("hover-cell", 2, 2)
        host.wait_for_timeout(800)
        blank = hover_row(shot())
        drive("hover-cell", *placed)
        check(becomes(lambda: hover_row(shot()), blank),
              "after frame, a cell verb still lands on the cell it names")

    for b in browsers:
        b.close()

for e in errors:
    print("ERROR", e)
if errors:
    raise SystemExit(1)
print("OK")
