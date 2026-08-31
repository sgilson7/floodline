//! Citizens take up room.
//!
//! Two rules, and both of them are about what a player sees: eight people
//! standing at a hearth were one circle with a number inside it, and a citizen
//! walking a straight line to a granary walked through the wall of whatever
//! was in the way.
//!
//! What it costs is in `tests/profile.rs` with everything else: a tick at five
//! hundred citizens with the flood running went from 0.36 ms to 0.46 against a
//! twenty-millisecond budget.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::command::Command;
use sim::fx::{Fx, V2};
use sim::map::Map;
use sim::nav::{self, Nav};
use sim::world::World;
use sim::PlayerId;

const ME: PlayerId = PlayerId(0);

fn build(w: &mut World, kind: Kind) -> sim::BuildingId {
    let (hx, hy) = w.map.hearth_sites[0];
    for r in 3..30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(ME, kind, Facing::EastWest, x, y).is_ok() {
                    w.apply(ME, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).unwrap();
                    let id = w.buildings.last().unwrap().id;
                    for g in Good::ALL {
                        let want = w.buildings[id.0 as usize].outstanding().get(g);
                        if want > 0 {
                            w.deliver_to(id, g, want);
                        }
                    }
                    assert!(w.build_at(id, kind.build_ticks()));
                    return id;
                }
            }
        }
    }
    panic!("nowhere for a {kind:?}");
}

/// The closest two of one player's living citizens are standing, squared.
fn tightest_of(w: &World, owner: PlayerId) -> i32 {
    let mut worst = i32::MAX;
    let mine: Vec<&sim::Citizen> =
        w.citizens.iter().filter(|c| c.owner == owner && c.alive()).collect();
    for (i, a) in mine.iter().enumerate() {
        for b in mine.iter().skip(i + 1) {
            worst = worst.min((a.pos - b.pos).len_sq().0);
        }
    }
    worst
}

#[test]
fn nobody_ends_a_tick_standing_in_a_wall() {
    // Including the very first one. Every citizen is spawned on its hearth's
    // site, and a Hearth blocks movement, so at tick zero the whole founding
    // party is inside a building.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    build(&mut w, Kind::Farm);
    build(&mut w, Kind::Granary);
    build(&mut w, Kind::Cottage);

    for t in 0..TICKS_PER_DAY * 2 {
        w.tick(&mut nav, &[]);
        for c in w.citizens.iter().filter(|c| c.alive()) {
            let (x, y) = c.pos.cell();
            assert!(
                Map::contains(x, y),
                "tick {t}: #{} walked off the map at ({x},{y})",
                c.id.0
            );
            assert!(
                nav::passable(&w, x, y),
                "tick {t}: #{} is standing in {:?} at ({x},{y})",
                c.id.0,
                w.building_at(x, y).map(|b| b.kind)
            );
        }
    }
}

#[test]
fn a_crowd_loosens_instead_of_standing_in_one_spot() {
    // Eight people in one cell: a single circle with a number of people
    // inside it, which is what the hearth used to look like.
    //
    // **The test bunches them itself.** It used to lean on the founding party
    // spawning inside `ELBOW_ROOM` on seed 31, and when the river moved the
    // hearth sites that party spawned a little wider — so the rule was
    // untouched, the premise was gone, and the test failed having measured
    // nothing. Where a party lands is a fact about the map; this is a question
    // about the rule.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    build(&mut w, Kind::Granary);

    let (hx, hy) = w.map.hearth_sites[0];
    let spot = (3..8i32)
        .flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r)].into_iter())
        .map(|(dx, dy)| (hx + dx, hy + dy))
        .find(|&(x, y)| nav::passable(&w, x, y))
        .expect("nowhere beside the hearth to stand");

    let mine: Vec<usize> = w
        .citizens
        .iter()
        .enumerate()
        .filter(|(_, c)| c.owner == ME && c.alive())
        .map(|(i, _)| i)
        .collect();
    assert!(mine.len() >= 4);
    for (n, &i) in mine.iter().enumerate() {
        // Not exactly on top of one another: a vector shorter than about a
        // sixteenth of a cell normalises to zero in `Fx`, and a crowd with no
        // direction to push in is a different bug from a crowd that will not
        // push. `crowd.rs` has a fallback for it; this test is not about that.
        w.citizens[i].halt();
        w.citizens[i].pos = V2::cell_centre(spot.0, spot.1)
            + V2::new(Fx(n as i32 * 3), Fx(n as i32 * 5));
    }

    let before = tightest_of(&w, ME);
    assert!(before < ELBOW_ROOM_SQ.0, "the test did not manage to bunch them: {before}");
    for _ in 0..200 {
        w.tick(&mut nav, &[]);
    }
    let after = tightest_of(&w, ME);
    assert!(
        after > before,
        "the crowd did not loosen at all: {before} -> {after} (squared)"
    );

    assert!(
        after >= ELBOW_ROOM_SQ.0 / 4,
        "two of them are still all but on top of each other: {after} squared, \
         wanted about {}",
        ELBOW_ROOM_SQ.0
    );
}

#[test]
fn a_crowd_pushed_at_a_wall_does_not_go_through_it() {
    // The case a separation rule gets wrong: everybody shoved toward the same
    // building at once. They may slide along it and they may not reach where
    // they were going, but none of them may end up inside it.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let granary = build(&mut w, Kind::Granary);
    let (gx, gy) = {
        let b = &w.buildings[granary.0 as usize];
        (b.x as i32, b.y as i32)
    };

    // Stack the whole city on the granary's doorstep.
    for i in 0..w.citizens.len() {
        if w.citizens[i].owner == ME {
            w.citizens[i].pos = V2::cell_centre(gx - 1, gy);
            w.citizens[i].halt();
        }
    }
    for _ in 0..120 {
        w.tick(&mut nav, &[]);
        for c in w.citizens.iter().filter(|c| c.alive() && c.owner == ME) {
            let (x, y) = c.pos.cell();
            assert!(
                w.building_at(x, y).map(|b| b.id) != Some(granary),
                "#{} was pushed inside the granary at ({x},{y})",
                c.id.0
            );
        }
    }
}

#[test]
fn the_crowd_settles_the_same_way_on_two_peers() {
    // The rule that makes this safe to have at all. Two worlds from the same
    // seed, ticked the same number of times, have to agree to the byte — a
    // crowd resolved in a different order on two machines would push the same
    // two people in different directions and the game would come apart inside
    // a second.
    let mut a = World::new(31, 3);
    let mut b = World::new(31, 3);
    let (mut na, mut nb) = (Nav::new(), Nav::new());
    for _ in 0..TICKS_PER_DAY {
        a.tick(&mut na, &[]);
        b.tick(&mut nb, &[]);
    }
    assert_eq!(a.checksum(), b.checksum(), "two peers settled the crowd differently");
}
