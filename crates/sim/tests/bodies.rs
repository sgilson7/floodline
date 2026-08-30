//! What the flood does to people and buildings.
//!
//! The plan names two: a citizen on open lowland in a height-18 surge "ends at
//! the far edge or dead", and one on the high corner is untouched. Both are
//! here, along with the rest of design §3.4's third paragraph.

use sim::balance::*;
use sim::building::{Good, Kind};
use sim::citizen::PlayerId;
use sim::fx::V2;
use sim::map::{Corner, Ground, CELLS, MAP_H, MAP_W};
use sim::nav::Nav;
use sim::world::World;

/// A flat world with a surge already scheduled, wound to the impact day.
fn at_the_impact_day(height: u16) -> (World, Nav) {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    w.disaster.sources = vec![(Corner::NorthWest, 0)];
    w.disaster.height = height;
    let mut nav = Nav::new();
    while w.day_of_age() < World::IMPACT_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }
    (w, nav)
}

fn keep_fed(w: &mut World) {
    for c in &mut w.citizens {
        if c.alive() {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
    }
}

#[test]
fn a_citizen_on_open_lowland_is_swept_away_or_drowned() {
    let (mut w, mut nav) = at_the_impact_day(18);

    // Standing in the path, a dozen cells in from the corner the water comes
    // out of, with nothing to climb.
    let victim = w.citizens[0].id;
    let start = V2::cell_centre(14, 14);
    w.citizens[victim.0 as usize].pos = start;

    let mut ever_swept = false;
    for _ in 0..SURGE_TICKS + 400 {
        keep_fed(&mut w);
        w.tick(&mut nav, &[]);
        ever_swept |= w.citizens[victim.0 as usize].swept;
        if !w.citizens[victim.0 as usize].alive() {
            break;
        }
    }

    let c = &w.citizens[victim.0 as usize];
    let moved = (c.pos - start).len().floor();
    assert!(
        !c.alive() || moved > 3,
        "a citizen stood in an age-two flood and neither drowned nor moved: \
         alive {}, moved {moved} cells, swept at some point {ever_swept}",
        c.alive()
    );
    assert!(ever_swept || !c.alive(), "never lost its footing either");
}

#[test]
fn a_citizen_on_the_high_corner_is_untouched() {
    // Not the flat test world — the real one, where the high corner is high.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let (hx, hy) = w.map.high_corner.cell();
    let (sx, sy) = (hx.min(MAP_W - 3).max(2), hy.min(MAP_H - 3).max(2));

    let safe = w.citizens[0].id;
    w.citizens[safe.0 as usize].pos = V2::cell_centre(sx, sy);
    let start = w.citizens[safe.0 as usize].pos;

    while w.day_of_age() < World::IMPACT_DAY {
        keep_fed(&mut w);
        w.citizens[safe.0 as usize].halt();
        w.tick(&mut nav, &[]);
        w.citizens[safe.0 as usize].pos = start;
    }
    for _ in 0..SURGE_TICKS + 400 {
        keep_fed(&mut w);
        w.citizens[safe.0 as usize].halt();
        w.tick(&mut nav, &[]);
        w.citizens[safe.0 as usize].pos = start;
    }

    let c = &w.citizens[safe.0 as usize];
    assert!(c.alive(), "the high corner drowned somebody");
    assert!(!c.swept, "the high corner swept somebody off their feet");
    assert_eq!(c.drowning_for, 0, "somebody on the high corner was out of their depth");
    assert_eq!(w.water.depth_at(sx, sy), 0, "the high corner got wet at all");
}

#[test]
fn a_roof_is_worth_standing_on() {
    // Design §5: high ground or a rooftop. A granary keeps five units of water
    // off you, which is the difference between wading and drowning.
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    let id = w.place(PlayerId(0), Kind::Granary, 20, 20).unwrap();
    w.deliver_to(id, Good::Wood, Kind::Granary.cost().wood);
    assert!(w.build_at(id, Kind::Granary.build_ticks()));

    // Four units of water: over the wading line everywhere, and over the
    // swimming line for nobody standing on a granary.
    for y in 18..24 {
        for x in 18..24 {
            w.water.raise_to(x, y, depth(4));
        }
    }
    assert!(w.depth_over(19, 19) >= WADE_DEPTH, "the ground should be wet");
    assert_eq!(w.depth_over(20, 20), 0, "the roof should be dry");

    // And enough to swim in on the ground is still standable on the roof.
    for y in 18..24 {
        for x in 18..24 {
            w.water.raise_to(x, y, depth(7));
        }
    }
    assert!(w.depth_over(19, 19) >= SWIM_DEPTH, "seven deep is out of your depth");
    assert!(w.depth_over(20, 20) < SWIM_DEPTH, "and a roof is five of that");
}

#[test]
fn a_rooftop_is_only_a_rooftop_while_the_building_stands() {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    let id = w.place(PlayerId(0), Kind::Cottage, 30, 30).unwrap();
    w.water.raise_to(30, 30, depth(6));
    assert_eq!(w.depth_over(30, 30), depth(6), "a building site is not a roof");

    w.deliver_to(id, Good::Wood, Kind::Cottage.cost().wood);
    w.build_at(id, Kind::Cottage.build_ticks());
    assert_eq!(w.depth_over(30, 30), depth(2), "a cottage keeps four off you");

    w.damage_building(id, Kind::Cottage.integrity());
    assert_eq!(w.depth_over(30, 30), depth(6), "and rubble keeps nothing off");
}

/// Put a building in the path of a real surge on flat ground and see how long
/// it lasts, in ticks. `None` means it was still standing when the water went.
fn survives_the_front(kind: Kind) -> Option<u32> {
    let (mut w, mut nav) = at_the_impact_day(18);
    // Close enough to the corner to be in the front itself.
    let id = w.place(PlayerId(0), kind, 16, 16).unwrap();
    for g in Good::ALL {
        let want = kind.cost().get(g);
        if want > 0 {
            w.deliver_to(id, g, want);
        }
    }
    assert!(w.build_at(id, kind.build_ticks()), "{kind:?} did not finish");

    for t in 0..SURGE_TICKS + 400 {
        keep_fed(&mut w);
        w.tick(&mut nav, &[]);
        if w.buildings[id.0 as usize].state == sim::building::BuildState::Rubble {
            return Some(t);
        }
    }
    None
}

#[test]
fn the_flood_breaks_the_roads_it_runs_over() {
    // Design §6: "the flood breaks road cells it flows over, which is what
    // makes rebuilding the link after an age a decision".
    let road = survives_the_front(Kind::Road);
    assert!(road.is_some(), "a road in the front of an age-two surge survived it");
}

#[test]
fn wood_gives_way_before_stone() {
    let cottage = survives_the_front(Kind::Cottage).expect("a cottage in the front survived");
    let dike = survives_the_front(Kind::Dike);
    match dike {
        None => {} // the dike outlasted the flood entirely, which is the point
        Some(t) => assert!(
            cottage < t,
            "a wooden cottage ({cottage}) outlasted a stone dike ({t})"
        ),
    }
}

#[test]
fn a_building_the_flood_takes_lets_its_people_go() {
    let (mut w, mut nav) = at_the_impact_day(18);
    let farm = w.place(PlayerId(0), Kind::Farm, 16, 16).unwrap();
    for g in Good::ALL {
        let want = Kind::Farm.cost().get(g);
        if want > 0 {
            w.deliver_to(farm, g, want);
        }
    }
    w.build_at(farm, Kind::Farm.build_ticks());

    let worker = w.citizens[0].id;
    w.apply(
        PlayerId(0),
        &sim::command::Command::Assign { citizens: vec![worker], building: farm },
    )
    .unwrap();
    assert_eq!(w.citizens[worker.0 as usize].workplace, Some(farm));

    let mut ruined = false;
    for _ in 0..SURGE_TICKS + 400 {
        keep_fed(&mut w);
        w.tick(&mut nav, &[]);
        if w.buildings[farm.0 as usize].state == sim::building::BuildState::Rubble {
            ruined = true;
            break;
        }
    }
    assert!(ruined, "a wooden farm in the front of an age-two surge survived it");
    assert_eq!(
        w.citizens[worker.0 as usize].workplace, None,
        "still working at a farm that is not there"
    );
}

#[test]
fn nobody_is_carried_into_a_wall() {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    // A solid building, and a citizen just upstream of it.
    let id = w.place(PlayerId(0), Kind::Granary, 62, 60).unwrap();
    w.deliver_to(id, Good::Wood, Kind::Granary.cost().wood);
    w.build_at(id, Kind::Granary.build_ticks());

    w.citizens[0].pos = V2::cell_centre(60, 60);
    let ground = w.ground_heights();
    for _ in 0..400 {
        for y in 50..72 {
            for x in 50..56 {
                w.water.raise_to(x, y, depth(60));
            }
        }
        w.water.step(&ground, 0);
        w.flood_bodies();
        let (cx, cy) = w.citizens[0].pos.cell();
        assert!(
            w.building_at(cx, cy).map(|b| !b.blocks_movement()).unwrap_or(true),
            "a body was carried inside the granary at ({cx},{cy})"
        );
        assert!(
            (0..MAP_W).contains(&cx) && (0..MAP_H).contains(&cy),
            "a body was carried off the map to ({cx},{cy})"
        );
    }
}

#[test]
fn drowning_takes_time_and_footing_resets_it() {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    w.citizens[0].pos = V2::cell_centre(70, 70);
    let ground = w.ground_heights();

    // Deep enough to be out of your depth, but not for long enough.
    // A block, not one cell: a single deep cell empties into its four
    // neighbours on the very next step and the citizen finds its feet again.
    for _ in 0..DROWN_TICKS - 1 {
        for y in 66..75 {
            for x in 66..75 {
                w.water.raise_to(x, y, SWIM_DEPTH + depth(6));
            }
        }
        w.water.step(&ground, 0);
        w.flood_bodies();
    }
    assert!(w.citizens[0].alive(), "drowned early");
    assert!(w.citizens[0].drowning_for > 0);

    // The water goes down; the clock goes back to nothing.
    for y in 60..80 {
        for x in 60..80 {
            w.water.depth[sim::map::Map::idx(x, y)] = 0;
        }
    }
    w.water.raise_to(0, 0, depth(1)); // keep the flood loop awake
    w.flood_bodies();
    assert_eq!(w.citizens[0].drowning_for, 0, "the clock kept running on dry land");
    assert!(w.citizens[0].alive());
}
