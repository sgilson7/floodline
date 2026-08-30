"""Where a map cell lands on the page.

One copy, imported by every script that clicks the map, for the reason the game
itself keeps its two conversions in `screen.rs`: the coordinate arithmetic in
this project has been wrong twice and both times it was because two places did
the same sum and disagreed. Mirrors `crates/gui/src/screen.rs` — if the
letterbox or the camera changes, this is the file that changes with it.
"""

# crates/gui/src/screen.rs
LOGICAL_W, LOGICAL_H = 1600.0, 980.0
PANEL_W = 366.0
CELL = 8.0
MAP_W = MAP_H = 128            # sim::MAP_W

# map_window()
WIN_X, WIN_Y = 12.0, 12.0
WIN_W, WIN_H = LOGICAL_W - PANEL_W - 24.0, LOGICAL_H - 24.0


class View:
    """The letterbox and the camera, together, for a given browser window."""

    def __init__(self, width, height, dpr=1.0):
        self.w, self.h, self.dpr = width, height, dpr
        self.scale = min(width / LOGICAL_W, height / LOGICAL_H)
        self.ox = (width - LOGICAL_W * self.scale) / 2.0
        self.oy = (height - LOGICAL_H * self.scale) / 2.0
        # MapView::default(): the whole map, as large as it will go.
        span = max(MAP_W, MAP_H) * CELL
        self.zoom = min(WIN_W / span, WIN_H / span)
        self.centre = (MAP_W * CELL / 2.0, MAP_H * CELL / 2.0)

    def css(self, lx, ly):
        """Logical canvas -> CSS pixels, which is what Playwright clicks in."""
        return self.ox + lx * self.scale, self.oy + ly * self.scale

    def logical_of_map(self, mx, my):
        """Map space -> the logical canvas, through the camera."""
        midx, midy = WIN_X + WIN_W / 2.0, WIN_Y + WIN_H / 2.0
        return (
            midx + (mx - self.centre[0]) * self.zoom,
            midy + (my - self.centre[1]) * self.zoom,
        )

    def cell(self, cx, cy):
        """The middle of a map cell, in CSS pixels."""
        lx, ly = self.logical_of_map((cx + 0.5) * CELL, (cy + 0.5) * CELL)
        return self.css(lx, ly)

    def map_cell(self, lx, ly):
        """The logical canvas -> a map cell, the inverse of `cell`."""
        midx, midy = WIN_X + WIN_W / 2.0, WIN_Y + WIN_H / 2.0
        mx = self.centre[0] + (lx - midx) / self.zoom
        my = self.centre[1] + (ly - midy) / self.zoom
        return int(mx // CELL), int(my // CELL)

    def map_corner(self):
        """Where cell (0,0)'s corner lands on the logical canvas."""
        return self.logical_of_map(0.0, 0.0)

    def px(self, img, lx, ly):
        """The pixel at a logical point, in a screenshot."""
        x, y = self.css(lx, ly)
        return img.getpixel((int(x * self.dpr), int(y * self.dpr)))
