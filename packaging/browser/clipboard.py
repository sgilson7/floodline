"""Does the Copy button put the invitation on the clipboard?

It did not, and the reason is worth keeping in mind for anything else that has
to happen "when the player clicks". `navigator.clipboard.writeText` is only
allowed while a user gesture is live, and macroquad reads a click in the
animation frame *after* the browser delivered it — by which time it is not. The
rejection came back as a failed promise, which `try`/`catch` cannot see, so the
old `execCommand` fallback never ran either: the button did nothing, silently,
on the one screen whose whole content is a string you have to get to somebody
else.

The plugin now copies inside the canvas's own click handler, using the text and
the button rectangle Rust hands it each frame. This test runs with clipboard
*writing denied*, which is the case the fallback is for.
"""
import sys, time
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
W, H = 1400, 900
scale = min(W / 1600.0, H / 980.0)
ox, oy = (W - 1600.0 * scale) / 2.0, (H - 980.0 * scale) / 2.0
css = lambda lx, ly: (ox + lx * scale, oy + ly * scale)
errors = []


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


with sync_playwright() as p:
    br = p.chromium.launch()
    ctx = br.new_context(viewport={"width": W, "height": H}, permissions=["clipboard-read"])
    pg = ctx.new_page()
    pg.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
    pg.goto(URL)
    pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    pg.wait_for_timeout(1500)
    pg.mouse.click(*css(970.0, 353.0))    # by pasted code
    pg.mouse.click(*css(630.0, 518.0))    # Host a game
    pg.wait_for_timeout(3000)

    blob = pg.evaluate("window.FLOODLINE_RTC.codeLocal()")
    check(bool(blob), "the host has an invitation to give away")
    pg.mouse.click(*css(800.0, 404.0))    # the Copy button
    pg.wait_for_timeout(600)
    got = pg.evaluate("navigator.clipboard.readText()")
    check(got == blob, f"it is on the clipboard ({len(got or '')} of {len(blob or '')} characters)")

    # And clicking somewhere else does not copy: the plugin checks the click
    # against the rectangle Rust drew the button in.
    pg.mouse.click(*css(300.0, 700.0))
    pg.wait_for_timeout(400)
    check(pg.evaluate("navigator.clipboard.readText()") == blob,
          "a click somewhere else leaves the clipboard alone")
    br.close()

for e in errors:
    print("ERROR", e)
if errors:
    raise SystemExit(1)
print("OK")
