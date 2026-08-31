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

/// What to say about the larder, if it is worth saying anything.
///
/// Its own function so the sentences can be tested at the widths a big city
/// makes of them: the panel keeps two rows of 52 columns and drops a third
/// without a word, and these are the only lines here that carry numbers.
///
/// It exists because of the M10.5 rehearsal, where the line said "the granary
/// is empty - give the farm a moment", unchanged, for two days while both
/// cities starved with their farms staffed three-of-three. It named the
/// mechanism and never the clock, so neither player could tell "a day too
/// slow" from "the food is not moving at all", and both of them separately
/// asked for this number afterwards.
fn larder(mouths: u32, eaten: u32, food: u16) -> Option<String> {
    if food == 0 {
        return Some(format!(
            "the granary is empty. {mouths} mouths eat {eaten} a day - \
             more farmers, or fewer hands carrying stone"
        ));
    }
    // Less than a day left. `days_of_food` would say nought; this says it in
    // the words a player can act on.
    if eaten > 0 && u32::from(food) < eaten {
        return Some(format!(
            "{food} food left, and {mouths} mouths eat {eaten} a day - under a day"
        ));
    }
    None
}

/// What to say when there is nobody left to say it to.
///
/// `next_thing` answers `None` for a dead city, which is correct — there is no
/// next thing — but the panel then drew a blank row and the status line went on
/// saying `playing`. Both players in the M10.5 rehearsal lost their city
/// without being told: one found out by noticing a grey nought in the roster.
///
/// Not while the run is over: the score screen says it better and says it for
/// everybody. This is for the gap between losing your own city and the run
/// ending, which can be two whole ages if the other city is standing.
pub fn obituary(w: &World, me: PlayerId) -> Option<&'static str> {
    let playing = w.players.contains(&me) && w.finished().is_none();
    (playing && w.population(me) == 0)
        .then_some("your city is gone. nothing is left to command")
}

/// The one thing this city most needs, and how to do it.
///
/// Ordered by what kills you soonest: a citizen empties at tick 1000 and dies
/// 3600 later, so a city with nowhere to eat has until day four, and the water
/// does not come until day six. Food first, always.
pub fn next_thing(w: &World, me: PlayerId) -> Option<String> {
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

    // **The water outranks everything, including the bootstrap.**
    //
    // This ladder is "what kills you soonest" and it was ordered as though the
    // only clock were food: the water was the *bottom* rung, under trade, gold
    // and levelling a farm. So in the M11.9 run it spent the whole of age 2
    // telling both players to build a trading post - one of them with a
    // stalled household, no children and a flood two days out.
    //
    // Above the granary, and that is the deliberate part. Drowning tomorrow
    // beats starving in three days, and a city with no granary and a flood on
    // the map has two problems of which only one is happening now: telling it
    // to press 3 is telling it to go and stand in the floodplain. The water is
    // the only thing in this game that keeps a calendar, so it is the only
    // thing that can be *late*.
    if w.omen() == sim::Omen::Impact {
        return Some("the water is here: choose everybody and send them uphill".to_owned());
    }
    if w.omen() == sim::Omen::Uneasy {
        return Some(
            if w.buildings.iter().any(|b| {
                b.owner == me && b.kind == Kind::Dike && b.state != sim::building::BuildState::Rubble
            }) {
                "the water comes tomorrow: choose everybody, send them uphill".to_owned()
            } else {
                "the water comes tomorrow: get uphill, or press 7 and drag a wall".to_owned()
            },
        );
    }

    // Food, in the order it can go wrong.
    if !placed(Kind::Granary) {
        return Some("press 3, click the ground: a granary. food is kept there, and eaten there".to_owned());
    }
    if !placed(Kind::Farm) {
        return Some("press 2, click the ground: a farm. nothing else grows food".to_owned());
    }
    if !standing(Kind::Granary) || !standing(Kind::Farm) {
        return Some("they are being built - your people fetch the wood themselves".to_owned());
    }
    if !working(Kind::Farm) {
        return Some("drag to choose your people, then right-click the farm".to_owned());
    }
    // What the city costs to keep, whenever the larder is thin.
    if let Some(line) = larder(w.population(me), w.eaten_a_day(me), w.treasury(me).food) {
        return Some(line);
    }

    // Then the two things that run out.
    let goods = w.treasury(me);
    if goods.wood < Kind::Cottage.cost().wood && !placed(Kind::Forester) {
        return Some("low on wood: press 4 for a forester's hut, the only source".to_owned());
    }
    if standing(Kind::Forester) && !working(Kind::Forester) {
        return Some("nobody is cutting wood: right-click the forester's hut".to_owned());
    }
    if goods.stone < Kind::Dike.cost().stone * 8 && !placed(Kind::Quarry) {
        return Some("low on stone: press 5 for a quarry. it needs rock beside it".to_owned());
    }
    if standing(Kind::Quarry) && !working(Kind::Quarry) {
        return Some("nobody is at the quarry: right-click it".to_owned());
    }

    // Trade, once the city can feed itself and cut its own timber. A cart
    // with nowhere to take its load is the one thing here that is *not*
    // advice — it is a report, and without it a player watches a mule stand in
    // the yard and has no way to find out why.
    if w.mules.iter().any(|m| m.alive() && m.owner == me && m.leg == sim::Leg::Stuck) {
        return Some("a mule has nowhere to take its load: no other city it can reach".to_owned());
    }
    // A household that has been settling for days with nothing else wrong is
    // the fault M11.9 could not name - "nothing anywhere told me what was
    // still missing" - and it outranks trade, which is a luxury.
    if w.households.iter().any(|h| h.owner == me && h.alive() && !h.settled()) {
        return Some(
            "a household settles when both of them stay fed: keep the granary stocked"
                .to_owned(),
        );
    }
    if standing(Kind::Forester) && !placed(Kind::TradingPost) {
        return Some("press 8 for a trading post: its mules sell wood abroad for gold".to_owned());
    }
    if standing(Kind::TradingPost) && !working(Kind::TradingPost) {
        return Some("nobody is at the trading post: right-click it to send a mule out".to_owned());
    }
    // Then the next generation.
    if standing(Kind::Cottage) && !placed(Kind::Nursery) {
        return Some("press 9 for a nursery: no nursery, no children".to_owned());
    }
    if standing(Kind::Nursery)
        && !w.households.iter().any(|h| h.owner == me && h.alive())
    {
        return Some("put two people in one cottage: a day of that makes a household".to_owned());
    }
    if w.treasury(me).gold >= sim::balance::UPGRADE_GOLD {
        return Some("you have gold: click a farm and level it - a level is one more pair of hands".to_owned());
    }

    // And the wall, when there is time to build one. Hoisted above trade
    // would make it the answer to everything; left here it is what a city
    // does with a quiet day, which is what a quiet day is for.
    if !placed(Kind::Dike) {
        return Some("press 7 and drag a wall between your city and the water".to_owned());
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
        line("8                   a trading post: mules sell wood for gold", 18.0, palette::INK, 26.0, &mut y);
        line("click a building    then level it with gold, or m to move it", 18.0, palette::INK, 26.0, &mut y);
        line("9                   a nursery. two in a cottage make a family", 18.0, palette::INK, 26.0, &mut y);
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

#[cfg(test)]
mod tests {
    use sim::{PlayerId, World};

    /// Every sentence this can say has to fit the two rows the panel reserves.
    ///
    /// `draw::panel` wraps at 52 columns and takes two rows; a third is dropped
    /// without a word. The food lines are the longest here and the only ones
    /// that carry numbers, so they are the ones that can grow past the edge
    /// when a city does — a city of thirty eats three digits a day.
    /// A dead city is told so, and a living one is not.
    #[test]
    fn a_city_that_has_died_is_told() {
        let mut w = World::new(5, 2);
        let me = PlayerId(0);
        assert_eq!(super::obituary(&w, me), None, "a living city has no obituary");

        for c in w.citizens.iter_mut().filter(|c| c.owner == me) {
            c.die();
        }
        assert_eq!(w.population(me), 0);
        let line = super::obituary(&w, me).expect("a dead city is told so");
        assert!(crate::ui::wrapped_words(line, 52).len() <= 2, "too wide: {line:?}");

        // And `next_thing` still says nothing, which is the right answer to
        // "what should I do next" and the reason this function exists.
        assert_eq!(super::next_thing(&w, me), None);
    }

    #[test]
    fn every_line_fits_the_two_rows_the_panel_keeps_for_it() {
        let mut w = World::new(3, 2);
        let me = PlayerId(0);

        // Walk a city through the states that produce a line, checking each.
        let mut seen = 0;
        for _ in 0..40 {
            if let Some(line) = super::next_thing(&w, me) {
                let rows = crate::ui::wrapped_words(&line, 52);
                assert!(
                    rows.len() <= 2,
                    "{:?} needs {} rows and the panel keeps two",
                    line,
                    rows.len()
                );
                seen += 1;
            }
            w.tick(&mut sim::nav::Nav::new(), &[]);
        }
        assert!(seen > 0, "the tutorial line never said anything");

        // And the worst case the numbers can make. Not played to: the point
        // is the width of the digits, and a city of ninety with three-digit
        // consumption is the widest these sentences ever get.
        for (mouths, eaten, food) in [(8u32, 96u32, 0u16), (30, 360, 0), (90, 1080, 0),
                                      (8, 96, 5), (90, 1080, 999)] {
            let line = super::larder(mouths, eaten, food)
                .unwrap_or_else(|| panic!("a hungry city of {mouths} says nothing"));
            assert!(line.contains(&eaten.to_string()), "it should name what they eat: {line:?}");
            let rows = crate::ui::wrapped_words(&line, 52);
            assert!(rows.len() <= 2, "{:?} needs {} rows and the panel keeps two", line, rows.len());
        }

        // A city that is fed says nothing about the larder at all.
        assert_eq!(super::larder(8, 96, 500), None);
    }

    /// The flood outranks trade, and a stalled household outranks it too.
    ///
    /// The ladder used to be "what kills you soonest" and stopped thinking
    /// after food: the water was the *bottom* rung, under trade, gold and
    /// levelling a farm. In the M11.9 run it spent the whole of age 2 telling
    /// both players to build a trading post - one of them with a stalled
    /// household, no children and a flood two days out.
    #[test]
    fn the_water_outranks_the_shopping() {
        let mut w = World::new(31, 2);
        let me = PlayerId(0);

        // No setup at all, deliberately: this city has not built a granary and
        // is still told about the water. That is the ordering under test - an
        // empty larder is a three-day problem and the flood is a one-day one.

        // Put it on the day before the impact, which is what `Uneasy` is.
        let day = sim::balance::TICKS_PER_DAY;
        w.age_start_tick = 0;
        w.tick = (sim::World::IMPACT_DAY - 2) * day + 1;
        assert_eq!(w.omen(), sim::Omen::Uneasy, "the test did not reach the day it needed");

        let line = super::next_thing(&w, me).unwrap_or_default();
        assert!(
            line.contains("water") || line.contains("uphill"),
            "with the water a day out the panel said: {line:?}"
        );
        assert!(
            !line.contains("trading post"),
            "the flood is tomorrow and it is still shopping: {line:?}"
        );

        // And on the day itself.
        w.tick = (sim::World::IMPACT_DAY - 1) * day + 1;
        assert_eq!(w.omen(), sim::Omen::Impact);
        let line = super::next_thing(&w, me).unwrap_or_default();
        assert!(line.contains("uphill"), "the water is here and the panel said: {line:?}");
    }
}
