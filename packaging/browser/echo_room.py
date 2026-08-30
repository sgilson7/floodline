"""Three tabs on a Trystero room: a host and two joiners, and the star."""
import sys, time, random
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/echo.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
ROOM = "fl-test-" + str(random.randint(100000, 999999))

def wire(page, tag):
    page.on("pageerror", lambda e: print(f"[{tag}] PAGEERROR {e}"))
    page.on("console", lambda m: print(f"[{tag}] console.{m.type}: {m.text}")
            if m.type == "error" else None)

def wait(pages, expr, what, timeout=60):
    end = time.time() + timeout
    while time.time() < end:
        if all(pg.evaluate(expr) for pg in pages):
            return time.time()
        time.sleep(0.25)
    for pg in pages:
        print("  errors:", pg.evaluate("window.ECHO.errors"),
              "peers:", pg.evaluate("window.ECHO.peers"))
    raise SystemExit(f"timed out waiting for {what}")

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(device_scale_factor=DPR)
    host = ctx.new_page(); wire(host, "host")
    a = ctx.new_page(); wire(a, "A")
    c = ctx.new_page(); wire(c, "B")
    for pg in (host, a, c):
        pg.goto(URL)
        pg.wait_for_selector("#host-room")
        pg.fill("#room", ROOM)

    print("room", ROOM, "strategy", host.evaluate("FLOODLINE_CONFIG.strategy"))
    t0 = time.time()
    host.click("#host-room")
    a.click("#join-room")
    t = wait([host, a], "window.ECHO.peers.length > 0", "the host and A to meet")
    print(f"A joined   {t-t0:.1f}s")

    t1 = time.time()
    c.click("#join-room")
    t = wait([c], "window.ECHO.peers.length > 0", "B to meet the host")
    print(f"B joined   {t-t1:.1f}s")
    wait([host], "window.ECHO.peers.length === 2", "the host to have both joiners")

    wait([host, a, c],
         "window.ECHO.got.reliable > 1 && window.ECHO.got.unreliable > 1",
         "bytes on both channels everywhere")
    for name, pg in (("host", host), ("A", a), ("B", c)):
        print(f"{name:5}", pg.evaluate("({peers: ECHO.peers, rtt: ECHO.rtt, got: ECHO.got})"))

    # The star: a joiner must see exactly one peer, the host, even though
    # Trystero has introduced it to the other joiner as well.
    for name, pg in (("A", a), ("B", c)):
        n = pg.evaluate("window.ECHO.peers.length")
        assert n == 1, f"{name} sees {n} peers; the star allows one"
    print("star       joiners see one peer each")

    a.close()
    end = time.time() + 20
    while time.time() < end and host.evaluate("window.ECHO.peers.length") != 1:
        time.sleep(0.25)
    print(f"left seen  {host.evaluate('window.ECHO.peers.length') == 1} "
          f"after {20 - (end - time.time()):.1f}s")

    print("host errors:", host.evaluate("window.ECHO.errors"))
    print("B errors:", c.evaluate("window.ECHO.errors"))
    b.close()
    print("OK")
