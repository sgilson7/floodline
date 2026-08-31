"""Sit two agents down at one game, and keep the browsers alive.

The lobby dance lives here and nowhere else. `two_agents.py` imports it to
check it; a run imports it to start, and then leaves the browsers standing so
the agents can reach them:

    table.py <url> [dpr]        launch, seat everybody, print the ports, wait

Two browsers rather than two contexts, with Chromium's throttling off — see
`DECISIONS.md`, "Two browsers, because thirty seconds of not rendering is a
drop". Each browser is given its own `--remote-debugging-port` so that
`driver.py` can attach to one of them without Playwright holding the session
open: an agent's turns are separate processes and cannot keep one.

Each agent is told one port and never the other. That is what makes "neither
may read the other's page" a property of the setup rather than a rule somebody
has to keep.
"""
import random
import time

# Chromium's throttling, off. Playwright passes all three itself today; they
# are named here anyway, because the reason we need them is ours.
FLAGS = [
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
]

# The two agents' debugging ports, in the order they are seated: host first.
PORTS = (9222, 9223)

W, H = 1400, 900

# Lobby geometry, from crates/gui/src/lobby.rs. Literals rather than read from
# anywhere: if the lobby moves, the check that imports this should notice
# rather than click a gap and blame something else.
CX = 800.0
BY_ROOM = (CX - 170.0, 353.0)    # start_screen: the "by room code" button
ROOM_FLD = (CX + 20.0, 512.0)    # its room field, which only relay mode has
HOST_BTN = (CX - 170.0, 574.0)
JOIN_BTN = (CX + 170.0, 574.0)
START_BTN = (CX, 534.0)          # hosting_screen's running total, relay mode
CARD = (CX, 400.0)               # anywhere on the modal first-run card


def sit_down(p, url, dpr=1.0, room=None, ports=None, on_error=None):
    """Launch two browsers and take them both into one game.

    Returns `(browsers, pages, room)`. The pages are past the lobby and past
    the modal first-run card, which covers the map on both.
    """
    room = room or "fl-m10-" + str(random.randint(100000, 999999))
    ports = ports if ports is not None else (None, None)

    browsers, pages = [], []
    for port, tag in zip(ports, ("host", "join")):
        args = list(FLAGS)
        if port:
            args.append(f"--remote-debugging-port={port}")
        b = p.chromium.launch(args=args)
        page = b.new_context(viewport={"width": W, "height": H},
                             device_scale_factor=dpr).new_page()
        if on_error:
            page.on("pageerror", lambda e, t=tag: on_error(f"[{t}] pageerror: {e}"))
            page.on("console", lambda m, t=tag: on_error(f"[{t}] console.error: {m.text}")
                    if m.type == "error" else None)
        browsers.append(b)
        pages.append(page)

    def click(page, lx, ly):
        scale = min(W / 1600.0, H / 980.0)
        ox, oy = (W - 1600.0 * scale) / 2.0, (H - 980.0 * scale) / 2.0
        page.mouse.click(ox + lx * scale, oy + ly * scale)

    for pg in pages:
        pg.goto(url)
        pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
    time.sleep(1.5)

    # The room-code path only. The pasted-code path is for players behind
    # strict NATs and has nothing to teach a playtest.
    for pg in pages:
        click(pg, *BY_ROOM)
        click(pg, *ROOM_FLD)
        pg.keyboard.type(room)
    # Seats are left at two, which is what `Lobby::new` starts them at: a seat
    # nobody takes is a city standing there with nobody commanding it.
    click(pages[0], *HOST_BTN)
    time.sleep(0.5)
    click(pages[1], *JOIN_BTN)

    end = time.time() + 60
    linked = False
    while time.time() < end:
        if all(pg.evaluate("window.FLOODLINE_RTC.debug()?.links.size || 0") > 0
               for pg in pages):
            linked = True
            break
        time.sleep(0.4)
    if not linked:
        return browsers, pages, None

    click(pages[0], *START_BTN)
    time.sleep(2.0)
    for pg in pages:
        click(pg, *CARD)      # the first-run card is modal, on both
    time.sleep(1.5)
    return browsers, pages, room


if __name__ == "__main__":
    import sys
    from playwright.sync_api import sync_playwright

    url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
    dpr = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

    with sync_playwright() as p:
        browsers, pages, room = sit_down(p, url, dpr, ports=PORTS,
                                         on_error=lambda m: print("ERROR", m))
        if room is None:
            print("the two browsers never found each other")
            raise SystemExit(1)
        print(f"room  {room}")
        print(f"url   {url}")
        for who, port in zip(("city 0 (host)", "city 1 (joiner)"), PORTS):
            print(f"{who:16} http://localhost:{port}")
        print("\nthe table is set. ctrl-c to clear it.")
        try:
            while True:
                time.sleep(3600)
        except KeyboardInterrupt:
            pass
