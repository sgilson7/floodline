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
a whole run has no way to tell.
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
TREASURY = 148.0        # "food N  wood N  stone N  gold N" + "  meals N"
# Sixteen point since M12.C, not eighteen: five figures do not fit on 330
# pixels at the old size, and `draw.rs` hands the two pixels back so nothing
# below this row moves. The meals figure appears only when the city has any -
# content that varies inside a row that does not, which is the only safe way
# to make this panel conditional.
CITIES = 188.0          # the "CITIES" label
FIRST_CITY = 211.0      # "city 0 (you): 8   city 1: 5" - two to a row
CITY_STEP = 24.0        # per *row*, which is per two seats

# ---- draw.rs, the bottom three ------------------------------------------
# Pinned to the foot of the window rather than to the running total, so these
# are the only rows that do not move when a city is added.

# Two rows since M11.2, not three: the tick and the peers share a line, which
# bought the variable stack above them enough clearance to stop drawing over
# the foot. `FOOT` is the whole of it, and `input::VARIABLE_FLOOR` is the line
# nothing variable may cross.
TICK = LOGICAL_H - 51.0         # "tick N   peers at [N, N]"
PEERS = TICK                    # the same row now; kept so callers still read
BUILD_SEED = LOGICAL_H - 28.0   # "build <hash>   seed N"
FOOT = LOGICAL_H - 70.0         # nothing variable may be drawn below this


def after_cities(cities=2):
    """The y the city list ends at. Everything below it moves with a seat.

    **Two cities to a row since M12**, so this grows by a row per *pair* of
    seats rather than per seat. `draw.rs` has the arithmetic and the reason:
    one per row put the fixed part of the panel 24 px further down for every
    seat, and the variable stack underneath - the level/move row, road-joins
    and incoming trade offers - is what got squeezed out. At three cities with
    a building selected an offer had nine pixels and needed thirty-eight.
    """
    import math
    return FIRST_CITY + CITY_STEP * math.ceil(cities / 2)


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

# Three tabs since M12.D, so a third of the width each. The row's *height* is
# unchanged, which is the part that matters: a taller tab row would move the
# whole build tab down, and there are eighty-one pixels between the trade offer
# and `input::VARIABLE_FLOOR`.
THIRD = (WIDE - 16.0) / 3.0
TABS = ["build", "households", "people"]


def tab(which, cities=2):
    """The middle of the "build", "households" or "people" tab button."""
    y = fixed_ends(cities) + 24.0
    x = LEFT + THIRD / 2.0 + TABS.index(which) * (THIRD + 8.0)
    return (x, y)


def person_chip(n, cities=2):
    """The middle of the nth chip in the people tab, counting from nought.

    Twenty-eight high on a thirty-two pitch, and the tab stops drawing chips
    that would cross `VARIABLE_FLOOR` and says "and N more" instead. A city of
    eight fits with room to spare.
    """
    return (LEFT + WIDE / 2.0, body_top(cities) + 8.0 + 14.0 + 32.0 * n)


def body_top(cities=2):
    """Where the chosen tab's body starts, under the tab row."""
    return fixed_ends(cities) + 40.0


# ---- input.rs::tools ----------------------------------------------------
# BUILDABLE is ten kinds in two columns, and the road and the point carry on in
# the same grid: twelve buttons, forty pixels of pitch, no "BUILD" heading.
#
# M12.B added the builder's hut and **the panel did not move**. Eleven buttons
# needed six rows and so do twelve, because eleven left a gap at the end and
# the hut fills it. That is luck rather than design: the next building added
# here costs a row, and a row is forty pixels that have to come from somewhere
# above `input::VARIABLE_FLOOR`. See SECOND-ORDER-M12.md.
BUILDS = ["cottage", "farm", "granary", "forester", "quarry",
          "stockpile", "dike", "post", "nursery", "hut", "cookery",
          "road", "point"]

# `input.rs::TOOL_PITCH` and `TOOL_BUTTON_H`, verbatim. Thirty-six and
# thirty-three since M12.C: thirteen buttons need seven rows, and at the old
# forty-pixel pitch the seventh row left five pixels between the trade offer
# and `VARIABLE_FLOOR` once a building was selected. See there.
PITCH = 36.0
BUTTON_H = 33.0


def _tools_height():
    """`input.rs::tools`: eight pixels, a rule, sixteen, the grid, eight.

    The one number the rows below the grid hang off. Everything under the build
    tab is `body_top + _tools_height() + k` in `input.rs`, and was written here
    as `body_top + (272 + k)` while the grid's height never changed. M12.C
    changed it twice over - a seventh row, and a shorter pitch - so it is
    derived now.
    """
    import math
    buildable = len(BUILDS) - 2          # road and point are not in BUILDABLE
    rows = math.floor((buildable + 3) / 2.0)
    return 24.0 + PITCH * rows + 8.0


def build_button(name, cities=2):
    """The middle of a grid button, `1 cottage` through `p point`."""
    i = BUILDS.index(name)
    top = body_top(cities)
    x = LEFT + HALF / 2.0 + (i % 2) * (HALF + 8.0)
    return (x, top + 24.0 + BUTTON_H / 2.0 + PITCH * (i // 2))


def road_button(cities=2):
    return build_button("road", cities)


def point_button(cities=2):
    return build_button("point", cities)


def tool_hint(cities=2):
    """"drag to choose. right-click to send them", and the rest."""
    return body_top(cities) + _tools_height()


def hover(cities=2):
    """What the cursor is over: `farm: 0 of 3 working`. Reserved when empty."""
    return tool_hint(cities) + 22.0


def chosen_count(cities=2):
    """"nobody chosen" / "3 chosen"."""
    return tool_hint(cities) + 66.0


def back_to_hauling(cities=2):
    return (LEFT + HALF / 2.0, tool_hint(cities) + 95.0)


def choose_all(cities=2):
    return (LEFT + HALF + 8.0 + HALF / 2.0, tool_hint(cities) + 95.0)


def propose_a_trade(cities=2):
    return (LEFT + WIDE / 2.0, tool_hint(cities) + 165.0)


def below_the_trade(cities=2):
    """Where the two *variable* stacks begin: offers, then level/move.

    Nothing fixed lives below this line, which is the property
    `panel_rows.py` checks and the reason the level/move row was moved here.
    And nothing at all lives below `FOOT`, which is the other half of the same
    check: the stack stops there and says what it could not show.
    """
    return body_top(cities) + 462.0


def row(y, size=15.0, x0=None, x1=None):
    """A text row's box, for cropping: `draw_text` takes y as the baseline."""
    return (LEFT if x0 is None else x0, y - size + 1.0,
            RIGHT if x1 is None else x1, y + 5.0)


def band(y0, y1):
    """A stretch of the panel, full width, for comparing two frames."""
    return (LEFT - 10.0, y0, RIGHT + 10.0, y1)
