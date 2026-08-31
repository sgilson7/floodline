//! The three properties the plan names for the automaton, and the surge.
//!
//! Most of these run on a deliberately flat, featureless map. Real terrain is
//! the wrong place to learn whether water conserves volume — every failure
//! looks like a hill.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::PlayerId;
use sim::map::{Ground, Map, CELLS, MAP_H, MAP_W};
use sim::nav::Nav;
use sim::water::Water;
use sim::world::World;

/// Rock, everywhere, for every test that drives the automaton directly.
///
/// M12.8 gave the ground an appetite: sand and grass drink, and what they hold
/// they pass down to an aquifer that deletes it. Every test in this file is
/// about the *automaton* - conservation, settling, what a dike holds back -
/// and running them on soil would fold two questions into one number. Rock
/// takes nothing, so these measure exactly what they measured before.
/// `the_ground_drinks_and_the_map_dries` is where the soil is tested.
const NO_SOIL: [sim::map::Ground; sim::map::CELLS] = [sim::map::Ground::Rock; sim::map::CELLS];

/// A world whose map is one flat plain at a given height.
fn flat(height: u8) -> World {
    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = height;
        w.map.ground[i] = Ground::Grass;
    }
    w
}

/// A column of `DIKE_LENGTH` cells, east of the source, with nothing standing
/// in it.
///
/// Picked rather than written down: the hearths move with the map, and once
/// the cities went on the river bank a wall at a fixed x ran straight through
/// one of them. The tests below only need the wall to be somewhere the water
/// has to get past.
fn clear_column(w: &World, from: i32) -> i32 {
    (from..MAP_W - DIKE_LENGTH)
        .find(|&x| {
            (0..MAP_H).all(|y| {
                (0..DIKE_LENGTH).all(|d| w.building_at(x + d, y).is_none())
            })
        })
        .expect("nowhere to put a wall")
}

/// Ground heights for a flat world, as `Water::step` wants them.
fn flat_ground(height: i32) -> Vec<i32> {
    vec![height; CELLS]
}

// ---- 1. volume ------------------------------------------------------------

#[test]
fn volume_is_conserved_except_at_the_edges() {
    let ground = flat_ground(40);
    let mut water = Water::dry();

    let poured = water.raise_to(64, 64, depth(200)) as u64;
    assert_eq!(water.volume(), poured);

    for t in 0..2000 {
        water.step(&ground, &NO_SOIL, 0);
        assert_eq!(
            water.volume() + water.drained,
            poured,
            "tick {t}: {} on the map plus {} drained is not the {poured} poured in",
            water.volume(),
            water.drained
        );
    }
    // Whether any of it reached an edge is beside the point here — the
    // invariant above is. Draining is tested on a slope, where there is
    // somewhere for it to go.
}

#[test]
fn water_drains_off_a_slope() {
    // Deliberately sloped, not flat. Water cannot spread thinner than one
    // sixteenth of a unit, so a column poured on a level plain settles into a
    // disc and stays there rather than creeping to an edge sixty cells away.
    // That is a property of the discretisation rather than a bug, and testing
    // drainage on flat ground would only be testing it.
    // Sixty down to eighteen, and never below zero: the sea outside the map
    // sits at surface zero, so ground below that is under water already and
    // will not drain. The first version of this test sloped to minus three and
    // then complained that the water pooled at the bottom of it.
    let ground: Vec<i32> = (0..CELLS)
        .map(|i| {
            let x = i as i32 % MAP_W;
            60 - x / 3
        })
        .collect();
    // Poured near the edge it is meant to run off. Further up the slope it
    // would thin below one sixteenth long before arriving and simply stop,
    // which says something true about the discretisation and nothing about
    // whether the edges drain.
    let mut water = Water::dry();
    let poured = water.raise_to(118, 64, depth(400)) as u64;

    for _ in 0..6000 {
        water.step(&ground, &NO_SOIL, 0);
        if water.volume() == 0 {
            break;
        }
    }
    assert!(
        water.volume() * 4 < poured,
        "only {} of {poured} ran off a slope",
        poured - water.volume()
    );
    assert_eq!(water.volume() + water.drained, poured, "and none of it vanished");
}

#[test]
fn a_puddle_on_flat_ground_spreads_symmetrically() {
    let ground = flat_ground(40);
    let mut water = Water::dry();
    water.raise_to(64, 64, depth(120));

    for _ in 0..60 {
        water.step(&ground, &NO_SOIL, 0);
    }

    // Mirrored in both axes about the cell it started in, and about both
    // diagonals — a round puddle, not a lopsided one. This is the property
    // that "give the remainder to the last neighbour" quietly destroys.
    for dy in 0..25i32 {
        for dx in 0..25i32 {
            let at = |x: i32, y: i32| water.depth_at(64 + x, 64 + y);
            let d = at(dx, dy);
            assert_eq!(d, at(-dx, dy), "mirrored in x at ({dx},{dy})");
            assert_eq!(d, at(dx, -dy), "mirrored in y at ({dx},{dy})");
            assert_eq!(d, at(dy, dx), "mirrored in the diagonal at ({dx},{dy})");
        }
    }
    assert!(water.depth_at(64, 64) > 0, "the puddle vanished");
    assert!(water.depth_at(68, 64) > 0, "the puddle never spread");
}

#[test]
fn a_puddle_settles_instead_of_sloshing_for_ever() {
    // Two cells that would trade the same water back and forth if the
    // transfer rule ever overshot. The checksum has to settle or the game
    // never stops changing.
    let ground = flat_ground(40);
    let mut water = Water::dry();
    water.raise_to(20, 20, depth(30));

    let mut previous = water.depth.clone();
    let mut settled_at = None;
    for t in 0..3000 {
        water.step(&ground, &NO_SOIL, 0);
        if water.depth == previous {
            settled_at = Some(t);
            break;
        }
        previous = water.depth.clone();
    }
    assert!(settled_at.is_some(), "the water never stopped moving");
}

#[test]
fn water_runs_downhill_and_pools_in_the_low_ground() {
    // A ramp: high in the west, low in the east.
    let ground: Vec<i32> = (0..CELLS)
        .map(|i| {
            let x = i as i32 % MAP_W;
            100 - x / 2
        })
        .collect();
    let mut water = Water::dry();
    water.raise_to(10, 64, depth(60));

    for _ in 0..600 {
        water.step(&ground, &NO_SOIL, 0);
    }
    // It has gone east, downhill, and not west.
    assert_eq!(water.depth_at(5, 64), 0, "water ran uphill");
    let east: u32 = (30..MAP_W).map(|x| water.depth_at(x, 64) as u32).sum();
    let west: u32 = (0..20).map(|x| water.depth_at(x, 64) as u32).sum();
    assert!(east > west, "water pooled at the top of the slope: {west} west, {east} east");
}

// ---- 3. dikes -------------------------------------------------------------

#[test]
fn water_behind_a_level_two_dike_stays_behind_it_for_a_height_twelve_surge() {
    // The plan's own test, and design §5's teaching moment: a dike two levels
    // high stops an age-one flood dead, and the water goes around.
    let mut w = flat(40);
    let mut nav = Nav::new();

    // A wall of dikes across the map from x = 40, two levels high. A dike
    // segment is DIKE_LENGTH cells long, so stacking one per row builds a
    // wall that is DIKE_LENGTH cells thick and MAP_H rows tall. Thickness is
    // not what is under test — height is — but "behind the wall" has to mean
    // past the far side of it and not on top of it.
    let wall_x = clear_column(&w, 40);
    let wall_back = wall_x + DIKE_LENGTH;
    let mut wall = Vec::new();
    for y in 0..MAP_H {
        let id = w.place(PlayerId(0), Kind::Dike, Facing::EastWest, wall_x, y).unwrap();
        w.deliver_to(id, Good::Stone, Kind::Dike.cost().stone);
        w.build_at(id, Kind::Dike.build_ticks());
        w.raise_dike(PlayerId(0), id).unwrap();
        w.deliver_to(id, Good::Stone, Kind::Dike.cost().stone);
        w.build_at(id, Kind::Dike.build_ticks());
        assert_eq!(w.buildings[id.0 as usize].level, 2);
        wall.push(id);
    }

    // Pour an age-one surge in from the west of the wall, for as long as one
    // really lasts.
    let ground = w.ground_heights();
    for x in 0..8 {
        for y in 60..68 {
            w.water.raise_to(x, y, depth(12));
        }
    }
    for _ in 0..SURGE_TICKS {
        for x in 0..8 {
            for y in 60..68 {
                w.water.raise_to(x, y, depth(12));
            }
        }
        w.water.step(&ground, &NO_SOIL, 0);
    }
    for _ in 0..600 {
        w.water.step(&ground, &NO_SOIL, 0);
    }

    let behind: u32 = (wall_back..MAP_W)
        .flat_map(|x| (0..MAP_H).map(move |y| (x, y)))
        .map(|(x, y)| w.water.depth_at(x, y) as u32)
        .sum();
    let front: u32 = (0..wall_x)
        .flat_map(|x| (0..MAP_H).map(move |y| (x, y)))
        .map(|(x, y)| w.water.depth_at(x, y) as u32)
        .sum();

    assert!(front > 0, "no water arrived at the dike at all");
    assert_eq!(behind, 0, "{behind} sixteenths of water got past a level-two dike");
    let _ = &mut nav;
}

#[test]
fn water_spills_over_a_dike_it_is_deeper_than() {
    // The other half of the lesson: a dike is not a wall, it is a height.
    let mut w = flat(40);
    let wall_x = clear_column(&w, 20);
    let wall_back = wall_x + DIKE_LENGTH;
    for y in 0..MAP_H {
        let id = w.place(PlayerId(0), Kind::Dike, Facing::EastWest, wall_x, y).unwrap();
        w.deliver_to(id, Good::Stone, Kind::Dike.cost().stone);
        w.build_at(id, Kind::Dike.build_ticks());
    }
    let ground = w.ground_heights();

    // Far deeper than one level of dike can hold.
    for _ in 0..800 {
        for x in 0..8 {
            for y in 0..MAP_H {
                w.water.raise_to(x, y, depth(60));
            }
        }
        w.water.step(&ground, &NO_SOIL, 0);
    }

    let behind: u32 = (wall_back..MAP_W)
        .flat_map(|x| (0..MAP_H).map(move |y| (x, y)))
        .map(|(x, y)| w.water.depth_at(x, y) as u32)
        .sum();
    assert!(behind > 0, "a one-level dike held back a sixty-deep flood");
}

// ---- the surge ------------------------------------------------------------

/// Flood a generated map from its own low corner and report what it did.
fn flood_a_real_map(seed: u64, height: u16) -> (usize, usize, u64) {
    let mut w = World::new(seed, 2);
    w.disaster.height = height;
    let mut nav = Nav::new();
    while w.day_of_age() < World::IMPACT_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }
    let (mut wet, mut deep, mut peak) = (0usize, 0usize, 0u64);
    for _ in 0..SURGE_TICKS + 400 {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
        wet = wet.max(w.water.wet_cells());
        deep = deep.max(w.water.depth.iter().filter(|&&d| d >= depth(2)).count());
        peak = peak.max(w.water.volume());
    }
    (wet, deep, peak)
}

#[test]
fn the_surge_takes_the_low_country() {
    // The plan asks that "the front reaches the map centre within N ticks",
    // and the design's own physics will not do that. Water finds its level: a
    // surge twelve deep cannot climb ground sixty higher, and design §5 needs
    // it not to — "anyone who reaches high ground or a rooftop survives" only
    // means something if high ground stays dry. What the surge must do is take
    // the low country, and that is what is asserted.
    for seed in 0..5u64 {
        let (wet, deep, peak) = flood_a_real_map(seed, 12);
        assert!(
            wet * 100 / CELLS >= 10,
            "seed {seed}: an age-one flood covered only {}% of the map",
            wet * 100 / CELLS
        );
        assert!(deep > 1_000, "seed {seed}: only {deep} cells got properly deep");
        assert!(peak > 100_000, "seed {seed}: peak volume of only {peak}");
    }
}

#[test]
fn a_bigger_surge_is_a_bigger_flood() {
    // Design §4's escalation table has to mean something, and at first it did
    // not: the source pumped a fixed amount inland whatever the age's height,
    // so a surge of twelve and a surge of twenty-four flooded identically. The
    // pump scales with the height now.
    let (_, deep_1, peak_1) = flood_a_real_map(3, 12);
    let (_, _, peak_2) = flood_a_real_map(3, 18);
    let (_, deep_4, peak_4) = flood_a_real_map(3, 24);

    assert!(peak_1 < peak_2 && peak_2 < peak_4, "{peak_1} {peak_2} {peak_4} do not escalate");
    assert!(deep_1 < deep_4, "an age-four flood was no deeper than an age-one");
    assert!(peak_4 > peak_1 * 2, "doubling the height barely changed the flood");
}

#[test]
fn the_high_corner_stays_dry() {
    // The first counter-play design §5 offers is "build on the high corner".
    // If the flood reaches it, that advice is a lie.
    for seed in 0..5u64 {
        let mut w = World::new(seed, 2);
        let mut nav = Nav::new();
        while w.day_of_age() < World::IMPACT_DAY {
            for c in &mut w.citizens {
                c.food = NEED_FULL;
            }
            w.tick(&mut nav, &[]);
        }
        for _ in 0..SURGE_TICKS + 400 {
            for c in &mut w.citizens {
                c.food = NEED_FULL;
            }
            w.tick(&mut nav, &[]);
        }
        let (hx, hy) = w.map.high_corner.cell();
        let (sx, sy) = (hx.min(MAP_W - 12), hy.min(MAP_H - 12));
        let wet: u32 = (sx..sx + 12)
            .flat_map(|x| (sy..sy + 12).map(move |y| (x, y)))
            .map(|(x, y)| w.water.depth_at(x, y) as u32)
            .sum();
        assert_eq!(wet, 0, "seed {seed}: the flood reached the high corner");
    }
}

#[test]
fn the_surge_pours_only_on_the_impact_day_and_only_for_its_time() {
    let mut w = flat(40);
    let mut nav = Nav::new();
    w.disaster.sources = vec![0];
    w.disaster.height = 12;

    for _ in 0..TICKS_PER_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }
    assert_eq!(w.water.volume(), 0, "it rained on day one");

    while w.day_of_age() < World::IMPACT_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }

    for _ in 0..SURGE_TICKS {
        w.tick(&mut nav, &[]);
    }
    let at_peak = w.water.volume();
    assert!(at_peak > 0, "the surge poured nothing");
    assert_eq!(w.surging(), 0, "the surge is still pouring after its time");

    // Design §5 step 6: once the source stops, the water drains away over the
    // rest of the day. The sea falls back with it, which is what lets the
    // edges take water again.
    for _ in 0..500 {
        w.tick(&mut nav, &[]);
    }
    assert!(
        w.water.volume() < at_peak,
        "the flood never started to go down: {at_peak} then {}",
        w.water.volume()
    );
    assert!(w.water.drained > 0, "nothing ran off the edges once the sea fell");
}

#[test]
fn the_flood_is_the_same_flood_on_two_peers() {
    // The automaton is sixteen thousand cells of integer arithmetic run every
    // tick. If any of it were order-dependent this is where it would show.
    let build = |seed: u64| {
        let mut w = flat(40);
        w.seed = seed;
        w.disaster.sources = vec![0];
        w.disaster.height = depth(18);
        w
    };
    let run = |seed: u64| -> Vec<u64> {
        let mut w = build(seed);
        let mut nav = Nav::new();
        let mut marks = Vec::new();
        while w.day_of_age() < World::IMPACT_DAY {
            for c in &mut w.citizens {
                c.food = NEED_FULL;
            }
            w.tick(&mut nav, &[]);
        }
        for t in 0..800 {
            for c in &mut w.citizens {
                c.food = NEED_FULL;
            }
            w.tick(&mut nav, &[]);
            if t % 100 == 0 {
                marks.push(w.water.volume());
            }
        }
        marks.push(w.checksum());
        marks
    };
    assert_eq!(run(5), run(5));
}

#[test]
fn dry_ground_costs_nothing() {
    // The automaton must not run at all when there is no water, or every tick
    // of every game pays for a flood that is not happening.
    let mut w = flat(40);
    let mut nav = Nav::new();
    for _ in 0..50 {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(w.water.volume(), 0);
    assert!(w.water.depth.iter().all(|&d| d == 0));
    let _ = Map::idx(0, 0);
}

#[test]
fn the_ground_drinks_and_the_map_dries() {
    // M12.8. City 1 spent age 3 days 1 and 2 reading "all quiet" while its
    // farm still read `wading`, and lost two souls to standing water on
    // nominally quiet days. The only way water left the map was over an edge,
    // so a hollow that filled stayed filled until the run ended - and the map
    // was blue for most of two ages, which is what made the high-water mark
    // unreadable in the one window it exists for.
    //
    // **What this arranges**: a flat grass map with a hollow in it, filled by
    // hand rather than by a surge. The question is what the *ground* does with
    // standing water, and a surge would fold in the whole automaton.
    use sim::balance::{depth, DAMP, TICKS_PER_DAY, WADE_DEPTH};
    use sim::map::{Ground, CELLS, MAP_H, MAP_W};
    use sim::world::World;

    let mut w = World::new(31, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    // A basin, so the water has nowhere to run off to and only the ground can
    // take it. Without this the map drains over its edges and this test would
    // be measuring the automaton again.
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let edge = x < 8 || y < 8 || x >= MAP_W - 8 || y >= MAP_H - 8;
            if edge {
                w.map.height[sim::Map::idx(x, y)] = 90;
            }
        }
    }
    let (px, py) = (60, 60);
    for y in py..py + 8 {
        for x in px..px + 8 {
            w.water.raise_to(x, y, WADE_DEPTH);
        }
    }
    let start = w.water.volume();
    assert!(start > 0);
    assert!(w.water.depth_at(px, py) >= WADE_DEPTH, "the pool was not poured");

    // One day.
    for _ in 0..TICKS_PER_DAY {
        w.step_water();
    }

    let left = w.water.depth_at(px, py);
    println!(
        "  a farm under wading water, one day later: {left} (damp is {DAMP}, wading is {WADE_DEPTH})"
    );
    assert!(
        left < WADE_DEPTH,
        "a day of standing water and the farm is still wading at {left}"
    );
    assert!(
        left <= DAMP,
        "it drained to {left}, and anything above {DAMP} the map still paints blue"
    );

    // And none of it vanished into arithmetic: what is on the surface, plus
    // what the ground holds, plus what has gone to the aquifer and over the
    // edges, is what was poured.
    assert_eq!(
        w.water.accounted(),
        start,
        "water went missing: {} on the surface, {} in the ground, {} gone",
        w.water.volume(),
        w.water.held_by_ground(),
        w.water.drained
    );

    // The ground will not drink water deep enough to swim in, which is what
    // keeps a dike under load: a wall is held up by the pool against it, and
    // ground that drank that pool would relieve the wall. `SOAK_CEILING` and
    // the table in `balance::SOAK_EVERY` are the measurement; this is the rule
    // itself, on one cell that cannot spread.
    let mut deep = World::new(31, 2);
    for i in 0..CELLS {
        deep.map.height[i] = 40;
        deep.map.ground[i] = Ground::Grass;
    }
    let (dx, dy) = (60, 60);
    for y in dy - 1..=dy + 1 {
        for x in dx - 1..=dx + 1 {
            if (x, y) != (dx, dy) {
                deep.map.height[sim::Map::idx(x, y)] = 90;
            }
        }
    }
    deep.water.raise_to(dx, dy, depth(10));
    let before = deep.water.depth_at(dx, dy);
    assert!(before > WADE_DEPTH);
    for _ in 0..TICKS_PER_DAY {
        deep.step_water();
    }
    let after = deep.water.depth_at(dx, dy);
    println!("  a cell ten deep with nowhere to go, one day later: {before} -> {after}");
    assert_eq!(
        after, before,
        "the ground drank water it is not allowed to reach"
    );
}
