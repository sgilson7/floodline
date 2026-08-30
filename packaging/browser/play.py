"""Actually play: choose citizens, put things down, lay a road, offer a trade.

The one thing `cargo test` cannot answer is whether the mouse reaches the
simulation, and this is the only place in the suite where a `Command` starts
its life as a click. It hosts a single-player game by pasted code — no relays,
no second tab — so anything that fails here is the input and nothing else.

Every assertion is made by looking at the picture, because that is what a
player has. The coordinates are the running totals in `crates/gui/src/input.rs`
and `crates/gui/src/lobby.rs`: if a panel is rearranged this test should notice
rather than quietly click on nothing.
"""
import io, sys
from PIL import Image
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
W, H = 1400, 900
LOGICAL_W, LOGICAL_H = 1600.0, 980.0
MAP_X, MAP_Y, CELL = 12.0, 12.0, 8.0
PANEL_L = LOGICAL_W - 366.0 + 18.0
errors = []

scale = min(W / LOGICAL_W, H / LOGICAL_H)
ox = (W - LOGICAL_W * scale) / 2.0
oy = (H - LOGICAL_H * scale) / 2.0


def css(lx, ly):
    """Logical canvas -> CSS pixels. No device pixel ratio: the mouse is
    already logical, and applying the ratio here is the input half of the
    letterbox trap."""
    return ox + lx * scale, oy + ly * scale


def cell_px(cx, cy):
    return css(MAP_X + (cx + 0.5) * CELL, MAP_Y + (cy + 0.5) * CELL)


def wire(page, tag):
    page.on("pageerror", lambda e: errors.append(f"[{tag}] pageerror: {e}"))
    page.on("console", lambda m: errors.append(f"[{tag}] console.error: {m.text}")
            if m.type == "error" else None)


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(viewport={"width": W, "height": H}, device_scale_factor=DPR)
    page = ctx.new_page()
    wire(page, "play")

    def shot(name):
        img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
        img.save(f"/tmp/floodline-{name}.png")
        return img

    def at(img, lx, ly):
        x, y = css(lx, ly)
        return img.getpixel((int(x * DPR), int(y * DPR)))

    def at_cell(img, cx, cy):
        x, y = cell_px(cx, cy)
        return img.getpixel((int(x * DPR), int(y * DPR)))

    def changed(a, c, x0, y0, x1, y1):
        """How many pixels of a logical rectangle differ between two frames.

        Counted over an area rather than sampled at a point, because most of
        what this test looks for is a thin line: a road routes around water and
        need not pass through the exact cell it was aimed at, and a ping is an
        outline whose centre is untouched.
        """
        n = 0
        for ly in range(int(y0), int(y1), 2):
            for lx in range(int(x0), int(x1), 2):
                px, py = css(lx, ly)
                px, py = int(px * DPR), int(py * DPR)
                if a.getpixel((px, py)) != c.getpixel((px, py)):
                    n += 1
        return n

    click = lambda lx, ly: page.mouse.click(*css(lx, ly))

    page.goto(URL)
    page.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    page.wait_for_timeout(1200)

    click(970.0, 353.0)   # by pasted code
    click(630.0, 518.0)   # Host a game
    page.wait_for_timeout(1200)
    click(800.0, 652.0)   # Start
    page.wait_for_timeout(1500)
    before = shot("play-0-start")

    # --- selection and orders ------------------------------------------------
    # "choose all", then right-click a far corner: they should set off.
    click(PANEL_L + 250, 601)
    page.wait_for_timeout(300)
    page.mouse.click(*cell_px(70, 70), button="right")
    page.wait_for_timeout(2500)
    moving = shot("play-1-moving")
    # The citizens start bunched at the hearth. Somebody having left the cell
    # they were standing in is the only sign from outside that MoveTo arrived.
    check(moving.tobytes() != before.tobytes(), "the world changed after an order")

    # --- building ------------------------------------------------------------
    page.keyboard.press("Digit1")
    page.wait_for_timeout(200)
    was = at_cell(shot("play-2-tool"), 40, 40)
    click(MAP_X + 40 * CELL + 4, MAP_Y + 40 * CELL + 4)
    page.wait_for_timeout(600)
    now = at_cell(shot("play-3-cottage"), 40, 40)
    check(now != was, f"a cottage site appeared at (40,40): {was} -> {now}")

    # --- a road --------------------------------------------------------------
    # Aimed along row 30 but scored over rows 26 to 36: `lay_road` takes the
    # cheapest path and will go round a wet cell rather than through it.
    was = shot("play-3-cottage")
    page.keyboard.press("KeyR")
    click(MAP_X + 30 * CELL + 4, MAP_Y + 30 * CELL + 4)
    page.wait_for_timeout(250)
    click(MAP_X + 50 * CELL + 4, MAP_Y + 30 * CELL + 4)
    page.wait_for_timeout(700)
    now = shot("play-4-road")
    n = changed(was, now, MAP_X + 30 * CELL, MAP_Y + 26 * CELL,
                MAP_X + 50 * CELL, MAP_Y + 36 * CELL)
    check(n > 40, f"a road was laid between (30,30) and (50,30): {n} pixels changed")

    # --- pointing ------------------------------------------------------------
    was = shot("play-4-road")
    page.keyboard.press("KeyP")
    click(MAP_X + 60 * CELL + 4, MAP_Y + 60 * CELL + 4)
    page.wait_for_timeout(300)
    now = shot("play-5-ping")
    n = changed(was, now, MAP_X + 57 * CELL, MAP_Y + 57 * CELL,
                MAP_X + 64 * CELL, MAP_Y + 64 * CELL)
    check(n > 5, f"a ping is drawn around (60,60): {n} pixels changed")

    # --- the trade dialog ----------------------------------------------------
    # Card at (340, 260) 620x420; the rows are input.rs's running total.
    click(PANEL_L + 165, 671)                # "propose a trade"
    page.wait_for_timeout(400)
    opened = shot("play-6-trade")
    check(at(opened, 650.0, 500.0) != at(before, 650.0, 500.0), "the trade dialog opened")
    click(770.0, 451.0)                      # "+" on the give row
    page.wait_for_timeout(250)
    stepped = shot("play-7-stepped")
    n = changed(opened, stepped, 680.0, 434.0, 750.0, 468.0)
    check(n > 3, f"the amount changed when + was pressed: {n} pixels")
    click(535.0, 570.0)                      # "propose it"
    page.wait_for_timeout(500)
    closed = shot("play-8-proposed")
    check(at(closed, 650.0, 500.0) != at(stepped, 650.0, 500.0),
          "the dialog closed once the trade was proposed")

    # --- a refusal says so ---------------------------------------------------
    # A dike on the hearth is illegal, and `Lockstep::issue` checks locally, so
    # the sentence appears under the map on this frame rather than nothing
    # happening three ticks later.
    page.keyboard.press("Digit5")
    click(MAP_X + 40 * CELL + 4, MAP_Y + 40 * CELL + 4)   # onto the cottage site
    page.wait_for_timeout(300)
    refused = shot("play-9-refused")
    strip = [at(refused, x, LOGICAL_H - 34.0) for x in range(400, 900, 8)]
    check(any(sum(c) > 90 for c in strip), "a refusal is written under the map")

    b.close()

for e in errors:
    print("ERROR", e)
print("screenshots in /tmp/floodline-play-*.png")
if errors:
    raise SystemExit(1)
print("OK")
