"""An agent's hands and eyes, one command at a time.

    driver.py <port> <verb> [args...]

An agent's turns are separate processes, so it cannot hold a `sync_playwright()`
session open across them. `table.py` launches the browsers once with their own
`--remote-debugging-port`; every action here is a short `connect_over_cdp`, act,
disconnect. The state lives in the browser, which is where it already lives.

One port is one agent. Nothing here can reach the other one.

**Map coordinates assume the default camera** — the whole map, framed, which is
`MapView::default` and what `view.py` mirrors. Zooming or panning invalidates
every `*-cell` verb until `frame` puts it back. That is the one sharp edge in
here, and `camera.py` is the check that says the arithmetic is right when the
camera is where this expects it.

    shot [path]                  the whole window
    panel [path]                 the side panel, with the line under the map
                                 stitched beneath it - that line is where a
                                 refusal is written, and cropping it away is
                                 how both players in M10.5 came to believe
                                 their clicks were being ignored
    rows [path]                  the status line and the three rows at the foot
    frame                        press 0: the whole map again, camera reset
    key <Name>                   Digit1..Digit9, KeyR, KeyP, KeyM, Escape, ...
    click <lx> <ly>              a point on the logical canvas
    button <name>                a named panel button - see `panel.py`
    click-cell <x> <y>           a map cell
    right-click-cell <x> <y>     the order gesture
    hover-cell <x> <y>           park the cursor, so the panel's hover row fills
    drag-cells <x0> <y0> <x1> <y1>   the wall gesture, and the only drag tool
    box-select <x0> <y0> <x1> <y1>   choose the people inside a rectangle
    wait <seconds>               let the world run
"""
import io
import sys
import time
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View, LOGICAL_W, LOGICAL_H, PANEL_W
import json
import panel as P
import table

def seated():
    """How many cities are in this game, so the panel's rows land right.

    **The panel moves with the city count** — the CITIES block sits above the
    tabs and everything under them — so a button addressed by name at the wrong
    count lands in a gap and does nothing at all, silently. That happened to all
    three players in the M12 three-player run: `choose all`, `back to hauling`
    and `tab-people` each reported success and changed nothing, for four game
    days, while the amber line was telling them to press exactly those buttons.

    `table.py` writes what it seated. Two if it has not, which is what every
    game before M12 was.
    """
    try:
        with open(table.TABLE) as f:
            seated_now = json.load(f)
    except (OSError, ValueError):
        return 2
    # Only if this is *our* table. The file outlives the session that wrote it,
    # so a two-player game run after a three-player one would otherwise inherit
    # the wrong count and put every click twenty-four pixels out - which is the
    # same fault this function exists to prevent, arriving from the other side.
    try:
        if int(sys.argv[1]) not in seated_now.get("ports", []):
            return 2
        return int(seated_now.get("cities", 2))
    except (ValueError, IndexError, TypeError):
        return 2


CITIES = seated()

BUTTONS = {
    "road": lambda c=CITIES: P.road_button(c),
    "point": lambda c=CITIES: P.point_button(c),
    "choose-all": lambda c=CITIES: P.choose_all(c),
    "back-to-hauling": lambda c=CITIES: P.back_to_hauling(c),
    "trade": lambda c=CITIES: P.propose_a_trade(c),
    "tab-build": lambda c=CITIES: P.tab("build", c),
    "tab-households": lambda c=CITIES: P.tab("households", c),
    "tab-people": lambda c=CITIES: P.tab("people", c),
    **{f"person-{n}": (lambda c=CITIES, k=n: P.person_chip(k, c)) for n in range(12)},
    **{name: (lambda c=CITIES, n=name: P.build_button(n, c)) for name in P.BUILDS},
}


def attach(p, port):
    """The one page in the one browser this agent is allowed to touch."""
    b = p.chromium.connect_over_cdp(f"http://localhost:{port}")
    ctx = b.contexts[0]
    page = ctx.pages[0]
    w, h = page.evaluate("[window.innerWidth, window.innerHeight]")
    dpr = page.evaluate("window.devicePixelRatio")
    return page, View(w, h, dpr)


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        raise SystemExit(2)
    port, verb, rest = sys.argv[1], sys.argv[2], sys.argv[3:]
    n = [float(a) for a in rest if a.replace("-", "").replace(".", "").isdigit()]

    with sync_playwright() as p:
        page, V = attach(p, port)
        out = rest[0] if rest and not rest[0].replace("-", "").isdigit() else None

        def save(box=None, name=None):
            path = out or name or f"/tmp/floodline-agent-{port}.png"
            img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
            if box:
                x0, y0 = V.css(box[0], box[1])
                x1, y1 = V.css(box[2], box[3])
                d = V.dpr
                img = img.crop((int(x0 * d), int(y0 * d), int(x1 * d), int(y1 * d)))
            img.save(path)
            print(path)

        if verb == "shot":
            save()
        elif verb == "panel":
            # The panel *and* the line under the map, stitched together.
            #
            # They are far apart on screen and the second one is easy to crop
            # away, which is exactly what this verb used to do. A refusal — why
            # a building would not go there, how many of the eight were taken —
            # is written under the *map*, at `LOGICAL_H - 52` in `input.rs`, so
            # an agent told to prefer `panel` because it is cheaper could not
            # see a single one. Both players in the M10.5 rehearsal reported
            # placements that "silently did nothing"; neither was silent, and
            # neither could have known that.
            path = out or f"/tmp/floodline-panel-{port}.png"
            img = Image.open(io.BytesIO(page.screenshot())).convert("RGB")
            d = V.dpr

            def box(x0, y0, x1, y1):
                a, b = V.css(x0, y0)
                c, e = V.css(x1, y1)
                return img.crop((int(a * d), int(b * d), int(c * d), int(e * d)))

            side = box(LOGICAL_W - PANEL_W, 0.0, LOGICAL_W, LOGICAL_H)
            # **All three message slots, not one.**
            #
            # This was `LOGICAL_H - 60` and it is the same fault the docstring
            # above warns about, made again. M12 gave the game three lines
            # instead of one - a refusal at 52 above the foot, the dead at 96,
            # and news at 140 - and this crop kept only the refusal. So in the
            # M12.11 run **neither player ever saw a single death**: city 0
            # lost eight people and city 1 five, both watched for the line on
            # every panel they took, and it was being cut off the bottom of
            # their own eyes. One of them called it the worst thing in the run.
            #
            # A slot added above this line has to move this line.
            notice = box(0.0, LOGICAL_H - 150.0, LOGICAL_W - PANEL_W, LOGICAL_H - 8.0)
            both = Image.new("RGB", (max(side.width, notice.width),
                                     side.height + notice.height))
            both.paste(side, (0, 0))
            both.paste(notice, (0, side.height))
            both.save(path)
            print(path)
        elif verb == "rows":
            # The status line and the three at the foot: what is being waited
            # on, and whether the peers are keeping step.
            save((LOGICAL_W - PANEL_W, P.status() - 20.0, LOGICAL_W, LOGICAL_H - 16.0),
                 f"/tmp/floodline-rows-{port}.png")
        elif verb == "frame":
            page.keyboard.press("Digit0")
            print("the whole map, framed")
        elif verb == "key":
            page.keyboard.press(rest[0])
            print(f"pressed {rest[0]}")
        elif verb == "click":
            page.mouse.click(*V.css(n[0], n[1]))
            print(f"clicked ({n[0]:.0f}, {n[1]:.0f}) on the canvas")
        elif verb == "button":
            name = rest[0]
            if name not in BUTTONS:
                print(f"no such button: {name}. one of: {', '.join(sorted(BUTTONS))}")
                raise SystemExit(2)
            # **Switch to the tab the button is on first.**
            #
            # Every build button, `choose all`, `back to hauling` and the trade
            # button are drawn by `input.rs::tools`, which only runs while the
            # build tab is showing. Pressed with `households` or `people` open,
            # the click lands on empty panel and *nothing happens at all* -
            # both M12.11 players lost a cycle of orders to this and one of
            # them only found out because the game correctly answered a later
            # right-click with "nobody chosen".
            #
            # The game is not at fault: a human sees that the button is not
            # there. An agent addresses it by name and cannot.
            if name not in ("tab-build", "tab-households", "tab-people"):
                tx, ty = P.tab("build", CITIES)
                page.mouse.click(*V.css(tx, ty))
                page.wait_for_timeout(120)
            lx, ly = BUTTONS[name]()
            page.mouse.click(*V.css(lx, ly))
            print(f"pressed the {name} button")
        elif verb == "click-cell":
            page.mouse.click(*V.cell(int(n[0]), int(n[1])))
            print(f"clicked cell ({int(n[0])}, {int(n[1])})")
        elif verb == "right-click-cell":
            page.mouse.click(*V.cell(int(n[0]), int(n[1])), button="right")
            print(f"right-clicked cell ({int(n[0])}, {int(n[1])})")
        elif verb == "hover-cell":
            page.mouse.move(*V.cell(int(n[0]), int(n[1])))
            print(f"cursor over cell ({int(n[0])}, {int(n[1])})")
        elif verb in ("drag-cells", "box-select"):
            page.mouse.move(*V.cell(int(n[0]), int(n[1])))
            page.mouse.down()
            page.mouse.move(*V.cell(int(n[2]), int(n[3])), steps=8)
            page.mouse.up()
            what = "drew" if verb == "drag-cells" else "chose inside"
            print(f"{what} ({int(n[0])},{int(n[1])}) to ({int(n[2])},{int(n[3])})")
        elif verb == "wait":
            time.sleep(min(n[0], 120.0))
            print(f"waited {n[0]:.0f}s")
        else:
            print(f"no such verb: {verb}")
            raise SystemExit(2)


if __name__ == "__main__":
    main()
