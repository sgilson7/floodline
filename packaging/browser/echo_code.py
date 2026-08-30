"""Two tabs, the pasted-code path: does an offer and an answer connect?"""
import sys, time
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/echo.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

def wire(page, tag):
    page.on("pageerror", lambda e: print(f"[{tag}] PAGEERROR {e}"))
    page.on("console", lambda m: print(f"[{tag}] console.{m.type}: {m.text}")
            if m.type in ("error", "warning") else None)

def wait(page, expr, what, timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        v = page.evaluate(expr)
        if v:
            return v
        time.sleep(0.25)
    raise SystemExit(f"timed out waiting for {what}: {page.evaluate('window.ECHO && window.ECHO.errors')}")

with sync_playwright() as p:
    b = p.chromium.launch()
    ctx = b.new_context(device_scale_factor=DPR)
    host = ctx.new_page(); wire(host, "host")
    join = ctx.new_page(); wire(join, "join")
    host.goto(URL); join.goto(URL)
    host.wait_for_selector("#host-code"); join.wait_for_selector("#join-code")

    t0 = time.time()
    host.click("#host-code")
    offer = wait(host, "window.ECHO.mine", "the host's invitation")
    print(f"offer      {len(offer)} chars, gathered in {time.time()-t0:.1f}s")

    join.click("#join-code")
    join.fill("#theirs", offer)
    join.click("#apply")
    answer = wait(join, "window.ECHO.mine", "the joiner's reply")
    print(f"answer     {len(answer)} chars")

    host.fill("#theirs", answer)
    host.click("#apply")

    wait(host, "window.ECHO.peers.length > 0", "the host to see a peer")
    wait(join, "window.ECHO.peers.length > 0", "the joiner to see a peer")
    print(f"connected  {time.time()-t0:.1f}s after the button")

    wait(host, "window.ECHO.got.reliable > 1 && window.ECHO.got.unreliable > 1",
         "bytes on both channels at the host")
    wait(join, "window.ECHO.got.reliable > 1 && window.ECHO.got.unreliable > 1",
         "bytes on both channels at the joiner")
    print("host  ", host.evaluate("({rtt: ECHO.rtt, got: ECHO.got, bytes: ECHO.bytes})"))
    print("joiner", join.evaluate("({rtt: ECHO.rtt, got: ECHO.got, bytes: ECHO.bytes})"))

    # The host should already be offering a code to the next joiner.
    nxt = wait(host, "window.ECHO.mine", "a fresh invitation for the next joiner")
    print("next invitation differs:", nxt != offer)

    # A closed tab must show up as Left on the other one.
    join.close()
    end = time.time() + 20
    while time.time() < end and host.evaluate("window.ECHO.peers.length") > 0:
        time.sleep(0.25)
    left = host.evaluate("window.ECHO.peers.length") == 0
    print(f"left seen  {left} after {time.time()-(end-20):.1f}s")

    errs = host.evaluate("window.ECHO.errors")
    print("host errors:", errs)
    b.close()
    if not left:
        raise SystemExit("a closed tab never produced Left")
    print("OK")
