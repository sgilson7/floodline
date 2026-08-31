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
use sim::map::{Ground, CELLS, MAP_H};
use sim::nav::Nav;
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

// ---- pressure ---------------------------------------------------------------

/// Build a wall down the column at `wall_x`, `levels` high, and return its ids.
fn wall(w: &mut World, wall_x: u8, levels: u8) -> Vec<BuildingId> {
    let ids = w.lay_dike_line(ME, (wall_x, 0), (wall_x, (MAP_H - 1) as u8)).unwrap();
    for &id in &ids {
        for _ in 0..levels {
            let owed = w.buildings[id.0 as usize].outstanding().stone;
            w.deliver_to(id, Good::Stone, owed);
            w.build_at(id, w.buildings[id.0 as usize].kind.build_ticks() * levels as u32);
            if w.buildings[id.0 as usize].level < levels {
                w.raise_dike(ME, id).unwrap();
            }
        }
        assert_eq!(w.buildings[id.0 as usize].level, levels);
        assert!(w.buildings[id.0 as usize].standing_now());
    }
    ids
}

/// Hold an age-one surge against the west edge for `ticks`, the way
/// `inject_surge` does, and let the flood act on the world each tick.
fn pour(w: &mut World, height: u16, ticks: u32) {
    let mut nav = Nav::new();
    for _ in 0..ticks {
        for x in 0..SURGE_SIZE {
            for y in 0..MAP_H {
                w.water.raise_to(x, y, depth(height));
            }
        }
        w.step_water();
        w.flood_bodies();
        let _ = &mut nav;
    }
}

/// Let the water drain and the flood act, with nothing poured in.
fn drain(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.step_water();
        w.flood_bodies();
    }
}

#[test]
fn a_level_one_wall_gives_way_where_a_level_two_holds() {
    // The plan's definition of done for M3, and the reason a dike has a
    // pressure model at all: height has to buy something the player can watch.
    for (level, expect_break) in [(1u8, true), (2u8, false)] {
        let mut w = flat();
        let ids = wall(&mut w, 40, level);
        // The segment in the middle of the wall, which is the one the surge
        // reaches squarest. How much of the rest goes with it is a question
        // about a whole map, and M5's to answer.
        let watch = ids[ids.len() / 2];
        pour(&mut w, 12, SURGE_TICKS);
        // Long enough for a level one to go. `dike_pressure_on_flat_ground`
        // says it does so around tick 1900 at the thresholds M5 measured — it
        // was 1100 at the provisional ones, and this window was 1300.
        //
        // 2 800 since M12.8, and the reason is worth having. Ground that
        // drinks does not change *whether* a level-one wall goes - the peak
        // stress is identical to the world before drainage, 12 751 either way,
        // which is the number that milestone was measured against. What it
        // changes is *when*: the apron of shallow water around the pool drains
        // away and stops feeding it, so the wall takes longer to reach the
        // same load. A level two still holds for the whole of the longer
        // window, which is what keeps this test a comparison rather than a
        // stopwatch.
        drain(&mut w, 2800);

        let standing =
            ids.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();
        let strain = w.buildings[watch.0 as usize].strain();
        if expect_break {
            assert!(
                !w.buildings[watch.0 as usize].standing_now(),
                "a level-{level} segment held an age-one surge at {strain}% strain"
            );
            assert!(standing < ids.len());
        } else {
            assert_eq!(
                standing,
                ids.len(),
                "a level-{level} wall lost segments to an age-one surge"
            );
            assert!(strain > 0, "a level-{level} wall was not even leaned on");
        }
    }
}

#[test]
fn a_wall_under_load_shows_it_before_it_goes() {
    // Without this the failure is arbitrary as far as a player is concerned.
    // `strain` is what the renderer darkens by, so it has to climb rather than
    // jump from nothing to rubble.
    let mut w = flat();
    let ids = wall(&mut w, 40, 1);
    let watch = ids[ids.len() / 2];
    let mut seen = Vec::new();

    for _ in 0..26 {
        pour(&mut w, 12, 50);
        drain(&mut w, 2800);
        seen.push(w.buildings[watch.0 as usize].strain());
        if !w.buildings[watch.0 as usize].standing_now() {
            break;
        }
    }

    assert!(!w.buildings[watch.0 as usize].standing_now(), "the wall never gave way: {seen:?}");
    assert!(
        seen.iter().any(|&s| (20..80).contains(&s)),
        "the wall went from sound to rubble with nothing in between: {seen:?}"
    );
    assert!(seen.windows(2).all(|p| p[0] <= p[1]), "strain went down under load: {seen:?}");
}

#[test]
fn a_wall_sheds_its_stress_once_the_water_is_gone() {
    let mut w = flat();
    let ids = wall(&mut w, 40, 2);
    let id = ids[ids.len() / 2];
    pour(&mut w, 12, SURGE_TICKS);
    drain(&mut w, 600);
    let loaded = w.buildings[id.0 as usize].stress;
    assert!(loaded > 0, "the surge did not lean on the wall at all");

    w.water = sim::water::Water::dry();
    drain(&mut w, 200);
    let after = w.buildings[id.0 as usize].stress;
    assert!(after < loaded, "stress did not bleed off: {loaded} then {after}");
    assert_eq!(after, loaded.saturating_sub(200 * STRESS_RELIEF));

    drain(&mut w, loaded / STRESS_RELIEF + 10);
    assert_eq!(w.buildings[id.0 as usize].stress, 0, "and it does reach nothing");
}

#[test]
fn a_wall_that_gives_way_stops_holding_the_water_up() {
    let mut w = flat();
    let ids = wall(&mut w, 40, 1);
    let id = ids[ids.len() / 2];
    let (bx, by) = (w.buildings[id.0 as usize].x as i32, w.buildings[id.0 as usize].y as i32);
    let held = w.effective_height(bx, by);

    pour(&mut w, 12, SURGE_TICKS);
    // 2 800 for the same reason as
    // `a_level_one_wall_gives_way_where_a_level_two_holds`: ground that drinks
    // moves *when* a level-one wall goes, not whether.
    drain(&mut w, 2800);

    assert!(!w.buildings[id.0 as usize].standing_now());
    assert!(
        w.effective_height(bx, by) < held,
        "rubble is still holding the water back"
    );
}

#[test]
fn only_a_dike_is_pressed() {
    // One model for a wall and not two: `batter_buildings` takes the flow over
    // a footprint, `press_dikes` takes the lean on a side, and a building is
    // in exactly one of them.
    let mut w = flat();
    let ids = wall(&mut w, 40, 1);
    let dike = ids[ids.len() / 2];
    let hut = w.place(ME, Kind::Cottage, Facing::EastWest, 60, 60).unwrap();
    w.deliver_to(hut, Good::Wood, Kind::Cottage.cost().wood);
    w.build_at(hut, Kind::Cottage.build_ticks());

    pour(&mut w, 12, SURGE_TICKS);
    drain(&mut w, 300);
    assert!(w.buildings[dike.0 as usize].stress > 0);
    assert_eq!(w.buildings[hut.0 as usize].stress, 0, "a cottage does not accumulate stress");
    assert_eq!(
        w.buildings[dike.0 as usize].integrity,
        Kind::Dike.integrity(),
        "a dike is pressed, not battered: its integrity is untouched"
    );
}

#[test]
fn the_wet_side_is_whichever_side_is_wet() {
    // A wall does not know which way round it was built, so the model asks
    // the water. Turning the world round has to give the same answer.
    let mut w = flat();
    let id = w.lay_dike_line(ME, (40, 64), (40, 64)).unwrap()[0];
    let b = &w.buildings[id.0 as usize];
    let [near, far] = b.sides();
    assert_eq!(near, vec![(40, 63), (41, 63), (42, 63)]);
    assert_eq!(far, vec![(40, 65), (41, 65), (42, 65)]);

    let id = w.lay_dike_line(ME, (60, 20), (60, 22)).unwrap()[0];
    let b = &w.buildings[id.0 as usize];
    let [near, far] = b.sides();
    assert_eq!(near, vec![(59, 20), (59, 21), (59, 22)]);
    assert_eq!(far, vec![(61, 20), (61, 21), (61, 22)]);
}

/// What the water actually does to a wall, on flat ground with nothing else in
/// the way. A measurement, not an assertion: the numbers in
/// `balance::DIKE_STRESS_LIMIT` come from here until M5 re-derives them
/// against the river.
#[test]
#[ignore]
fn dike_pressure_on_flat_ground() {
    for height in [12u16, 20] {
        println!();
        println!("  a surge of {height} for {SURGE_TICKS} ticks, then left to drain");
        println!("   level   peak stress   at tick   broke   standing at the end   sheds in");

        for level in 1..=DIKE_MAX_LEVEL {
            let mut w = flat();
            let ids = wall(&mut w, 40, level);
            let watch = ids[ids.len() / 2];

            let mut peak = 0;
            let mut peak_at = 0;
            let mut broke = None;
            for t in 1..=(SURGE_TICKS + 3 * TICKS_PER_DAY) {
                if t <= SURGE_TICKS {
                    pour(&mut w, height, 1);
                } else {
                    w.step_water();
                    w.flood_bodies();
                }
                let b = &w.buildings[watch.0 as usize];
                if b.stress > peak {
                    peak = b.stress;
                    peak_at = t;
                }
                if broke.is_none() && !b.standing_now() {
                    broke = Some(t);
                }
            }
            let standing =
                ids.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();

            // And how long an intact one takes to shed a full load.
            let mut dry = flat();
            let one = wall(&mut dry, 40, level)[0];
            dry.buildings[one.0 as usize].stress = dike_stress_limit(level) - 1;
            let mut clear = 0;
            while dry.buildings[one.0 as usize].stress > 0 && clear < 10 * TICKS_PER_DAY {
                dry.flood_bodies();
                clear += 1;
            }

            println!(
                "   {level:>5}   {peak:>11}   {peak_at:>7}   {:>5}   {standing:>19}   {} ticks",
                match broke {
                    Some(t) => format!("t{t}"),
                    None => "no".to_owned(),
                },
                clear,
            );
        }
    }
}

// ---- M5: which dikes break --------------------------------------------------

/// Wall one bank of the river, `out` cells from the centreline, in segments
/// alternating between level 1 and level 2.
///
/// Built by fiat rather than hauled and paid for. Whether a city can *afford*
/// a wall is `playtest.rs`'s question and depends on eight people and a day's
/// work; this one is only about whether the water takes it down, and mixing
/// the two questions is how a threshold ends up tuned against a shortage of
/// stone.
///
/// Returns one entry per segment: its id, its level, and how far it stands
/// from the channel.
fn wall_the_bank(w: &mut World, out: i32, side: i32) -> Vec<(BuildingId, u8, i32)> {
    let river: Vec<(i32, i32)> =
        w.map.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
    let n = river.len() as i32;
    let mut built = Vec::new();

    let mut at = 0;
    while at < n {
        let (rx, ry) = river[at as usize];
        let a = river[(at - 4).clamp(0, n - 1) as usize];
        let b = river[(at + 4).clamp(0, n - 1) as usize];
        let (tx, ty) = (b.0 - a.0, b.1 - a.1);
        let (px, py) = ((-ty).signum() * side, tx.signum() * side);
        let (x, y) = (rx + px * out, ry + py * out);

        // Alternating, so both levels meet the same water in the same run and
        // a difference between them cannot be a difference between two places.
        let level = if built.len() % 2 == 0 { 1u8 } else { 2u8 };
        let facing = Facing::of_run((0, 0), (tx, ty));

        if w.can_place(ME, Kind::Dike, facing, x, y).is_ok() {
            let id = w.place(ME, Kind::Dike, facing, x, y).unwrap();
            for _ in 0..level {
                let owed = w.buildings[id.0 as usize].outstanding().stone;
                w.deliver_to(id, Good::Stone, owed);
                w.build_at(id, Kind::Dike.build_ticks());
                if w.buildings[id.0 as usize].level < level {
                    w.raise_dike(ME, id).unwrap();
                }
            }
            if w.buildings[id.0 as usize].standing_now() {
                built.push((id, level, out));
            }
        }
        at += DIKE_LENGTH;
    }
    built
}

/// Run the impact day and let the water drain, without anybody in the way.
///
/// `World::tick` is not used: a citizen in a flood re-paths every tick against
/// an effective height the water is changing under it, and this is a question
/// about walls.
fn flood_it(w: &mut World, height: u16) {
    w.disaster.height = height;
    w.tick = (World::IMPACT_DAY - 1) * TICKS_PER_DAY;
    for _ in 0..(SURGE_TICKS + 2 * TICKS_PER_DAY) {
        w.step_water();
        w.flood_bodies();
        w.tick += 1;
    }
}

/// M5's measurement: how much of a wall along the river the flood takes.
///
/// The plan's target is a fraction, not a rule — "a lot of level one dikes
/// break and not all of them; many level twos hold" — so it has to be counted
/// across seeds and ages rather than asserted on one. `DIKE_STRESS_LIMIT` is
/// set from what this prints.
#[test]
#[ignore]
fn which_dikes_break() {
    const SEEDS: [u64; 10] = [
        31, 97, 1_000_003, 4_043_362_590, 12_345, 3, 7, 88_888, 555, 20_260_830,
    ];
    const OUT: [i32; 3] = [6, 12, 20];

    for height in [12u16, 18] {
        println!();
        println!("  a surge of {height} (age {})", if height == 12 { "1-2" } else { "3" });
        println!("  from the channel   level 1 broken   level 2 broken   segments");

        let mut totals = [(0usize, 0usize), (0usize, 0usize)];
        let mut per_seed: Vec<(u64, [(usize, usize); 2])> =
            SEEDS.iter().map(|&s| (s, [(0, 0), (0, 0)])).collect();
        for out in OUT {
            let mut counts = [(0usize, 0usize), (0usize, 0usize)];
            for (si, seed) in SEEDS.into_iter().enumerate() {
                let mut w = World::new(seed, 2);
                let mut built = wall_the_bank(&mut w, out, 1);
                built.extend(wall_the_bank(&mut w, out, -1));
                flood_it(&mut w, height);

                for (id, level, _) in built {
                    let gone = !w.buildings[id.0 as usize].standing_now();
                    let slot = &mut counts[level as usize - 1];
                    slot.1 += 1;
                    if gone {
                        slot.0 += 1;
                    }
                    let seat = &mut per_seed[si].1[level as usize - 1];
                    seat.1 += 1;
                    if gone {
                        seat.0 += 1;
                    }
                }
            }
            let pct = |(broken, all): (usize, usize)| {
                if all == 0 { 0 } else { broken * 100 / all }
            };
            println!(
                "  {out:>13} cells   {:>13}%   {:>13}%   {:>8}",
                pct(counts[0]),
                pct(counts[1]),
                counts[0].1 + counts[1].1
            );
            for i in 0..2 {
                totals[i].0 += counts[i].0;
                totals[i].1 += counts[i].1;
            }
        }
        let pct = |(broken, all): (usize, usize)| {
            if all == 0 { 0 } else { broken * 100 / all }
        };
        println!(
            "  all distances       {:>13}%   {:>13}%   {:>8}",
            pct(totals[0]),
            pct(totals[1]),
            totals[0].1 + totals[1].1
        );
        println!(
            "  the target is 60-80% of level one gone and 70-90% of level two standing"
        );
        if height == 12 {
            println!("  by seed:");
            let mut hit = 0;
            for (seed, counts) in &per_seed {
                let one = pct(counts[0]);
                let two = 100 - pct(counts[1]);
                let ok = (60..=80).contains(&one) && (70..=90).contains(&two);
                hit += usize::from(ok);
                println!(
                    "    {seed:>12}   level 1 {one:>3}% gone   level 2 {two:>3}% standing   {}",
                    if ok { "on target" } else { "off" }
                );
            }
            println!("  {hit} of {} seeds on target", SEEDS.len());
        }
    }
}

#[test]
fn a_course_still_going_up_does_not_say_it_was_never_started() {
    // M12.7, fault 8. An M11.9 player watched a segment read `level 1 of 4`,
    // clicked it two minutes later to raise it again, and was told **"it is
    // not built yet"** - about a wall they had watched go up. They spent the
    // rest of the run believing the interaction was broken.
    //
    // Reproduced, and the message was the thing that was wrong rather than the
    // rule. Raising adds the level *at once* and puts the segment back to a
    // site, so between the raise and the last builder-tick the segment really
    // is not standing - but it is not unbuilt either, and the two want
    // different sentences.
    use sim::building::{BuildState, Facing, Good, Kind};
    use sim::command::Command;
    use sim::world::{RuleError, World};
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let (hx, hy) = w.map.hearth_sites[0];

    let mut dike = None;
    'ring: for r in 4..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Dike, Facing::EastWest, x, y).is_ok() {
                    w.apply(
                        me,
                        &Command::Place {
                            kind: Kind::Dike,
                            facing: Facing::EastWest,
                            x: x as u8,
                            y: y as u8,
                        },
                    )
                    .unwrap();
                    dike = Some(w.buildings.last().unwrap().id);
                    break 'ring;
                }
            }
        }
    }
    let dike = dike.expect("nowhere for a dike on seed 31");

    // A site nobody has started really is not built yet, and says so.
    assert_eq!(w.raise_dike(me, dike), Err(RuleError::NotStanding));

    for g in Good::ALL {
        let want = w.buildings[dike.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(dike, g, want);
        }
    }
    assert!(w.build_at(dike, Kind::Dike.build_ticks()));
    assert_eq!(w.buildings[dike.0 as usize].level, 1);

    // Raised: level 2 at once, and back to a site while the course goes on.
    w.raise_dike(me, dike).unwrap();
    assert_eq!(w.buildings[dike.0 as usize].level, 2);
    assert_eq!(w.buildings[dike.0 as usize].state, BuildState::Site);

    // Clicking it again is refused - correctly - and must not borrow the
    // sentence a never-started segment uses.
    assert_eq!(
        w.raise_dike(me, dike),
        Err(RuleError::StillRising),
        "a course part-way up said it had never been started"
    );
    assert_eq!(RuleError::StillRising.to_message(), "that course is still going up");
    assert_ne!(
        RuleError::StillRising.to_message(),
        RuleError::NotStanding.to_message(),
        "the two states a player has to tell apart still read the same"
    );
}
