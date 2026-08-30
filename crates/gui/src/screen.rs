//! The fixed logical canvas and where it lands on a real window.
//!
//! Everything is drawn at 1600 × 980 no matter what size the window is, and
//! the letterbox scales it. gear-master's approach, and the reason for it is
//! the same: layout arithmetic that has to cope with an arbitrary window size
//! is where the fiddly bugs live, and a fixed canvas has none of it.

use macroquad::prelude::*;

pub const LOGICAL_W: f32 = 1600.0;
pub const LOGICAL_H: f32 = 980.0;

/// The side panel, from design §7 by way of the plan's phase 5. Declared here
/// with the canvas it divides so the map's width is never a magic number.
pub const PANEL_W: f32 = 366.0;

/// A map cell is eight units of map space. The camera decides how many logical
/// pixels that is.
pub const CELL: f32 = 8.0;

/// The window on the logical canvas that the map is seen through.
///
/// Everything left of the panel, with a margin. The map itself is bigger than
/// this at any zoom past the fit, which is what `MapView`'s camera is for: the
/// window is a hole, not a canvas.
pub fn map_window() -> Rect {
    Rect::new(12.0, 12.0, LOGICAL_W - PANEL_W - 24.0, LOGICAL_H - 24.0)
}

/// Where the logical canvas lands on the real screen, letterboxed and centred.
///
/// **Two coordinate systems meet here, and they are not the same one.**
/// `screen_width()` and the mouse are in *logical* pixels — what CSS calls a
/// pixel — while `Camera2D::viewport` is in *framebuffer* pixels, which on a
/// retina display or any browser at a device pixel ratio above one are twice
/// as many. Computing the viewport from logical sizes and handing it to GL
/// unconverted put the whole game in the bottom-left quarter of the window
/// with the rest black: a quarter of the area because the rect was half-size
/// in each direction, and the bottom-left because that is where GL's viewport
/// origin is.
///
/// So `x`, `y` and `scale` are kept in logical pixels — that is what input
/// needs — and `dpi` is applied only where the framebuffer is being addressed.
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// Framebuffer pixels per logical pixel.
    pub dpi: f32,
}

impl Viewport {
    pub fn current() -> Self {
        // The only place that asks the real window how big it is.
        let (sw, sh) = (screen_width(), screen_height());
        let scale = (sw / LOGICAL_W).min(sh / LOGICAL_H);
        Viewport {
            x: (sw - LOGICAL_W * scale) / 2.0,
            y: (sh - LOGICAL_H * scale) / 2.0,
            scale,
            dpi: miniquad::window::dpi_scale(),
        }
    }

    pub fn camera(&self) -> Camera2D {
        // NOT `Camera2D::from_display_rect`: that sets a negative y-zoom,
        // which double-flips against macroquad's screen convention and
        // renders the whole frame upside down. Positive y-zoom keeps y
        // pointing down.
        let mut cam = Camera2D {
            target: vec2(LOGICAL_W / 2.0, LOGICAL_H / 2.0),
            zoom: vec2(2.0 / LOGICAL_W, 2.0 / LOGICAL_H),
            ..Default::default()
        };
        // Framebuffer pixels, not logical ones — see the note on `Viewport`.
        cam.viewport = Some((
            (self.x * self.dpi) as i32,
            (self.y * self.dpi) as i32,
            (LOGICAL_W * self.scale * self.dpi) as i32,
            (LOGICAL_H * self.scale * self.dpi) as i32,
        ));
        cam
    }

    /// Real mouse pixels -> logical coordinates. Every input path will use
    /// this, so hit-testing lines up with drawing at any window size.
    ///
    /// No `dpi` here, and that is deliberate rather than an omission:
    /// `mouse_position()` is already in logical pixels, the same ones `x`,
    /// `y` and `scale` are in. Applying the ratio again would put the cursor
    /// twice as far from the top-left as it really is — the mirror image of
    /// the bug that put the whole canvas in the corner.
    #[allow(dead_code)]
    pub fn mouse(&self) -> (f32, f32) {
        let (mx, my) = mouse_position();
        ((mx - self.x) / self.scale, (my - self.y) / self.scale)
    }
}

/// Where the map is being looked at from.
///
/// The second of this program's two coordinate conversions, and it obeys the
/// same rule as the first: **it is the only thing that converts between the
/// logical canvas and the map**, and everything else asks it. The letterbox
/// has been got wrong twice, both times because two places did the same
/// arithmetic and disagreed, so a second transform gets one home rather than
/// a multiplication scattered through the drawing code.
///
/// Map space is cells times `CELL`, with its origin at the map's top-left
/// corner — the same units the game was drawn in before there was a camera, so
/// nothing about a building's position had to change.
pub struct MapView {
    /// Logical pixels per unit of map space. One means eight pixels a cell.
    pub zoom: f32,
    /// The point in map space at the middle of the window.
    pub centre: Vec2,
}

impl Default for MapView {
    fn default() -> MapView {
        let mut v = MapView { zoom: 0.0, centre: Vec2::ZERO };
        v.frame_the_map();
        v
    }
}

impl MapView {
    /// The whole map, as large as it will go in the window.
    pub fn fit(&self) -> f32 {
        let win = map_window();
        let span = sim::MAP_W.max(sim::MAP_H) as f32 * CELL;
        (win.w / span).min(win.h / span)
    }

    pub fn frame_the_map(&mut self) {
        self.zoom = self.fit();
        self.centre = vec2(
            sim::MAP_W as f32 * CELL / 2.0,
            sim::MAP_H as f32 * CELL / 2.0,
        );
    }

    /// Closest in a village will ever need: six times life size, so a cottage
    /// is a hundred pixels across and a citizen is somebody rather than a dot.
    pub const CLOSEST: f32 = 6.0;

    /// Zoom about a point on the logical canvas, so the thing under the cursor
    /// stays under the cursor.
    pub fn zoom_about(&mut self, logical: Vec2, factor: f32) {
        let before = self.to_map(logical);
        self.zoom = (self.zoom * factor).clamp(self.fit(), Self::CLOSEST);
        let after = self.to_map(logical);
        self.centre += before - after;
        self.rein_in();
    }

    /// Move the eye, in logical pixels.
    pub fn pan(&mut self, by: Vec2) {
        self.centre += by / self.zoom;
        self.rein_in();
    }

    /// Keep the map on screen. Once it is smaller than the window it is
    /// centred and cannot be dragged at all, which is the only sensible answer
    /// to "where is the edge" when there is slack in both directions.
    fn rein_in(&mut self) {
        let win = map_window();
        let (w, h) = (sim::MAP_W as f32 * CELL, sim::MAP_H as f32 * CELL);
        let (half_w, half_h) = (win.w / self.zoom / 2.0, win.h / self.zoom / 2.0);
        self.centre.x = if half_w * 2.0 >= w { w / 2.0 } else { self.centre.x.clamp(half_w, w - half_w) };
        self.centre.y = if half_h * 2.0 >= h { h / 2.0 } else { self.centre.y.clamp(half_h, h - half_h) };
    }

    /// A point on the logical canvas, in map space.
    pub fn to_map(&self, logical: Vec2) -> Vec2 {
        let win = map_window();
        let mid = vec2(win.x + win.w / 2.0, win.y + win.h / 2.0);
        self.centre + (logical - mid) / self.zoom
    }

    /// The cell under a point on the logical canvas, if the point is in the
    /// window and the cell is on the map.
    pub fn cell_at(&self, logical: Vec2) -> Option<(i32, i32)> {
        if !map_window().contains(logical) {
            return None;
        }
        let m = self.to_map(logical);
        let (x, y) = ((m.x / CELL).floor() as i32, (m.y / CELL).floor() as i32);
        if x < 0 || y < 0 || x >= sim::MAP_W || y >= sim::MAP_H {
            return None;
        }
        Some((x, y))
    }

    /// The cells worth drawing, as `(x0, y0, x1, y1)` inclusive. Zoomed in
    /// this is most of the saving: the ground pass is sixteen thousand
    /// rectangles at the fit and a few hundred up close.
    pub fn visible(&self) -> (i32, i32, i32, i32) {
        let win = map_window();
        let a = self.to_map(vec2(win.x, win.y));
        let b = self.to_map(vec2(win.x + win.w, win.y + win.h));
        (
            ((a.x / CELL).floor() as i32 - 1).max(0),
            ((a.y / CELL).floor() as i32 - 1).max(0),
            ((b.x / CELL).ceil() as i32 + 1).min(sim::MAP_W - 1),
            ((b.y / CELL).ceil() as i32 + 1).min(sim::MAP_H - 1),
        )
    }

    /// A camera that draws map space into the window, and clips to it.
    ///
    /// The `y` here is *not* the one `Viewport::camera` uses, and that is not
    /// an inconsistency. GL's viewport origin is the bottom-left of the
    /// framebuffer, so a rectangle has to be given its distance from the
    /// bottom. The letterbox gets away with passing its top margin because it
    /// is centred — its top and bottom margins are equal, so the two numbers
    /// happen to be the same. The map window is not centred, so it does not.
    pub fn camera(&self, view: &Viewport) -> Camera2D {
        let win = map_window();
        let (fb_w, fb_h) = (screen_width() * view.dpi, screen_height() * view.dpi);
        let _ = fb_w;
        let x = (view.x + win.x * view.scale) * view.dpi;
        let top = (view.y + win.y * view.scale) * view.dpi;
        let w = win.w * view.scale * view.dpi;
        let h = win.h * view.scale * view.dpi;

        let mut cam = Camera2D {
            target: self.centre,
            // Map units across the window, turned into macroquad's
            // half-extent zoom. Positive y, like the outer camera: negative
            // double-flips against the screen convention and renders upside
            // down.
            zoom: vec2(2.0 * self.zoom / win.w, 2.0 * self.zoom / win.h),
            ..Default::default()
        };
        cam.viewport = Some((x as i32, (fb_h - top - h) as i32, w as i32, h as i32));
        cam
    }
}
