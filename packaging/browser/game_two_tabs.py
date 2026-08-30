"""Two tabs of the real game: the lobby, the join, and one shared world.

This is phase 4's last item and phase 5's first: `net::Loopback` swapped for
`net-web` under the same lockstep `cargo test -p net` proves, with the whole
stack in the picture — wasm, sapp-jsutils, the plugin, WebRTC and `sim`.
"""
import io, sys, time, random
from PIL import Image
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
MODE = sys.argv[3] if len(sys.argv) > 3 else "room"

W, H = 1400, 900
errors = []


def wire(page, tag):
    page.on("pageerror", lambda e: errors.append(f"[{tag}] pageerror: {e}"))
    page.on("console", lambda m: errors.append(f"[{tag}] console.error: {m.text}")
            if m.type == "error" else None)


def canvas_xy(page, lx, ly):
    """Logical canvas coordinates -> CSS pixels on the page.

    The same letterbox arithmetic `screen::Viewport` does, from the outside.
    Playwright clicks in CSS pixels, which is what `mouse_position()` reports,
    so no device pixel ratio appears here — and that is the half of the
    letterbox trap that bites input rather than drawing.
    """
    scale = min(W / 1600.0, H / 980.0)
    ox = (W - 1600.0 * scale) / 2.0
    oy = (H - 980.0 * scale) / 2.0
    return ox + lx * scale, oy + ly * scale


def click_logical(page, lx, ly):
    x, y = canvas_xy(page, lx, ly)
    page.mouse.click(x, y)


def shot(page, name):
    img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
    img.save(f"/tmp/floodline-{name}.png")
    return img


def paste(page):
    """Whatever this platform's browser calls paste.

    Chromium raises the `paste` event for ⌘V on a Mac and ctrl-V elsewhere, and
    only for the right one — so both are pressed and the field takes whichever
    arrives. The game watches the clipboard for a change rather than watching
    for the keystroke, which is why either works.
    """
    page.keyboard.press("Meta+v")
    page.keyboard.press("Control+v")


def wait_until(fn, what, timeout=45):
    end = time.time() + timeout
    while time.time() < end:
        if fn():
            return True
        time.sleep(0.4)
    return False


with sync_playwright() as p:
    b = p.chromium.launch()
    # The pasted-code path is a clipboard path; a headless browser has to be
    # told it may have one.
    ctx = b.new_context(
        viewport={"width": W, "height": H},
        device_scale_factor=DPR,
        permissions=["clipboard-read", "clipboard-write"],
    )
    host, join = ctx.new_page(), ctx.new_page()
    wire(host, "host"); wire(join, "join")
    room = "fl-game-" + str(random.randint(100000, 999999))

    for pg in (host, join):
        pg.goto(URL)
        pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    time.sleep(1.5)

    # Lobby geometry, from crates/gui/src/lobby.rs. Kept as literals rather
    # than read from anywhere: if the lobby moves, this test should notice.
    CX = 800.0
    BY_ROOM   = (CX - 170.0, 353.0)
    BY_CODE   = (CX + 170.0, 353.0)
    ROOM_FLD  = (CX + 20.0,  512.0)
    # The Host/Join row sits 56 logical pixels lower when there is a room
    # field above it, which there only is in room mode.
    BTN_Y     = 574.0 if MODE == "room" else 518.0
    HOST_BTN  = (CX - 170.0, BTN_Y)
    JOIN_BTN  = (CX + 170.0, BTN_Y)

    if MODE == "room":
        for pg in (host, join):
            click_logical(pg, *BY_ROOM)
        # The room field: click it, type the code.
        for pg in (host, join):
            click_logical(pg, *ROOM_FLD)
            pg.keyboard.type(room)
        click_logical(host, *HOST_BTN)
        time.sleep(0.5)
        click_logical(join, *JOIN_BTN)
    else:
        for pg in (host, join):
            click_logical(pg, *BY_CODE)
        click_logical(host, *HOST_BTN)
        click_logical(join, *JOIN_BTN)
        # The host's invitation is on its clipboard once it presses copy; read
        # it out of the page instead, which is what a player's eyes do.
        time.sleep(2.0)
        blob = host.evaluate("window.FLOODLINE_RTC.codeLocal()")
        if not blob:
            errors.append("the host never produced an invitation")
        else:
            print(f"invitation {len(blob)} characters")
            join.evaluate("b => navigator.clipboard.writeText(b)", blob)
            click_logical(join, CX, 320.0)          # the paste field
            paste(join)
            time.sleep(0.3)
            click_logical(join, CX, 382.0)          # "use it"
            time.sleep(2.5)
            reply = join.evaluate("window.FLOODLINE_RTC.codeLocal()")
            print(f"reply      {len(reply) if reply else 0} characters")
            host.evaluate("b => navigator.clipboard.writeText(b)", reply)
            click_logical(host, CX, 490.0)          # the reply field
            paste(host)
            time.sleep(0.3)
            click_logical(host, CX, 548.0)          # "use it"

    ok = wait_until(
        lambda: host.evaluate("window.FLOODLINE_RTC.debug()?.links.size || 0") > 0
        and join.evaluate("window.FLOODLINE_RTC.debug()?.links.size || 0") > 0,
        "the two tabs to connect",
    )
    print("connected  ", ok)
    if not ok:
        shot(host, "host-stuck"); shot(join, "join-stuck")
        errors.append("the two tabs never connected")
    else:
        shot(host, f"host-lobby-{MODE}")
        # Start. Its y is where lobby.rs's running total puts it, which is a
        # different total in each mode because the pasted-code screen has two
        # more boxes above it.
        start_y = 534.0 if MODE == "room" else 652.0
        click_logical(host, CX, start_y)
        time.sleep(3.0)

    hi = shot(host, f"host-{MODE}")
    ji = shot(join, f"join-{MODE}")
    # The map is drawn on both, which means both left the lobby: the joiner
    # only does that when the host's first bundle arrives.
    def painted(img):
        px, (w, h) = img.load(), img.size
        return sum(px[int(w * 0.3), int(h * 0.5)]) > 80
    print("host drawing the map ", painted(hi))
    print("join drawing the map ", painted(ji))
    if not (painted(hi) and painted(ji)):
        errors.append("one of the tabs never left the lobby")

    b.close()

for e in errors:
    print("ERROR", e)
print("screenshots in /tmp/floodline-*.png")
if errors:
    raise SystemExit(1)
print("OK")
