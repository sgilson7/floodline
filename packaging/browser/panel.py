"""Where the panel's rows are, in logical canvas coordinates.

`view.py` is the one copy of the letterbox and the camera; this is the one copy
of the panel, and it exists for the same reason. The panel has shifted five
times and every time it silently broke two checks that carried a y-coordinate
as a literal — the symptom is a click landing in a gap and something unrelated
being reported as the failure.

Not a check. Mirrors the running totals in `crates/gui/src/draw.rs::panel`,
`input.rs::tabs` and `input.rs::tools` — if a row is added to any of them, this
is the file that changes with it.

`play.py` and `assign.py` deliberately keep their own literals: their job is to
notice when the panel moves, and a shared module would rob them of it. Anything
written for M10 imports this instead, because an agent that mis-clicks for
thirty-six minutes has no way to tell.
"""

from view import LOGICAL_W, LOGICAL_H, PANEL_W

# input.rs uses all three of these verbatim.
LEFT = LOGICAL_W - PANEL_W + 18.0          # 1252
WIDE = PANEL_W - 36.0                      # 330
HALF = (WIDE - 8.0) / 2.0                  # 161
RIGHT = LEFT + WIDE

# ---- draw.rs::panel, the fixed part ------------------------------------
# `line()` there advances by size + 8, so every row below is that running
# total and nothing else.

TITLE = 44.0
AGE_DAY = 86.0          # "age 1 of 3   day 1 of 6"
OMEN = 112.0            # "all quiet" / "THE WATER IS HERE"
TREASURY = 148.0        # "food N   wood N   stone N   gold N"
CITIES = 188.0          # the "CITIES" label
FIRST_CITY = 211.0      # "city 0 (you): 8 souls"
CITY_STEP = 24.0

# ---- draw.rs, the bottom three ------------------------------------------
# Pinned to the foot of the window rather than to the running total, so these
# are the only rows that do not move when a city is added.

TICK = LOGICAL_H - 74.0         # "tick N"
PEERS = LOGICAL_H - 51.0        # "peers at [N, N]" - two numbers on the host
BUILD_SEED = LOGICAL_H - 28.0   # "build <hash>   seed N"


def after_cities(cities=2):
    """The y the city list ends at. Everything below it moves with a seat."""
    return FIRST_CITY + CITY_STEP * cities


def tutorial(cities=2):
    """The two reserved rows that say what to do next. Wrapped on words."""
    top = after_cities(cities) + 14.0
    return (top, top + 18.0)


def status(cities=2):
    """`playing`, `waiting on city N`, or `DESYNC with city N at tick T`."""
    return after_cities(cities) + 56.0


def fixed_ends(cities=2):
    """Where `draw::panel` stops and `input::panel_layer` takes over."""
    return status(cities) + 25.0


# ---- input.rs::tabs -----------------------------------------------------

def tab(which, cities=2):
    """The middle of the "build" or "households" tab button."""
    y = fixed_ends(cities) + 24.0
    x = LEFT + HALF / 2.0 + (0.0 if which == "build" else HALF + 8.0)
    return (x, y)


def body_top(cities=2):
    """Where the chosen tab's body starts, under the tab row."""
    return fixed_ends(cities) + 40.0


# ---- input.rs::tools ----------------------------------------------------
# BUILDABLE is nine kinds in two columns, so five rows of buttons.

BUILDS = ["cottage", "farm", "granary", "forester", "quarry",
          "stockpile", "dike", "post", "nursery"]


def build_button(name, cities=2):
    """The middle of a build button. `1 cottage` through `9 nursery`."""
    i = BUILDS.index(name)
    top = body_top(cities)
    x = LEFT + HALF / 2.0 + (i % 2) * (HALF + 8.0)
    return (x, top + 62.0 + 42.0 * (i // 2))


def road_button(cities=2):
    return (LEFT + HALF / 2.0, body_top(cities) + 280.0)


def point_button(cities=2):
    return (LEFT + HALF + 8.0 + HALF / 2.0, body_top(cities) + 280.0)


def tool_hint(cities=2):
    """"drag to choose. right-click to send them", and the rest."""
    return body_top(cities) + 310.0


def hover(cities=2):
    """What the cursor is over: `farm: 0 of 3 working`. Reserved when empty."""
    return body_top(cities) + 332.0


def chosen_count(cities=2):
    """"nobody chosen" / "3 chosen"."""
    return body_top(cities) + 376.0


def back_to_hauling(cities=2):
    return (LEFT + HALF / 2.0, body_top(cities) + 405.0)


def choose_all(cities=2):
    return (LEFT + HALF + 8.0 + HALF / 2.0, body_top(cities) + 405.0)


def propose_a_trade(cities=2):
    return (LEFT + WIDE / 2.0, body_top(cities) + 475.0)


def below_the_trade(cities=2):
    """Where the two *variable* stacks begin: offers, then level/move.

    Nothing fixed lives below this line, which is the property
    `panel_rows.py` checks and the reason the level/move row was moved here.
    """
    return body_top(cities) + 500.0


def row(y, size=15.0, x0=None, x1=None):
    """A text row's box, for cropping: `draw_text` takes y as the baseline."""
    return (LEFT if x0 is None else x0, y - size + 1.0,
            RIGHT if x1 is None else x1, y + 5.0)


def band(y0, y1):
    """A stretch of the panel, full width, for comparing two frames."""
    return (LEFT - 10.0, y0, RIGHT + 10.0, y1)
