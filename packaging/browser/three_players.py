"""Three cities in one game, which nobody had ever managed.

`two_agents.py` is the same check at two, and two is the only size this game
has ever been played at — while the lobby's `+` goes to six and
`six_players_stay_in_step` says the lockstep plays six. So "three people cannot
be got into a game" was untestable rather than untrue, and the harness was the
thing insisting on two.

What a third player adds that a second does not:

* **The room stops being a star.** Two joiners are connected to each other as
  well as to the host, so a message sent where the design says it may not be
  actually arrives. `Loopback` cannot show this - see
  `Conditions::mesh` - and a browser is where it is real.
* **The panel has a third city to draw**, and the cities list is above the
  variable stack that holds incoming trade offers. One city per row put an
  offer below `VARIABLE_FLOOR` at three seats with a building selected; two to
  a row is why it fits.
* **The host bundles two turns a tick instead of one**, and every joiner has to
  agree with a world it did not generate.

    three_players.py [url] [dpr]
"""
import io, sys, time
from PIL import Image
from playwright.sync_api import sync_playwright
from view import View
import panel as P
import table

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
DPR = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
V = View(table.W, table.H, DPR)
errors = []

# Three cities, so the panel's fixed part is one row taller than at two.
CITIES = 3
TICK_ROW = (P.LEFT, P.TICK - 16.0, P.LEFT + 200.0, P.TICK + 6.0)


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


def crop(img, box):
    x0, y0 = V.css(box[0], box[1])
    x1, y1 = V.css(box[2], box[3])
    return img.crop((int(x0 * DPR), int(y0 * DPR),
                     int(x1 * DPR), int(y1 * DPR))).tobytes()


def ticks(pages):
    out = []
    for pg in pages:
        img = Image.open(io.BytesIO(pg.screenshot())).convert("RGB")
        out.append(crop(img, TICK_ROW))
    return out


with sync_playwright() as p:
    browsers, pages, room = table.sit_down(
        p, URL, DPR, ports=(None, None, None),
        on_error=lambda m: errors.append(m))
    check(room is not None, "three browsers found each other in one room")
    if room is None:
        for b in browsers:
            b.close()
        raise SystemExit(1)

    check(len(pages) == 3, f"three pages were seated, not {len(pages)}")

    # Everybody is past the lobby and drawing a world.
    for i, pg in enumerate(pages):
        img = Image.open(io.BytesIO(pg.screenshot())).convert("RGB")
        img.save(f"/tmp/floodline-three-{i}.png")
        # The map window, not the panel: a page still in the lobby draws the
        # lobby across the whole canvas and has no map at all.
        band = crop(img, (40.0, 300.0, 900.0, 700.0))
        check(len(set(band)) > 4, f"city {i} is drawing a map")

    # And all three are ticking. Sampled twice, because a page that has stopped
    # and a page that is slow look identical in one frame.
    was = ticks(pages)
    time.sleep(6)
    now = ticks(pages)
    for i in range(3):
        check(now[i] != was[i], f"city {i}'s tick count is advancing")

    # The panel still has room for what a player has to be able to click. At
    # three cities the fixed part is a row taller than at two, and the trade
    # button is the bottom of the variable stack.
    trade = P.propose_a_trade(CITIES)
    check(
        trade[1] < P.FOOT,
        f"the trade button is at y {trade[1]:.0f} and nothing variable may be "
        f"drawn below {P.FOOT:.0f}",
    )

    for b in browsers:
        b.close()

for e in errors:
    print("ERROR", e)
if errors:
    raise SystemExit(1)
print("OK")
