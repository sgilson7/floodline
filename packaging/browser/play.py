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
from view import View, LOGICAL_W, LOGICAL_H, CELL

W, H = 1400, 900
PANEL_L = LOGICAL_W - 366.0 + 18.0
errors = []

V = View(W, H)
css = V.css
cell_px = V.cell


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

    def changed_cells(a, c, cx0, cy0, cx1, cy1):
        """How many sampled pixels differ over a block of map cells."""
        x0, y0 = V.logical_of_map(cx0 * CELL, cy0 * CELL)
        x1, y1 = V.logical_of_map(cx1 * CELL, cy1 * CELL)
        return changed(a, c, x0, y0, x1, y1)

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
    page.wait_for_timeout(1200)
    click(800.0, 400.0)   # dismiss the first-run card, which is modal
    page.wait_for_timeout(800)
    before = shot("play-0-start")

    # --- selection and orders ------------------------------------------------
    # "choose all", then right-click a far corner: they should set off.
    click(PANEL_L + 250, 747)
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
    page.mouse.click(*cell_px(40, 40))
    page.wait_for_timeout(600)
    now = at_cell(shot("play-3-cottage"), 40, 40)
    check(now != was, f"a cottage site appeared at (40,40): {was} -> {now}")

    # --- a road --------------------------------------------------------------
    # Short, near the cottage that just went down, and retried along a few
    # lines: rock is impassable and a road cannot cross it, so a fixed pair of
    # cells is a coin toss about the map rather than a test of the road tool.
    # ("no way through" is the right answer when it happens - see the message
    # under the map.)
    laid = 0
    for row in (44, 36, 48, 32):
        was = shot("play-3-cottage")
        page.keyboard.press("KeyR")
        page.mouse.click(*cell_px(38, row))
        page.wait_for_timeout(250)
        page.mouse.click(*cell_px(50, row))
        page.wait_for_timeout(700)
        now = shot("play-4-road")
        laid = changed_cells(was, now, 38, row - 4, 50, row + 4)
        if laid > 40:
            print(f"  road along row {row}")
            break
    check(laid > 40, f"a road was laid near the cottage: {laid} pixels changed")

    # --- pointing ------------------------------------------------------------
    was = shot("play-4-road")
    page.keyboard.press("KeyP")
    page.mouse.click(*cell_px(60, 60))
    page.wait_for_timeout(300)
    now = shot("play-5-ping")
    n = changed_cells(was, now, 57, 57, 64, 64)
    check(n > 5, f"a ping is drawn around (60,60): {n} pixels changed")

    # --- the trade dialog ----------------------------------------------------
    # Card at (340, 260) 620x420; the rows are input.rs's running total.
    click(PANEL_L + 165, 817)                # "propose a trade"
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

    # --- a wall, drawn ------------------------------------------------------
    # The one gesture in the game that is a drag rather than a click, and the
    # only place a `DikeLine` starts life as a mouse. Drawn well clear of the
    # city so nothing it meets is already occupied, and checked over the whole
    # run rather than at one cell, because the run snaps to a whole number of
    # three-cell segments and may overshoot where the button came up.
    page.keyboard.press("Digit7")
    page.wait_for_timeout(200)
    empty = shot("play-9-wall-tool")
    page.mouse.move(*cell_px(52, 30))
    page.mouse.down()
    page.mouse.move(*cell_px(64, 30), steps=8)
    ghost = shot("play-10-wall-ghost")
    n = changed_cells(empty, ghost, 52, 29, 66, 32)
    check(n > 8, f"a ghost of the run followed the drag: {n} pixels")
    page.mouse.up()
    page.wait_for_timeout(700)
    built = shot("play-11-wall")
    n = changed_cells(empty, built, 52, 29, 66, 32)
    check(n > 8, f"a wall of dike sites went down along the drag: {n} pixels")

    # --- a refusal says so ---------------------------------------------------
    # A dike on the hearth is illegal, and `Lockstep::issue` checks locally, so
    # the sentence appears under the map on this frame rather than nothing
    # happening three ticks later.
    page.keyboard.press("Digit7")   # dike
    page.mouse.click(*cell_px(40, 40))   # onto the cottage site
    page.wait_for_timeout(300)
    refused = shot("play-12-refused")
    strip = [at(refused, x, LOGICAL_H - 34.0) for x in range(400, 900, 8)]
    check(any(sum(c) > 90 for c in strip), "a refusal is written under the map")

    b.close()

for e in errors:
    print("ERROR", e)
print("screenshots in /tmp/floodline-play-*.png")
if errors:
    raise SystemExit(1)
print("OK")
