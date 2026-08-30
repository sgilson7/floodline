//! Walls: how a line drawn on the map becomes a run of three-cell segments.
//!
//! A dike is the one building a player draws rather than places, and the one
//! whose shape the flood argues with, so its geometry gets its own file. The
//! pressure that breaks it is here too, beside the shape it acts on.

use sim::balance::*;
use sim::building::{BuildState, Facing, Good, Kind};
use sim::citizen::PlayerId;
use sim::command::Command;
use sim::world::{RuleError, World};
use sim::map::{Ground, CELLS};
use sim::BuildingId;

const ME: PlayerId = PlayerId(0);

/// A world with no relief and nothing but grass, so a wall is only ever
/// refused for a reason the test put there.
fn flat() -> World {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    w
}

/// Every cell of every dike the player owns, sorted.
fn walled(w: &World) -> Vec<(i32, i32)> {
    let mut cells: Vec<(i32, i32)> = w
        .buildings
        .iter()
        .filter(|b| b.kind == Kind::Dike && b.state != BuildState::Rubble)
        .flat_map(|b| b.cells())
        .collect();
    cells.sort();
    cells
}

#[test]
fn a_line_is_laid_in_whole_segments_end_to_end() {
    let mut w = flat();
    let built = w.lay_dike_line(ME, (20, 50), (28, 50)).unwrap();

    // Nine cells is exactly three segments, and they meet.
    assert_eq!(built.len(), 3);
    assert_eq!(walled(&w), (20..=28).map(|x| (x, 50)).collect::<Vec<_>>());

    for id in built {
        let b = &w.buildings[id.0 as usize];
        assert_eq!(b.owner, ME);
        assert_eq!(b.facing, Facing::EastWest);
        assert_eq!(b.state, BuildState::Site, "a wall is hauled and built like anything else");
        assert_eq!(b.outstanding(), Kind::Dike.cost(), "and paid for by the segment");
    }
}

#[test]
fn a_wall_stays_on_the_line_it_was_drawn_from() {
    // A road takes the cheapest path. A wall does not wander: it snaps to
    // whichever axis the drag was longer along and keeps the row it started
    // on, so what the ghost showed is what gets built.
    let mut w = flat();
    w.lay_dike_line(ME, (20, 50), (40, 56)).unwrap();
    assert!(walled(&w).iter().all(|&(_, y)| y == 50), "the wall drifted off its row");

    let mut w = flat();
    w.lay_dike_line(ME, (20, 50), (26, 80)).unwrap();
    assert!(walled(&w).iter().all(|&(x, _)| x == 20), "the wall drifted off its column");
}

#[test]
fn a_wall_reaches_the_cursor_and_may_overshoot_it_but_never_falls_short() {
    // A wall with a one-cell hole in it is not a wall, so a run that does not
    // divide by three rounds up rather than leaving a gap at the far end.
    for len in 1..=12i32 {
        let mut w = flat();
        let to = 20 + len - 1;
        w.lay_dike_line(ME, (20, 50), (to as u8, 50)).unwrap();
        let cells = walled(&w);

        let far = cells.iter().map(|&(x, _)| x).max().unwrap();
        assert!(far >= to, "a {len}-cell run stopped at {far}, short of {to}");
        assert!(far < to + DIKE_LENGTH, "a {len}-cell run overshot {to} by more than a segment");
        assert_eq!(cells, (20..=far).map(|x| (x, 50)).collect::<Vec<_>>(), "a hole at {len}");
    }
}

#[test]
fn a_run_drawn_backwards_is_the_same_wall() {
    let mut forwards = flat();
    forwards.lay_dike_line(ME, (20, 50), (31, 50)).unwrap();
    let mut backwards = flat();
    backwards.lay_dike_line(ME, (31, 50), (20, 50)).unwrap();
    assert_eq!(walled(&forwards), walled(&backwards));
}

#[test]
fn a_north_south_wall_is_one_cell_thick() {
    let mut w = flat();
    w.lay_dike_line(ME, (60, 20), (60, 28)).unwrap();
    assert_eq!(walled(&w), (20..=28).map(|y| (60, y)).collect::<Vec<_>>());
}

#[test]
fn a_line_drawn_across_a_building_walls_both_sides_of_it() {
    // Partial placement, and the reason for it: the alternative is a tool
    // that refuses the whole wall because one segment in the middle of it
    // clipped the corner of a farm, which is a tool a player stops using.
    let mut w = flat();
    let farm = w.place(ME, Kind::Farm, Facing::EastWest, 26, 50).unwrap();
    let built = w.lay_dike_line(ME, (20, 50), (40, 50)).unwrap();

    let cells = walled(&w);
    assert!(!built.is_empty());
    assert!(cells.iter().any(|&(x, _)| x < 26), "nothing was walled before the farm");
    assert!(cells.iter().any(|&(x, _)| x > 28), "nothing was walled beyond the farm");
    for (fx, fy) in w.buildings[farm.0 as usize].cells() {
        assert!(!cells.contains(&(fx, fy)), "a segment was laid through the farm");
    }
}

#[test]
fn a_line_with_nowhere_to_go_says_why() {
    let mut w = flat();
    for i in 0..CELLS {
        w.map.ground[i] = Ground::Rock;
    }
    assert_eq!(w.lay_dike_line(ME, (20, 50), (40, 50)), Err(RuleError::WrongGround));
    assert_eq!(w.lay_dike_line(ME, (20, 50), (200, 50)), Err(RuleError::NoSuchCell));
}

#[test]
fn the_ghost_and_the_wall_are_the_same_arithmetic() {
    // `plan_dike_line` is what the drag tool draws and totals the cost from.
    // If it could disagree with `lay_dike_line`, a player would be shown a
    // wall and sold a different one.
    let mut w = flat();
    w.place(ME, Kind::Granary, Facing::EastWest, 30, 50).unwrap();

    let planned = w.plan_dike_line(ME, (20, 50), (44, 50));
    let built = w.lay_dike_line(ME, (20, 50), (44, 50)).unwrap();
    let origins: Vec<(i32, i32)> = built
        .iter()
        .map(|id| {
            let b = &w.buildings[id.0 as usize];
            (b.x as i32, b.y as i32)
        })
        .collect();
    assert_eq!(planned, origins);
    assert!(!planned.is_empty());
}

#[test]
fn a_wall_is_paid_for_by_the_segment() {
    let mut w = flat();
    let built = w.lay_dike_line(ME, (20, 50), (38, 50)).unwrap();
    let owed: u16 = built
        .iter()
        .map(|id| w.buildings[id.0 as usize].outstanding().stone)
        .sum();
    assert_eq!(owed, Kind::Dike.cost().stone * built.len() as u16);
    assert!(
        owed <= STARTING_STONE,
        "a nineteen-cell wall costs {owed} against an opening purse of {STARTING_STONE}"
    );
}

#[test]
fn only_your_own_wall() {
    let mut w = flat();
    assert_eq!(w.lay_dike_line(PlayerId(7), (20, 50), (40, 50)), Err(RuleError::NotYours));
}

#[test]
fn a_wall_drawn_over_the_wire_is_the_same_wall() {
    // The command is the wire format, so a line that cannot be written down
    // and read back is a line two peers can disagree about.
    let mut direct = flat();
    direct.lay_dike_line(ME, (20, 50), (40, 50)).unwrap();

    let cmd = Command::DikeLine { from: (20, 50), to: (40, 50) };
    assert_eq!(Command::parse(&cmd.line()), Some(cmd.clone()));

    let mut applied = flat();
    applied.apply(ME, &cmd).unwrap();
    assert_eq!(walled(&direct), walled(&applied));
}

#[test]
fn a_finished_wall_raises_the_ground_the_water_sees() {
    // The point of all of it: the flood asks `effective_height`, and a wall
    // has to answer for every cell of every segment and not only its corner.
    let mut w = flat();
    let built = w.lay_dike_line(ME, (20, 50), (28, 50)).unwrap();
    let bare = w.effective_height(20, 50);

    for id in &built {
        w.deliver_to(*id, Good::Stone, Kind::Dike.cost().stone);
        w.build_at(*id, Kind::Dike.build_ticks());
    }
    for (x, y) in walled(&w) {
        assert_eq!(
            w.effective_height(x, y),
            bare + DIKE_HEIGHT_PER_LEVEL,
            "the water sees flat ground at ({x}, {y})"
        );
    }
}

#[test]
fn a_segment_is_raised_whole() {
    // `RaiseDike` takes a building, and a segment is one building, so raising
    // it lifts all three of its cells at once. That is the trade the drag
    // tool makes: you buy height by the segment, not by the cell.
    let mut w = flat();
    let built = w.lay_dike_line(ME, (20, 50), (22, 50)).unwrap();
    let id: BuildingId = built[0];
    w.deliver_to(id, Good::Stone, Kind::Dike.cost().stone);
    w.build_at(id, Kind::Dike.build_ticks());

    w.raise_dike(ME, id).unwrap();
    w.deliver_to(id, Good::Stone, w.buildings[id.0 as usize].outstanding().stone);
    w.build_at(id, Kind::Dike.build_ticks());
    assert_eq!(w.buildings[id.0 as usize].level, 2);

    let bare = 40u16;
    for x in 20..=22 {
        assert_eq!(w.effective_height(x, 50), bare + DIKE_HEIGHT_PER_LEVEL * 2);
    }
}
