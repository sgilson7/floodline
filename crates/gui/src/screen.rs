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
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
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
        cam.viewport = Some((
            self.x as i32,
            self.y as i32,
            (LOGICAL_W * self.scale) as i32,
            (LOGICAL_H * self.scale) as i32,
        ));
        cam
    }

    /// Real mouse pixels -> logical coordinates. Every input path will use
    /// this, so hit-testing lines up with drawing at any window size.
    #[allow(dead_code)]
    pub fn mouse(&self) -> (f32, f32) {
        let (mx, my) = mouse_position();
        ((mx - self.x) / self.scale, (my - self.y) / self.scale)
    }
}
