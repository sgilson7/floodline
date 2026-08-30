"""The torrent strategy, and the build-hash prefix keeping a stale tab out."""
import sys, time, random
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/echo.html"

def wire(page, tag):
    page.on("pageerror", lambda e: print(f"[{tag}] PAGEERROR {e}"))
    page.on("console", lambda m: print(f"[{tag}] console.error: {m.text}")
            if m.type == "error" else None)

def wait(pages, expr, what, timeout=45):
    end = time.time() + timeout
    while time.time() < end:
        if all(pg.evaluate(expr) for pg in pages):
            return time.time()
        time.sleep(0.25)
    for pg in pages:
        print("  errors:", pg.evaluate("window.ECHO.errors"))
    raise SystemExit(f"timed out waiting for {what}")

with sync_playwright() as p:
    b = p.chromium.launch()

    # --- the torrent strategy -------------------------------------------
    ctx = b.new_context()
    ctx.add_init_script("window.addEventListener('DOMContentLoaded',()=>{"
                        "window.FLOODLINE_CONFIG.strategy='torrent';});")
    room = "fl-tor-" + str(random.randint(100000, 999999))
    h, j = ctx.new_page(), ctx.new_page()
    wire(h, "tor-host"); wire(j, "tor-join")
    for pg in (h, j):
        pg.goto(URL); pg.wait_for_selector("#host-room"); pg.fill("#room", room)
    print("strategy:", h.evaluate("FLOODLINE_CONFIG.strategy"))
    t0 = time.time()
    h.click("#host-room"); j.click("#join-room")
    try:
        wait([h, j], "window.ECHO.peers.length > 0", "the torrent trackers", 45)
        wait([h, j], "window.ECHO.got.reliable > 0 && window.ECHO.got.unreliable > 0",
             "bytes on both channels")
        print(f"torrent    connected in {time.time()-t0:.1f}s, "
              f"host {h.evaluate('ECHO.got')}")
    except SystemExit as e:
        print("torrent    FAILED:", e)
    ctx.close()

    # --- the build hash in the room name --------------------------------
    room = "fl-stale-" + str(random.randint(100000, 999999))
    c1 = b.new_context(); c2 = b.new_context()
    # After the page's own stamp, not before it: add_init_script runs first
    # and index.html/echo.html then overwrite the global.
    c2.add_init_script("window.addEventListener('DOMContentLoaded',()=>{"
                       "window.FLOODLINE_BUILD='a-different-build';});")
    old, new = c1.new_page(), c2.new_page()
    wire(old, "old"); wire(new, "new")
    for pg in (old, new):
        pg.goto(URL); pg.wait_for_selector("#host-room"); pg.fill("#room", room)
    print("builds:", old.evaluate("FLOODLINE_BUILD"), "vs", new.evaluate("FLOODLINE_BUILD"))
    old.click("#host-room"); new.click("#join-room")
    time.sleep(15)
    met = old.evaluate("ECHO.peers.length") + new.evaluate("ECHO.peers.length")
    print(f"stale tab  met {met} peers in 15s (design 9.4 wants 0)")
    b.close()
    if met:
        raise SystemExit("a different build found this game")
    print("OK")
