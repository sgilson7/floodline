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
#[allow(dead_code)]
pub const PANEL_W: f32 = 366.0;

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
