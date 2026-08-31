//! Everything on the screen, drawn with primitives.

use crate::palette;
use crate::screen::{MapView, CELL, LOGICAL_H, LOGICAL_W, PANEL_W};
use macroquad::prelude::*;
use sim::{PlayerId, World};

/// The map's own origin is its top-left corner, and everything below draws in
/// map space: `cell * CELL`. `screen::MapView` puts that on the screen, and
/// nothing here knows where on the canvas it lands.
pub const MAP_X: f32 = 0.0;
pub const MAP_Y: f32 = 0.0;

/// Everything under the map camera. The caller sets it; this draws in map
/// space and never asks where the window is.
pub fn world(w: &World, me: PlayerId, selected: &[sim::CitizenId], view: &MapView) {
    let seen = view.visible();
    ground(w, seen);
    water(w, seen);
    buildings(w, seen);
    citizens(w, selected);
    mules(w);
    pings(w);
    let _ = me;
}

/// Whether a footprint is anywhere near the window, for culling.
fn on_screen(seen: (i32, i32, i32, i32), x: i32, y: i32, w: i32, h: i32) -> bool {
    x + w > seen.0 && y + h > seen.1 && x <= seen.2 && y <= seen.3
}

fn ground(w: &World, seen: (i32, i32, i32, i32)) {
    let relief = *w.map.height.iter().max().unwrap_or(&1) as i32;
    for y in seen.1..=seen.3 {
        for x in seen.0..=seen.2 {
            let i = sim::Map::idx(x, y);
            draw_rectangle(
                MAP_X + x as f32 * CELL,
                MAP_Y + y as f32 * CELL,
                CELL,
                CELL,
                palette::ground(w.map.ground[i], w.map.height[i], relief),
            );
        }
    }
}

fn water(w: &World, seen: (i32, i32, i32, i32)) {
    if w.water.volume() == 0 {
        return;
    }
    for y in seen.1..=seen.3 {
        for x in seen.0..=seen.2 {
            let d = w.water.depth[sim::Map::idx(x, y)];
            if d == 0 {
                continue;
            }
            draw_rectangle(
                MAP_X + x as f32 * CELL,
                MAP_Y + y as f32 * CELL,
                CELL,
                CELL,
                palette::water(d),
            );
        }
    }
}

fn buildings(w: &World, seen: (i32, i32, i32, i32)) {
    for b in &w.buildings {
        if b.state == sim::building::BuildState::Rubble {
            continue;
        }
        let (bw, bh) = b.size();
        if !on_screen(seen, b.x as i32, b.y as i32, bw, bh) {
            continue;
        }
        let x = MAP_X + b.x as f32 * CELL;
        let y = MAP_Y + b.y as f32 * CELL;
        let (pw, ph) = (bw as f32 * CELL, bh as f32 * CELL);
        // A dike carries its own load on its face. Everything else is drawn
        // in its owner's colour flat.
        let colour = palette::strained(palette::player(b.owner), b.strain());

        if b.standing_now() {
            draw_rectangle(x, y, pw, ph, colour);
            draw_rectangle_lines(x, y, pw, ph, 1.0, palette::BACKDROP);
        } else {
            // A site is an outline: you can see what is coming and that it is
            // not there yet.
            draw_rectangle(x, y, pw, ph, Color { a: 0.20, ..colour });
            draw_rectangle_lines(x, y, pw, ph, 1.0, colour);
        }

        let g = palette::glyph(b.kind);
        if !g.is_empty() && pw >= CELL * 2.0 {
            let m = measure_text(g, None, 13, 1.0);
            draw_text(
                g,
                x + (pw - m.width) / 2.0,
                y + (ph + m.height) / 2.0,
                13.0,
                palette::BACKDROP,
            );
        }
    }
}

/// A circle with two lines for legs (design §1), and a ring in the owner's
/// colour when selected.
fn citizens(w: &World, selected: &[sim::CitizenId]) {
    for c in &w.citizens {
        if !c.alive() {
            continue;
        }
        let px = MAP_X + c.pos.x.raw() as f32 / 256.0 * CELL;
        let py = MAP_Y + c.pos.y.raw() as f32 / 256.0 * CELL;
        let colour = palette::player(c.owner);

        if selected.contains(&c.id) {
            draw_circle_lines(px, py, 5.0, 1.0, palette::INK);
        }
        draw_circle(px, py, 2.0, colour);
        draw_line(px, py + 1.5, px - 1.5, py + 4.0, 1.0, colour);
        draw_line(px, py + 1.5, px + 1.5, py + 4.0, 1.0, colour);

        // Somebody in trouble is worth seeing from across the map.
        if c.swept {
            draw_circle_lines(px, py, 4.0, 1.0, palette::ALARM);
        }
    }
}

/// A cart, and not another circle with two legs.
///
/// A mule is the only thing on the map that belongs to a city and is not one
/// of its people, and it has to read that way at a glance: a box on the road
/// with a load on it, in its owner's colour. Loaded or empty is the one bit of
/// its state worth seeing from across the map — an empty cart is on its way
/// out, a full one is bringing something home.
fn mules(w: &World) {
    for m in &w.mules {
        if !m.alive() {
            continue;
        }
        let px = MAP_X + m.pos.x.raw() as f32 / 256.0 * CELL;
        let py = MAP_Y + m.pos.y.raw() as f32 / 256.0 * CELL;
        let colour = palette::player(m.owner);

        draw_rectangle(px - 3.0, py - 2.0, 6.0, 4.0, colour);
        draw_rectangle_lines(px - 3.0, py - 2.0, 6.0, 4.0, 1.0, palette::BACKDROP);
        if m.carrying.gold > 0 {
            draw_rectangle(px - 1.5, py - 3.5, 3.0, 1.5, palette::WARNING);
        } else if m.carrying_any() {
            draw_rectangle(px - 2.0, py - 3.5, 4.0, 1.5, palette::BACKDROP);
        }
        // Nowhere to take it. Drawn, and said in the panel as well: a cart
        // standing in the yard for a reason nobody can see is the failure this
        // whole state exists to avoid.
        if m.leg == sim::Leg::Stuck {
            draw_circle_lines(px, py, 5.0, 1.0, palette::ALARM);
        }
    }
}

fn pings(w: &World) {
    for p in &w.pings {
        let age = w.tick.saturating_sub(p.at) as f32
            / sim::balance::PING_LIFETIME.max(1) as f32;
        let r = 4.0 + 14.0 * age;
        draw_circle_lines(
            MAP_X + p.x as f32 * CELL + CELL / 2.0,
            MAP_Y + p.y as f32 * CELL + CELL / 2.0,
            r,
            2.0,
            Color { a: 1.0 - age, ..palette::player(p.by) },
        );
    }
}

/// The side panel: what a player needs to know without looking away from the
/// map (plan phase 5).
/// Returns the y the fixed part of the panel ended at, so the tools below it
/// move down when there are six cities in the list instead of two.
pub fn panel(w: &World, me: PlayerId, status: &net::Status, build: &str, ticks: &[u32]) -> f32 {
    let x = LOGICAL_W - PANEL_W;
    draw_rectangle(x, 0.0, PANEL_W, LOGICAL_H, palette::PANEL);
    draw_line(x, 0.0, x, LOGICAL_H, 1.0, palette::RULE);

    let left = x + 18.0;
    let mut y = 44.0;
    let line = |text: &str, size: u16, colour: Color, y: &mut f32| {
        draw_text(text, left, *y, size as f32, colour);
        *y += size as f32 + 8.0;
    };

    line("FLOODLINE", 28, palette::INK, &mut y);
    y += 6.0;

    let omen = match w.omen() {
        sim::Omen::Quiet => "all quiet",
        sim::Omen::Uneasy => "the elders are uneasy",
        sim::Omen::Impact => "THE WATER IS HERE",
        sim::Omen::Aftermath => "it is over",
    };
    let omen_colour = match w.omen() {
        sim::Omen::Impact => palette::ALARM,
        sim::Omen::Uneasy => palette::WARNING,
        _ => palette::FAINT,
    };
    line(
        &format!("age {} of {}   day {} of {}", w.age(), sim::balance::MAX_AGE,
                 w.day_of_age(), sim::balance::DAYS_PER_AGE),
        18, palette::INK, &mut y,
    );
    line(omen, 18, omen_colour, &mut y);
    y += 10.0;

    let goods = w.treasury(me);
    line(
        &format!(
            "food {}   wood {}   stone {}   gold {}",
            goods.food, goods.wood, goods.stone, goods.gold
        ),
        18, palette::INK, &mut y,
    );
    y += 14.0;

    line("CITIES", 15, palette::FAINT, &mut y);
    for &p in &w.players {
        let alive = w.population(p);
        let mine = if p == me { " (you)" } else { "" };
        let gone = if w.dropped.contains(&p) { " - gone" } else { "" };
        draw_rectangle(left, y - 10.0, 10.0, 10.0, palette::player(p));
        draw_text(
            &format!("city {}{}: {} souls{}", p.0, mine, alive, gone),
            left + 18.0,
            y,
            17.0,
            if alive == 0 { palette::FAINT } else { palette::INK },
        );
        y += 24.0;
    }

    // The one thing wrong with this city that will kill it, if there is one.
    //
    // The plan's phase 5 asks the panel for a "warning line" and this is it.
    // It exists because a city with no granary is doomed and nothing said so:
    // food can only be stored in a granary and eaten at one, so a player whose
    // granary placement was refused - a fading red line they may not have been
    // looking at - watches everybody starve on day four with a farm working
    // beside them and no idea why.
    y += 14.0;
    // The row is always here, whether or not there is anything to put in it.
    // Reserving it costs twenty-five pixels of panel and buys buttons that do
    // not move under the cursor when a city's situation changes.
    // Two lines, wrapped on words: the panel is 330 pixels of usable width,
    // which is about fifty-six characters at this size, and a sentence that
    // says what to do next does not fit in one. Both rows are reserved whether
    // or not there is anything in them, so the buttons below do not move as a
    // city's situation changes.
    if let Some(next) = crate::tutorial::next_thing(w, me) {
        for (i, row) in crate::ui::wrapped_words(next, 52).iter().take(2).enumerate() {
            draw_text(row, left, y + i as f32 * 18.0, 15.0, palette::WARNING);
        }
    }
    y += 42.0;

    let (text, colour) = match status {
        net::Status::Lobby => ("in the lobby - press SPACE to start".to_owned(), palette::WARNING),
        net::Status::Playing => ("playing".to_owned(), palette::FAINT),
        net::Status::WaitingOn(who) => (
            format!("waiting on {}", who.iter().map(|p| format!("city {}", p.0))
                .collect::<Vec<_>>().join(", ")),
            palette::WARNING,
        ),
        net::Status::Desync { with, tick } => (
            format!("DESYNC with city {} at tick {tick}", with.0), palette::ALARM,
        ),
        net::Status::Ended(reason) => (reason.clone(), palette::ALARM),
    };
    line(&text, 17, colour, &mut y);
    let ended = y;

    // The bottom of the panel is for the things you only look at when
    // something is wrong.
    let mut y = LOGICAL_H - 74.0;
    line(&format!("tick {}", w.tick), 15, palette::FAINT, &mut y);
    line(&format!("peers at {ticks:?}"), 15, palette::FAINT, &mut y);
    line(&format!("build {build}   seed {}", w.seed), 15, palette::FAINT, &mut y);
    ended
}

/// The score screen (design §4).
pub fn score(w: &World) {
    let s = w.score();
    let panel = Rect::new(LOGICAL_W / 2.0 - 320.0, LOGICAL_H / 2.0 - 200.0, 640.0, 400.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color { a: 0.96, ..palette::PANEL });
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 2.0, palette::RULE);

    let left = panel.x + 40.0;
    let mut y = panel.y + 64.0;
    let line = |text: &str, size: u16, colour: Color, y: &mut f32| {
        draw_text(text, left, *y, size as f32, colour);
        *y += size as f32 + 12.0;
    };

    let ending = match w.ending {
        Some(sim::Ending::AgesRanOut) => "The map stood.",
        Some(sim::Ending::LastCityFell) => "The last city fell.",
        None => "",
    };
    line(ending, 32, palette::INK, &mut y);
    line(&format!("{} ages survived, over {} days", s.ages_survived, s.days), 20,
         palette::INK, &mut y);
    y += 10.0;

    for c in &s.cities {
        draw_rectangle(left, y - 12.0, 12.0, 12.0, palette::player(c.player));
        draw_text(
            &format!(
                "city {}: {} at its height, {} left - {}",
                c.player.0, c.peak_population, c.final_population,
                if c.survived { "standing" } else { "gone" }
            ),
            left + 22.0, y, 18.0,
            if c.survived { palette::INK } else { palette::FAINT },
        );
        y += 26.0;
    }

    y += 16.0;
    line(&format!("seed {} - the same map again, if you want it", s.seed), 16,
         palette::FAINT, &mut y);
    // The plan's "New run". There is no button because there is nothing for a
    // button to do that the lobby does not already do better: a new run is a
    // new room, a new seed and possibly a different set of people.
    y += 10.0;
    line("ENTER for a new run", 20, palette::INK, &mut y);
}
