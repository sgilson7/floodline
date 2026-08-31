"""Do the panel's fixed rows stay put, and its foot stay readable?

`panel_rows_do_not_move_when_a_building_is_chosen`, which is the check
`input.rs::tools` now names. It exists because the answer was no, five times.

It asks two questions. The first is whether choosing a building *moves* the
rows below it. The second, added in M11.2, is whether anything is drawn *over*
the three rows at the foot — `tick`, `peers at` and `build`/`seed` — which are
the rows a player is told to read when something is wrong, and the ones M10
nominated as its desync instrument. The M10.6 run spent twelve minutes with a
trade offer sitting where the tick count belongs, and the referee dutifully
reported a hundred and sixteen stalls that never happened.

An offer needs two peers, so the case exercised here is the cheap one that
fails for the same reason: a selected building puts the level/move row over the
foot on its own. The fix has to make the foot inviolable rather than fit one
particular stack, so this covers both.

The level/move row appears the moment a player clicks one of their own
buildings and disappears when they click away. Every row below it moved by
forty-eight pixels with it, so a script — or an agent — clicking "choose all"
at a written-down coordinate hit the gap above it instead and nothing happened,
silently. Reasoning about the layout has failed at this five times; this asks
the picture.
"""
import io, sys
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View, LOGICAL_W, LOGICAL_H, PANEL_W, CELL
import panel as P

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
W, H = 1400, 900
V = View(W, H)
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

    def shot(name=None):
        img = Image.open(io.BytesIO(pg.screenshot())).convert("RGB")
        if name:
            img.save(f"/tmp/floodline-{name}.png")
        return img

    def strip(img, y0, y1):
        x0, yy0 = V.css(*P.band(y0, y1)[:2])
        x1, yy1 = V.css(*P.band(y0, y1)[2:])
        return img.crop((int(x0), int(yy0), int(x1), int(yy1))).tobytes()

    def plate_pixels(img):
        """How much of the panel's foot is covered by a button.

        The foot should be text on panel background and nothing else. A button
        drawn over it is a broad fill of an intermediate brightness — darker
        than a glyph, lighter than the panel — so counting those separates
        "a row of text" from "a row of text with a plate on top of it".

        Measured rather than guessed: with nothing selected the foot has about
        900 such pixels, which is the antialiasing on its own glyphs. With a
        building selected it has 4 400.
        """
        x0, y0 = V.css(P.LEFT - 12.0, P.TICK - 18.0)
        x1, y1 = V.css(P.RIGHT + 8.0, P.BUILD_SEED + 8.0)
        band = img.crop((int(x0), int(y0), int(x1), int(y1))).convert("RGB")
        return sum(1 for q in band.getdata() if 80 <= sum(q) < 200)

    # Single player by pasted code: no relays, no second tab, and the panel is
    # the whole subject. Two cities would put a second row in the city list and
    # move everything below it, which `panel.py` knows about and this does not
    # need to exercise.
    pg.goto(URL)
    pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    pg.wait_for_timeout(1500)
    pg.mouse.click(*V.css(970.0, 353.0))   # by pasted code
    pg.mouse.click(*V.css(630.0, 518.0))   # Host a game
    pg.wait_for_timeout(1500)
    pg.mouse.click(*V.css(800.0, 652.0))   # Start
    pg.wait_for_timeout(1200)
    pg.mouse.click(*V.css(800.0, 400.0))   # dismiss the first-run card
    pg.wait_for_timeout(800)

    # Two cities, not one. Hosting alone still generates the map with as many
    # cities as there are seats, and the lobby starts at two — a seat nobody
    # takes is a city standing there with nobody commanding it, which is
    # `lobby.rs`'s reason for making seats the one thing decided up front. The
    # panel therefore lists two, and every row below the list is 24 pixels
    # lower than a reading of "single player" would suggest.
    CITIES = 2
    FIXED = (P.chosen_count(CITIES) - 22.0, P.below_the_trade(CITIES))
    VARIABLE = (P.below_the_trade(CITIES), LOGICAL_H - 84.0)

    # Where the hearth is, so a cottage can go down near it.
    px, pts = shot("rows-0-start").load(), []
    x0, y0 = V.css(0.0, 0.0)
    x1, y1 = V.css(LOGICAL_W - PANEL_W, LOGICAL_H)
    for y in range(int(y0), int(y1), 2):
        for x in range(int(x0), int(x1), 2):
            r, g, b = px[x, y]
            if abs(r - BLUE[0]) < 26 and abs(g - BLUE[1]) < 26 and abs(b - BLUE[2]) < 26:
                pts.append((x, y))
    check(bool(pts), "found the city on screen")
    if not pts:
        raise SystemExit(1)
    mx = sum(q[0] for q in pts) / len(pts)
    my = sum(q[1] for q in pts) / len(pts)
    hx, hy = V.map_cell((mx - V.ox) / V.scale, (my - V.oy) / V.scale)

    before = shot("rows-1-nothing-chosen")
    was_fixed = strip(before, *FIXED)
    was_variable = strip(before, *VARIABLE)
    was_foot = plate_pixels(before)

    # A cottage, then click it. A cottage is movable, so the row appears even
    # while it is still a site waiting for its wood — which is what makes this
    # cheap: no need to wait out a build.
    inward = (1 if hx < 64 else -1, 1 if hy < 64 else -1)
    chosen = None
    for step in (6, 10, 14, 20):
        for dx, dy in ((inward[0] * step, 0), (0, inward[1] * step),
                       (inward[0] * step, inward[1] * step)):
            spot = (max(2, min(125, hx + dx)), max(2, min(125, hy + dy)))
            pg.keyboard.press("Digit1")
            pg.mouse.click(*V.cell(*spot))
            pg.wait_for_timeout(500)
            pg.keyboard.press("Escape")
            pg.mouse.click(*V.cell(*spot))
            pg.wait_for_timeout(500)
            if strip(shot(), *VARIABLE) != was_variable:
                chosen = spot
                break
        if chosen:
            break

    after = shot("rows-2-cottage-chosen")
    check(chosen is not None,
          "a cottage went down and clicking it drew the level/move row")
    if not chosen:
        raise SystemExit(1)
    print(f"  hearth ({hx},{hy}), cottage {chosen}")

    # The point of the whole exercise.
    check(strip(after, *FIXED) == was_fixed,
          "the chosen row, the two orders and the trade button did not move")
    # And the variable stack is genuinely below them, not merely absent.
    check(strip(after, *VARIABLE) != was_variable,
          "the level/move row is drawn below everything fixed")

    # But "below everything fixed" has to stop somewhere, and the foot is it.
    now_foot = plate_pixels(after)
    check(now_foot < was_foot * 3 // 2,
          f"nothing is drawn over tick, peers at and build/seed "
          f"({was_foot} -> {now_foot} plate pixels)")

    br.close()

for e in errors:
    print("ERROR", e)
print("screenshots in /tmp/floodline-rows-*.png")
if errors:
    raise SystemExit(1)
print("OK")
