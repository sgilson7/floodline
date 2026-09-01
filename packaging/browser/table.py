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
import json
import os
import random
import time

# Where `sit_down` writes what it seated, for `driver.py` to read.
#
# **The panel's rows move with the number of cities**, so an agent's hands need
# to know how many are playing and have no other way to find out. Without this
# every named button in `driver.py` missed by twenty-four logical pixels in the
# M12 three-player run - `choose all`, `back to hauling` and `tab-people` all
# reported success and did nothing, for four game days, while the amber line
# said "choose all, then back to hauling" and pressing it did nothing at all.
# One of the three players worked it out and recovered by computing the
# coordinate by hand; that is not a thing a player should have to do.
TABLE = "/tmp/floodline-table.json"

# Chromium's throttling, off. Playwright passes all three itself today; they
# are named here anyway, because the reason we need them is ours.
FLAGS = [
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-renderer-backgrounding",
]

# The agents' debugging ports, in the order they are seated: host first.
#
# Three of them, because two is the only size this game has ever been played
# at and the lobby's `+` goes to six. `sit_down` takes as many as it is given.
PORTS = (9222, 9223, 9224)

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
# `start_screen`'s seat stepper. `lobby.rs` draws it at `Rect::new(cx + 28, y,
# 44, 38)` one row above the room field, which is the row this file already
# knows the y of. One click is one more city on the map.
SEATS_PLUS = (CX + 50.0, 451.0)
CARD = (CX, 400.0)               # anywhere on the modal first-run card


def sit_down(p, url, dpr=1.0, room=None, ports=None, on_error=None):
    """Launch a browser per port and take them all into one game.

    Returns `(browsers, pages, room)`. The pages are past the lobby and past
    the modal first-run card, which covers the map on all of them.

    **As many as it is given.** This seated exactly two until M12: two browsers,
    two tags, seats left at the lobby's default. The lobby's `+` goes to six and
    the lockstep plays six - `six_players_stay_in_step` - so the harness was the
    only thing insisting on two, and "three people cannot be got into a game"
    was therefore untestable rather than untrue.
    """
    room = room or "fl-m10-" + str(random.randint(100000, 999999))
    ports = tuple(ports) if ports is not None else (None, None)
    n = len(ports)
    assert 2 <= n <= 6, f"a game seats two to six, not {n}"

    browsers, pages = [], []
    tags = ["host"] + [f"join{i}" for i in range(1, n)]
    for port, tag in zip(ports, tags):
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
    # Seats. `Lobby::new` starts at two - a seat nobody takes is a city
    # standing there with nobody commanding it - so the host presses `+` once
    # per extra player before it hosts. The map is generated for this number
    # and cannot be changed after, which is why it is the one thing that has to
    # be right before anybody clicks Host.
    for _ in range(n - 2):
        click(pages[0], *SEATS_PLUS)
        time.sleep(0.2)
    click(pages[0], *HOST_BTN)
    time.sleep(0.5)
    for pg in pages[1:]:
        click(pg, *JOIN_BTN)
        time.sleep(0.4)

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

    # What was seated, so `driver.py` can put its clicks in the right place.
    try:
        with open(TABLE, "w") as f:
            json.dump({"room": room, "cities": n, "ports": list(ports)}, f)
    except OSError:
        pass

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
    # How many sit down. Two unless asked otherwise; the lobby goes to six.
    seats = int(sys.argv[3]) if len(sys.argv) > 3 else 2
    ports = PORTS[:seats] if seats <= len(PORTS) else tuple(9222 + i for i in range(seats))

    with sync_playwright() as p:
        browsers, pages, room = sit_down(p, url, dpr, ports=ports,
                                         on_error=lambda m: print("ERROR", m))
        if room is None:
            print("the two browsers never found each other")
            raise SystemExit(1)
        print(f"room  {room}")
        print(f"url   {url}")
        for i, port in enumerate(ports):
            who = f"city {i} (host)" if i == 0 else f"city {i} (joiner)"
            print(f"{who:16} http://localhost:{port}")
        print("\nthe table is set. ctrl-c to clear it.")
        try:
            while True:
                time.sleep(3600)
        except KeyboardInterrupt:
            pass
