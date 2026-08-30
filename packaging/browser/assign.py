"""Choose everybody, right-click the farm, and see what the game says.

A farm has three job slots and a command is all-or-nothing, so asking it to
take a whole city of eight was refused *whole*: nobody farmed, the farm stood
empty, and the city starved on day four with a red line under the map that
faded in three seconds as the only sign. The mouse now asks how many will fit
and sends that many.

`crates/sim/tests/scenario.rs::the_opening_a_player_would_play_reaches_the_flood`
proves the city then lives to the water. This proves the gesture reaches the
simulation and says what it did.
"""
import io, sys, time
from PIL import Image
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
W, H = 1400, 900
LW, LH, MAP_X, MAP_Y, CELL = 1600.0, 980.0, 12.0, 12.0, 8.0
scale = min(W / LW, H / LH)
ox, oy = (W - LW * scale) / 2.0, (H - LH * scale) / 2.0
css = lambda lx, ly: (ox + lx * scale, oy + ly * scale)
cell = lambda cx, cy: css(MAP_X + (cx + 0.5) * CELL, MAP_Y + (cy + 0.5) * CELL)
BLUE = (92, 168, 242)          # palette::player(PlayerId(0))
errors = []


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


with sync_playwright() as p:
    br = p.chromium.launch()
    pg = br.new_context(viewport={"width": W, "height": H}).new_page()
    pg.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
    pg.on("console", lambda m: errors.append(f"console.error: {m.text}")
          if m.type == "error" else None)

    def shot():
        return Image.open(io.BytesIO(pg.screenshot())).convert("RGB")

    def own(img):
        """Pixels of this player's own colour: their standing buildings."""
        px, pts = img.load(), []
        for y in range(int(oy), int(oy + LH * scale), 2):
            for x in range(int(ox), int(ox + 1036 * scale), 2):
                r, g, b = px[x, y]
                if abs(r - BLUE[0]) < 26 and abs(g - BLUE[1]) < 26 and abs(b - BLUE[2]) < 26:
                    pts.append((x, y))
        return pts

    def patch(img, cx, cy, w_cells, h_cells):
        """The pixels on these cells, for comparing before and after a click.

        A comparison rather than a colour match: a construction site is a
        one-pixel outline over a fifth-alpha wash, which looks almost exactly
        like the ground it is drawn on, and matching the player's colour finds
        the hearth and the citizens standing around it instead.
        """
        x0, y0 = css(MAP_X + cx * CELL, MAP_Y + cy * CELL)
        x1, y1 = css(MAP_X + (cx + w_cells) * CELL, MAP_Y + (cy + h_cells) * CELL)
        return img.crop((int(x0), int(y0), int(x1), int(y1))).tobytes()

    def alarm_band(img):
        """Red pixels where a refusal or a count is written, under the map."""
        px, n = img.load(), 0
        y0, y1 = css(0.0, LH - 54.0)[1], css(0.0, LH - 14.0)[1]
        for y in range(int(y0), int(y1)):
            for x in range(int(ox), int(ox + 1036 * scale), 2):
                r, g, b = px[x, y]
                if r > g + 40 and r > b + 30 and r > 90:
                    n += 1
        return n

    def hover_line(img):
        """The panel's "what is under the cursor" row: `farm: 0 of 3 working`."""
        x0, y0 = css(1252.0, 548.0)
        x1, y1 = css(1590.0, 578.0)
        return img.crop((int(x0), int(y0), int(x1), int(y1))).tobytes()

    pg.goto(URL)
    pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    pg.wait_for_timeout(1500)
    pg.mouse.click(*css(970.0, 353.0))   # by pasted code
    pg.mouse.click(*css(630.0, 518.0))   # Host a game
    pg.wait_for_timeout(1500)
    pg.mouse.click(*css(800.0, 652.0))   # Start
    pg.wait_for_timeout(2000)

    pts = own(shot())
    check(bool(pts), "found the city on screen")
    if not pts:
        raise SystemExit(1)
    hx = int(((sum(p[0] for p in pts) / len(pts) - ox) / scale - MAP_X) / CELL)
    hy = int(((sum(p[1] for p in pts) / len(pts) - oy) / scale - MAP_Y) / CELL)
    hx, hy = max(10, min(112, hx)), max(10, min(112, hy))

    # A farm somewhere near. The ground by the shore is often shallows or rock,
    # so try a ring until one takes: a refusal costs nothing.
    # Toward the middle of the map, and well clear of the hearth: cities sit
    # eight cells from the edge of the map on the shore side, so half the ring
    # around one is off the map or under it.
    inward = (1 if hx < 64 else -1, 1 if hy < 64 else -1)
    farm = None
    for step in (12, 18, 24, 30):
        for dx, dy in ((inward[0] * step, 0), (0, inward[1] * step),
                       (inward[0] * step, inward[1] * step)):
            spot = (max(2, min(122, hx + dx)), max(2, min(122, hy + dy)))
            was = patch(shot(), spot[0], spot[1], 3, 3)
            pg.keyboard.press("Digit2")
            pg.mouse.click(*cell(*spot))
            pg.wait_for_timeout(700)
            if patch(shot(), spot[0], spot[1], 3, 3) != was:
                farm = spot
                break
        if farm:
            break
    check(farm is not None, "a farm went down")
    if not farm:
        raise SystemExit(1)
    print(f"  hearth ({hx},{hy}), farm {farm}")
    pg.keyboard.press("Escape")

    # Let it get built, then the gesture: choose everybody, right-click the
    # farm. The panel's hover row says how many are working there, so it is
    # read with the cursor over the farm both times.
    pg.wait_for_timeout(45000)   # 450 ticks: hauling plus a farm's 300 builder-ticks
    pg.mouse.move(*cell(*farm))
    pg.wait_for_timeout(400)
    before_work = hover_line(shot())

    pg.mouse.click(*css(1502.0, 644.0))                  # choose all
    pg.wait_for_timeout(400)
    quiet = alarm_band(shot())
    pg.mouse.click(*cell(*farm), button="right")         # put them to work
    pg.wait_for_timeout(900)
    after = shot()
    after.save("/tmp/floodline-assign.png")

    check(alarm_band(after) > quiet + 40,
          "the game said how many of the eight it took")
    check(hover_line(after) != before_work,
          "the farm went from nobody working it to somebody")
    br.close()

for e in errors:
    print("ERROR", e)
print("screenshot in /tmp/floodline-assign.png")
if errors:
    raise SystemExit(1)
print("OK")
