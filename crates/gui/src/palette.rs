//! Colours, in one place.
//!
//! Everything is drawn with `draw_rectangle`, `draw_circle` and `draw_line` —
//! design §1 is explicit that there are no sprites and that this is the point,
//! because it puts all the effort into the simulation. What a flat-colour game
//! does need is a palette that reads at a glance: which ground is dry, how deep
//! the water is, and whose city that is.

use macroquad::prelude::*;

pub const BACKDROP: Color = Color::new(0.031, 0.031, 0.047, 1.0);
pub const PANEL: Color = Color::new(0.055, 0.055, 0.082, 1.0);
pub const RULE: Color = Color::new(0.16, 0.16, 0.22, 1.0);
pub const INK: Color = Color::new(0.85, 0.86, 0.91, 1.0);
pub const FAINT: Color = Color::new(0.42, 0.42, 0.52, 1.0);
pub const WARNING: Color = Color::new(0.94, 0.72, 0.28, 1.0);
pub const ALARM: Color = Color::new(0.90, 0.33, 0.28, 1.0);

/// Ground, shaded by height so the lie of the land reads without contours.
///
/// The ramp runs from a drab olive in the lowlands to a pale grey on the
/// tops. Where the flood will go is the part a player has to be able to see
/// before it goes there.
pub fn ground(kind: sim::Ground, height: u8, relief: i32) -> Color {
    let t = (height as f32 / relief.max(1) as f32).clamp(0.0, 1.0);
    match kind {
        sim::Ground::Shallows => Color::new(0.16, 0.28, 0.36, 1.0),
        sim::Ground::Sand => Color::new(0.52, 0.47, 0.35, 1.0),
        sim::Ground::Rock => {
            let g = 0.34 + 0.22 * t;
            Color::new(g, g * 0.97, g * 1.02, 1.0)
        }
        sim::Ground::Grass => Color::new(
            0.20 + 0.30 * t,
            0.29 + 0.28 * t,
            0.17 + 0.30 * t,
            1.0,
        ),
    }
}

/// Water: blue, with its opacity carrying the depth (design §1).
pub fn water(depth: u16) -> Color {
    // Six units is over your head, so that is where it stops getting darker —
    // past that the difference does not change what happens to you.
    let t = (depth as f32 / (sim::balance::SWIM_DEPTH as f32)).clamp(0.0, 1.0);
    Color::new(0.13, 0.34, 0.62, 0.25 + 0.6 * t)
}

/// One per city, and they have to be told apart at a glance across a whole map.
pub fn player(p: sim::PlayerId) -> Color {
    const CITIES: [Color; 6] = [
        Color::new(0.36, 0.66, 0.95, 1.0), // blue
        Color::new(0.95, 0.55, 0.30, 1.0), // orange
        Color::new(0.45, 0.82, 0.48, 1.0), // green
        Color::new(0.85, 0.45, 0.80, 1.0), // magenta
        Color::new(0.92, 0.85, 0.36, 1.0), // yellow
        Color::new(0.55, 0.85, 0.85, 1.0), // cyan
    ];
    CITIES[p.0 as usize % CITIES.len()]
}

/// Buildings are rectangles with a glyph (design §1).
pub fn glyph(kind: sim::Kind) -> &'static str {
    use sim::Kind;
    match kind {
        Kind::Hearth => "H",
        Kind::Cottage => "c",
        Kind::Farm => "F",
        Kind::Forester => "T",
        Kind::Quarry => "Q",
        Kind::Granary => "G",
        Kind::Stockpile => "s",
        Kind::Dike => "=",
        Kind::Road => "",
        Kind::Bridge => "",
    }
}

/// Lobby chrome. A button is a shade lighter than the panel it sits on and a
/// field a shade darker, which is the whole of the depth in this interface.
pub const BUTTON: Color = Color::new(0.11, 0.11, 0.15, 1.0);
pub const FIELD: Color = Color::new(0.04, 0.04, 0.06, 1.0);

/// A legal placement, under the cursor.
pub const GOOD: Color = Color::new(0.42, 0.78, 0.45, 1.0);
