"""The real game in a real browser: does it load, letterbox and run?

Everything here is about the wasm, which `echo.html` deliberately does not
touch. Run at a device pixel ratio of 1 and of 2 — `Camera2D::viewport` is in
framebuffer pixels and the mouse is in logical ones, and getting that wrong is
invisible at 1 and puts the whole game in a corner at 2.
"""
import io
import sys
from PIL import Image
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

# crates/gui/src/screen.rs and crates/gui/src/draw.rs, and sim::MAP_W.
LOGICAL_W, LOGICAL_H = 1600.0, 980.0
MAP_X, MAP_Y, MAP_PX = 12.0, 12.0, 128 * 8.0
WINDOW_W, WINDOW_H = 1400, 900

errors = []


def wire(page, tag):
    page.on("pageerror", lambda e: errors.append(f"[{tag}] pageerror: {e}"))
    page.on(
        "console",
        lambda m: errors.append(f"[{tag}] console.error: {m.text}") if m.type == "error" else None,
    )


def shot(page):
    """A screenshot, not a `drawImage` of the canvas.

    WebGL clears its drawing buffer at presentation unless
    `preserveDrawingBuffer` is set, so reading the canvas back from JS gives an
    entirely black image whatever the game is doing. The compositor's copy is
    the one that tells the truth, and the bug this exists to catch — a correct
    frame drawn into the wrong quarter of the window — is only visible in a
    picture.
    """
    return Image.open(io.BytesIO(page.screenshot())).convert("RGB")


def map_bounds(img):
    """Where the terrain actually landed, in screenshot pixels.

    The map is the only large block of colour on the page; the panel and the
    letterbox bars are both near-black. So the bounding box of anything
    brighter than they are *is* the map, and comparing it with where the
    letterbox says the map should be is the whole test.
    """
    w, h = img.size
    px = img.load()
    step = 4
    xs, ys = [], []
    for y in range(0, h, step):
        for x in range(0, w, step):
            r, g, b = px[x, y]
            if r + g + b > 120:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), min(ys), max(xs), max(ys)


def letterbox(img):
    """The same arithmetic `screen::Viewport` does, from the outside."""
    w, h = img.size
    scale = min(w / DPR / LOGICAL_W, h / DPR / LOGICAL_H)
    ox = (w / DPR - LOGICAL_W * scale) / 2.0
    oy = (h / DPR - LOGICAL_H * scale) / 2.0
    return ox * DPR, oy * DPR, LOGICAL_W * scale * DPR, LOGICAL_H * scale * DPR, scale


with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(
        viewport={"width": WINDOW_W, "height": WINDOW_H}, device_scale_factor=DPR
    )
    page = ctx.new_page()
    wire(page, f"dpr{DPR:g}")
    page.goto(URL)
    page.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    page.wait_for_timeout(1500)

    # Into a game. Hosting by pasted code needs no relays and no second tab, so
    # this stays a test of the letterbox and nothing else.
    #
    # These are logical-canvas coordinates from crates/gui/src/lobby.rs, put
    # through the same letterbox arithmetic the game does — in CSS pixels, with
    # no device pixel ratio anywhere, because that is what `mouse_position()`
    # reports and getting *that* wrong is the other half of the trap.
    scale = min(WINDOW_W / LOGICAL_W, WINDOW_H / LOGICAL_H)
    ox = (WINDOW_W - LOGICAL_W * scale) / 2.0
    oy = (WINDOW_H - LOGICAL_H * scale) / 2.0
    click = lambda lx, ly: page.mouse.click(ox + lx * scale, oy + ly * scale)
    click(970.0, 353.0)   # by pasted code
    click(630.0, 518.0)   # Host a game
    page.wait_for_timeout(1200)
    click(800.0, 652.0)   # Start
    page.wait_for_timeout(1200)

    img = shot(page)
    got = map_bounds(img)
    lx, ly, lw, lh, scale = letterbox(img)
    print(f"dpr {DPR:g}  screenshot {img.size[0]}x{img.size[1]}")
    if got is None:
        errors.append(f"nothing was drawn at all at dpr {DPR:g}")
    else:
        print("          drawn      ({:.0f}, {:.0f})-({:.0f}, {:.0f})".format(*got))
        print("          letterbox  ({:.0f}, {:.0f}) {:.0f}x{:.0f}".format(lx, ly, lw, lh))

        # Where the map's own top-left corner has to be. This is the number the
        # letterbox bug moved: a viewport computed in logical pixels and handed
        # to GL put the whole frame in the bottom-left quarter, so the content
        # started halfway down the window instead of twelve logical pixels from
        # the top of the canvas.
        want_x = lx + MAP_X * scale * DPR
        want_y = ly + MAP_Y * scale * DPR
        slack = 0.02 * max(img.size)
        off = max(abs(got[0] - want_x), abs(got[1] - want_y))
        print(f"          map corner wanted ({want_x:.0f}, {want_y:.0f}), "
              f"off by {off:.0f}px (allowed {slack:.0f})")
        if off > slack:
            errors.append(
                f"the drawing does not start where the letterbox says at dpr {DPR:g}"
            )

        # And it fills the letterbox rather than a corner of it. The panel runs
        # to the right edge of the logical canvas and the map to the bottom of
        # it, so anything much under the whole rect means the viewport is the
        # wrong size — the other half of the same bug.
        fill_w = (got[2] - got[0]) / lw
        fill_h = (got[3] - got[1]) / lh
        print(f"          fills {fill_w:.0%} x {fill_h:.0%} of the letterbox")
        if fill_w < 0.85 or fill_h < 0.85:
            errors.append(
                f"the drawing covers {fill_w:.0%}x{fill_h:.0%} of the letterbox "
                f"at dpr {DPR:g} — it should be nearly all of it"
            )

        # Outside the bars is the backdrop and nothing else.
        px = img.load()
        if ly > 4:
            r, g, bl = px[img.size[0] // 2, int(ly / 2)]
            if r + g + bl > 60:
                errors.append(f"something is drawn in the letterbox bar at dpr {DPR:g}")

    # And it keeps running.
    page.wait_for_timeout(2500)
    if map_bounds(shot(page)) is None:
        errors.append("the map stopped being drawn")
    else:
        print("          still drawing a few seconds in")

    b.close()

for e in errors:
    print("ERROR", e)
if errors:
    raise SystemExit(1)
print("OK")
