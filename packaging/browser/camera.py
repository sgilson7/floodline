"""Zoom and pan, and then: does a click still hit the cell you pointed at?

The coordinate code in this project has been wrong twice, both times because
two places did the same arithmetic and disagreed, and both times it was
invisible at a device pixel ratio of one. The camera is a second transform on
top of the letterbox, so this is the test that matters: put a building down at
a known cell while zoomed and panned, and check the game put it where the
cursor was.
"""
import io, sys, time
from PIL import Image
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
from view import View, LOGICAL_W as LW, LOGICAL_H as LH, PANEL_W as PANEL
from view import WIN_X, WIN_Y, WIN_W, WIN_H

W, H = 1400, 900
errors = []


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


with sync_playwright() as p:
    br = p.chromium.launch()
    V = View(W, H, DPR)
    css = V.css
    pg = br.new_context(viewport={"width": W, "height": H}, device_scale_factor=DPR).new_page()
    pg.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
    pg.on("console", lambda m: errors.append(f"console.error: {m.text}")
          if m.type == "error" else None)

    def shot(name=None):
        img = Image.open(io.BytesIO(pg.screenshot())).convert("RGB")
        if name:
            img.save(f"/tmp/floodline-{name}.png")
        return img

    def px(img, lx, ly):
        return V.px(img, lx, ly)

    pg.goto(URL)
    pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    pg.wait_for_timeout(1500)
    pg.mouse.click(*css(970.0, 353.0))   # by pasted code
    pg.mouse.click(*css(630.0, 518.0))   # Host a game
    pg.wait_for_timeout(1200)
    pg.mouse.click(*css(800.0, 652.0))   # Start
    pg.wait_for_timeout(1000)
    pg.mouse.click(*css(800.0, 400.0))   # dismiss the card
    pg.wait_for_timeout(800)

    fit = shot(f"camera-{DPR:g}-fit")

    # Nothing may be drawn on the panel's side of the window, at any zoom.
    def panel_clean(img, what):
        for ly in range(40, 940, 40):
            r, g, b = px(img, LW - PANEL + 6.0, float(ly))
            # The panel is #0E0E15; terrain is green or grey and much brighter.
            if r + g + b > 90:
                check(False, f"{what}: terrain spilled onto the panel at y={ly}")
                return
        check(True, f"{what}: nothing drawn over the panel")

    panel_clean(fit, "at the fit")

    # Zoom in on the middle of the window, then pan.
    mid = (WIN_X + WIN_W / 2, WIN_Y + WIN_H / 2)
    for _ in range(8):
        pg.mouse.move(*css(*mid))
        pg.mouse.wheel(0, -120)
        pg.wait_for_timeout(60)
    pg.wait_for_timeout(500)
    zoomed = shot(f"camera-{DPR:g}-zoomed")
    check(zoomed.tobytes() != fit.tobytes(), "the wheel changed the view")
    panel_clean(zoomed, "zoomed in")

    pg.keyboard.down("ArrowRight")
    pg.wait_for_timeout(500)
    pg.keyboard.up("ArrowRight")
    pg.wait_for_timeout(400)
    panned = shot(f"camera-{DPR:g}-panned")
    check(panned.tobytes() != zoomed.tobytes(), "the arrow keys panned")
    panel_clean(panned, "panned")

    # The one that matters. Put a stockpile down under the cursor and check the
    # game drew it under the cursor: it is free, one click, and 2x2.
    target = (mid[0] + 90.0, mid[1] - 60.0)
    before = shot()
    pg.keyboard.press("Digit6")          # stockpile
    pg.mouse.move(*css(*target))
    pg.wait_for_timeout(200)
    pg.mouse.click(*css(*target))
    pg.wait_for_timeout(700)
    after = shot(f"camera-{DPR:g}-placed")

    # Something changed within a cell or two of the cursor...
    def changed_near(a, b, lx, ly, reach):
        n = 0
        for dy in range(-reach, reach + 1, 2):
            for dx in range(-reach, reach + 1, 2):
                if px(a, lx + dx, ly + dy) != px(b, lx + dx, ly + dy):
                    n += 1
        return n

    near = changed_near(before, after, target[0], target[1], 24)
    far = changed_near(before, after, target[0] + 260.0, target[1] + 200.0, 24)
    check(near > 20, f"a building appeared under the cursor ({near} pixels)")
    check(far < 8, f"and not somewhere else ({far} pixels changed 260 away)")

    pg.keyboard.press("Escape")
    br.close()

for e in errors:
    print("ERROR", e)
print(f"screenshots in /tmp/floodline-camera-{DPR:g}-*.png")
if errors:
    raise SystemExit(1)
print("OK")
