"""The two ways a room used to stop working, and the ghost it left behind.

Both cost a real evening of play. A host that had two seats accepted exactly
one joiner for its whole life, because seats were handed out by a counter that
only went up; and a host that left its own lobby stayed in the room at the
transport level for ever, so joiners connected, said `Hello`, and waited on a
game that was not there while their screen said "looking for the host".
"""
import sys, time, random
from playwright.sync_api import sync_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8123/index.html"
W, H = 1400, 900
scale = min(W / 1600.0, H / 980.0)
ox, oy = (W - 1600.0 * scale) / 2.0, (H - 980.0 * scale) / 2.0
css = lambda lx, ly: (ox + lx * scale, oy + ly * scale)
errors = []

HOOK = """
window.__wire = [];
const orig = RTCDataChannel.prototype.send;
RTCDataChannel.prototype.send = function (d) {
  if (this.label && this.label.indexOf('floodline') === 0)
    window.__wire.push({dir:'out', n:new Uint8Array(d).length});
  return orig.apply(this, arguments);
};
const oc = RTCPeerConnection.prototype.createDataChannel;
RTCPeerConnection.prototype.createDataChannel = function () {
  const ch = oc.apply(this, arguments);
  if (ch.label && ch.label.indexOf('floodline') === 0)
    ch.addEventListener('message', e =>
      window.__wire.push({dir:'in', n:new Uint8Array(e.data).length}));
  return ch;
};
"""


def check(ok, what):
    print(f"  {'ok  ' if ok else 'FAIL'}  {what}")
    if not ok:
        errors.append(what)


with sync_playwright() as p:
    br = p.chromium.launch()
    ctx = br.new_context(viewport={"width": W, "height": H})
    ctx.add_init_script(HOOK)

    def page(tag):
        pg = ctx.new_page()
        pg.on("pageerror", lambda e: errors.append(f"[{tag}] pageerror: {e}"))
        pg.on("console", lambda m: errors.append(f"[{tag}] console.error: {m.text}")
              if m.type == "error" else None)
        pg.goto(URL)
        pg.wait_for_function("document.getElementById('glcanvas').width > 0", timeout=30000)
        pg.wait_for_timeout(1200)
        return pg

    def room_field(pg, room):
        pg.mouse.click(*css(630.0, 353.0))
        pg.mouse.click(*css(820.0, 512.0))
        pg.keyboard.type(room)

    def welcomed(pg):
        """A `Welcome` carries the whole world, so it is the one big frame."""
        return any(f["dir"] == "in" and f["n"] > 10000 for f in pg.evaluate("window.__wire"))

    # ---- a seat given back ------------------------------------------------
    room = "rejoin-" + str(random.randint(100000, 999999))
    host = page("host")
    room_field(host, room)
    host.mouse.click(*css(630.0, 574.0))          # Host a game
    time.sleep(3)

    a = page("A"); room_field(a, room); a.mouse.click(*css(970.0, 574.0))
    time.sleep(8)
    check(welcomed(a), "the first joiner is welcomed")
    a.close()
    time.sleep(4)

    b = page("B"); room_field(b, room); b.mouse.click(*css(970.0, 574.0))
    time.sleep(10)
    check(welcomed(b), "a second joiner takes the seat the first one left")
    b.screenshot(path="/tmp/floodline-rejoin.png")

    # ---- and no ghost -----------------------------------------------------
    ghost_room = "ghost-" + str(random.randint(100000, 999999))
    g = page("ghost")
    room_field(g, ghost_room)
    g.mouse.click(*css(630.0, 574.0))             # Host a game
    time.sleep(3)
    g.mouse.click(*css(800.0, 594.0))             # back
    time.sleep(2)
    check(
        g.evaluate("window.FLOODLINE_RTC.debug() === null"),
        "leaving the lobby leaves the room",
    )

    c = page("C"); room_field(c, ghost_room); c.mouse.click(*css(970.0, 574.0))
    time.sleep(8)
    frames = c.evaluate("window.__wire")
    check(not frames, f"nobody is squatting the abandoned room (saw {len(frames)} frames)")

    # ---- and hosting again does not close the room it just opened ---------
    again = "again-" + str(random.randint(100000, 999999))
    room_field(g, again)
    g.mouse.click(*css(630.0, 574.0))
    time.sleep(3)
    check(
        g.evaluate("(() => {const s = FLOODLINE_RTC.debug(); return s && s.room})()") == again,
        "hosting a second game leaves that game open",
    )
    d = page("D"); room_field(d, again); d.mouse.click(*css(970.0, 574.0))
    time.sleep(10)
    check(welcomed(d), "and somebody can join it")

    br.close()

for e in errors:
    print("ERROR", e)
if errors:
    raise SystemExit(1)
print("OK")
