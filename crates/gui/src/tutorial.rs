//! Teaching the game by naming the next thing to do.
//!
//! Not a scripted sequence with a Next button. The panel has one line that
//! always says what this city most needs, and there is a card on the first run
//! that explains the three controls and the deadline. Both come from the state
//! of the world rather than from a step counter, so a player who does things
//! out of order is never told to do something they have already done, and one
//! who knows the game sees an empty line.
//!
//! It exists because of what playing it found: a village that starved on day
//! four with a farm standing in it and nothing on screen saying why. Every
//! sentence here is one somebody needed and did not have.

use crate::screen::LOGICAL_W;
use crate::ui;
use crate::palette;
use macroquad::prelude::*;
use sim::building::Kind;
use sim::{PlayerId, World};

/// The one thing this city most needs, and how to do it.
///
/// Ordered by what kills you soonest: a citizen empties at tick 1000 and dies
/// 3600 later, so a city with nowhere to eat has until day four, and the water
/// does not come until day six. Food first, always.
pub fn next_thing(w: &World, me: PlayerId) -> Option<&'static str> {
    if !w.players.contains(&me) || w.population(me) == 0 || w.finished().is_some() {
        return None;
    }
    let standing = |k: Kind| {
        w.buildings.iter().any(|b| b.owner == me && b.kind == k && b.standing_now())
    };
    let placed = |k: Kind| {
        w.buildings.iter().any(|b| b.owner == me && b.kind == k && b.state != sim::building::BuildState::Rubble)
    };
    let working = |k: Kind| {
        w.citizens
            .iter()
            .any(|c| c.owner == me && c.alive() && c.job == sim::citizen::Job::at(k))
    };

    // Food, in the order it can go wrong.
    if !placed(Kind::Granary) {
        return Some("press 3, click the ground: a granary. food is kept there, and eaten there");
    }
    if !placed(Kind::Farm) {
        return Some("press 2, click the ground: a farm. nothing else grows food");
    }
    if !standing(Kind::Granary) || !standing(Kind::Farm) {
        return Some("they are being built - your people fetch the wood themselves");
    }
    if !working(Kind::Farm) {
        return Some("drag to choose your people, then right-click the farm");
    }
    if w.treasury(me).food == 0 {
        return Some("the granary is empty - give the farm a moment");
    }

    // Then the two things that run out.
    let goods = w.treasury(me);
    if goods.wood < Kind::Cottage.cost().wood && !placed(Kind::Forester) {
        return Some("low on wood: press 4 for a forester's hut, the only source");
    }
    if standing(Kind::Forester) && !working(Kind::Forester) {
        return Some("nobody is cutting wood: right-click the forester's hut");
    }
    if goods.stone < Kind::Dike.cost().stone * 8 && !placed(Kind::Quarry) {
        return Some("low on stone: press 5 for a quarry. it needs rock beside it");
    }
    if standing(Kind::Quarry) && !working(Kind::Quarry) {
        return Some("nobody is at the quarry: right-click it");
    }

    // Then the flood.
    if !placed(Kind::Dike) {
        return Some("press 7 and drag a wall between your city and the water");
    }
    if w.omen() == sim::Omen::Uneasy {
        return Some("the water comes tomorrow: choose everybody, send them uphill");
    }
    None
}

/// The card a player sees once, on their first run.
///
/// Dismissed by any click or key, and it does not come back: a second reading
/// of an explanation nobody asked for is worse than none.
pub struct Welcome {
    shown: bool,
}

impl Default for Welcome {
    fn default() -> Welcome {
        Welcome { shown: true }
    }
}

impl Welcome {
    pub fn showing(&self) -> bool {
        self.shown
    }

    /// Draw it, and take the click or key that dismisses it.
    pub fn draw(&mut self, ui: &ui::Ui) {
        if !self.shown {
            return;
        }
        let card = Rect::new(LOGICAL_W / 2.0 - 380.0, 150.0, 760.0, 470.0);
        draw_rectangle(card.x, card.y, card.w, card.h, Color { a: 0.97, ..palette::PANEL });
        draw_rectangle_lines(card.x, card.y, card.w, card.h, 2.0, palette::RULE);

        let left = card.x + 44.0;
        let mut y = card.y + 62.0;
        let line = |text: &str, size: f32, colour: Color, gap: f32, y: &mut f32| {
            draw_text(text, left, *y, size, colour);
            *y += gap;
        };

        line("A CITY ON A RIVER", 30.0, palette::INK, 40.0, &mut y);
        line(
            "At the end of every age the water comes out of the low corner.",
            19.0,
            palette::FAINT,
            26.0,
            &mut y,
        );
        line(
            "It comes on day 6. Your people starve on day 4 if they cannot eat.",
            19.0,
            palette::WARNING,
            44.0,
            &mut y,
        );

        line("FIRST", 15.0, palette::FAINT, 26.0, &mut y);
        line("3 then click        a granary - the only place food is kept", 18.0, palette::INK, 26.0, &mut y);
        line("2 then click        a farm", 18.0, palette::INK, 26.0, &mut y);
        line("drag, right-click   choose your people, put them to work", 18.0, palette::INK, 40.0, &mut y);

        line("THEN", 15.0, palette::FAINT, 26.0, &mut y);
        line("4 and 5             a forester's hut and a quarry: wood and stone", 18.0, palette::INK, 26.0, &mut y);
        line("7 then drag         a wall between your city and the water", 18.0, palette::INK, 26.0, &mut y);
        line("right-click ground  send them there, and they stay", 18.0, palette::INK, 40.0, &mut y);

        ui::centred(
            "the panel always says what to do next - click to begin",
            card.y + card.h - 26.0,
            17.0,
            palette::GOOD,
        );

        if ui.clicked || ui.right_clicked || get_last_key_pressed().is_some() {
            self.shown = false;
        }
    }
}
