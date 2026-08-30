//! The plan's definition of done for phase 1.
//!
//! > a scripted two-player game (`cargo test -p sim --test scenario`) founds
//! > two cities, builds a road, trades food for wood for three days, and the
//! > determinism test passes.
//!
//! Everything here goes through `World::apply`, because that is the point:
//! this is the game a browser and a bot will play at each other in phase 4,
//! and if it can be driven by a list of commands it can be driven over a wire.

use sim::balance::*;
use sim::building::{Good, Kind};
use sim::citizen::{CitizenId, PlayerId};
use sim::command::Command;
use sim::nav::Nav;
use sim::road::RoadId;
use sim::world::World;
use sim::BuildingId;

const SEED: u64 = 31;

fn spot(w: &World, p: u8, kind: Kind) -> (i32, i32) {
    let (hx, hy) = w.map.hearth_sites[p as usize];
    for r in 3..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if w.can_place(PlayerId(p), kind, hx + dx, hy + dy).is_ok() {
                    return (hx + dx, hy + dy);
                }
            }
        }
    }
    panic!("nowhere for a {kind:?} in city {p}");
}

/// Place through the command door, then finish it outright — the days of
/// hauling and building that would precede this are item 4's and item 6's
/// tests, not this one's.
fn found(w: &mut World, p: u8, kind: Kind) -> BuildingId {
    let (x, y) = spot(w, p, kind);
    w.apply(PlayerId(p), &Command::Place { kind, x: x as u8, y: y as u8 }).unwrap();
    let id = w.buildings.last().unwrap().id;
    for g in Good::ALL {
        let want = kind.cost().get(g);
        if want > 0 {
            w.deliver_to(id, g, want);
        }
    }
    assert!(w.build_at(id, kind.build_ticks()), "{kind:?} did not finish");
    id
}

/// Finish every road and bridge site outright, as the ordering city's builders
/// would over the next few days.
fn open_the_road(w: &mut World) {
    let sites: Vec<BuildingId> = w
        .buildings
        .iter()
        .filter(|b| matches!(b.kind, Kind::Road | Kind::Bridge) && !b.standing_now())
        .map(|b| b.id)
        .collect();
    for id in sites {
        let kind = w.buildings[id.0 as usize].kind;
        for g in Good::ALL {
            let want = w.buildings[id.0 as usize].outstanding().get(g);
            if want > 0 {
                w.deliver_to(id, g, want);
            }
        }
        assert!(w.build_at(id, kind.build_ticks()), "a road cell did not finish");
    }
}

/// Two cities, each with a farm, a granary and cottages, farmers at work.
fn two_cities() -> (World, Nav) {
    let mut w = World::new(SEED, 2);
    for p in 0..2u8 {
        let farm = found(&mut w, p, Kind::Farm);
        found(&mut w, p, Kind::Granary);
        found(&mut w, p, Kind::Cottage);
        found(&mut w, p, Kind::Cottage);

        let mine: Vec<CitizenId> = w
            .citizens
            .iter()
            .filter(|c| c.owner == PlayerId(p))
            .map(|c| c.id)
            .take(Kind::Farm.job_slots())
            .collect();
        w.apply(PlayerId(p), &Command::Assign { citizens: mine, building: farm })
            .unwrap();
    }
    (w, Nav::new())
}

/// Lay a road from city `a` to city `b` and have `b` accept it.
fn join_the_cities(w: &mut World, a: u8, b: u8) -> RoadId {
    let (ax, ay) = w.map.hearth_sites[a as usize];
    let (bx, by) = w.map.hearth_sites[b as usize];

    // Aim beside the far hearth rather than at it: the hearth itself is a
    // building, and a road does not go through one.
    let target = (0..8)
        .flat_map(|r| {
            [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r)].into_iter()
        })
        .map(|(dx, dy)| (bx + dx, by + dy))
        .find(|&(x, y)| w.building_at(x, y).is_none() && w.map.buildable(x, y))
        .expect("nowhere beside the far hearth to aim at");

    let start = (0..8)
        .flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r)].into_iter())
        .map(|(dx, dy)| (ax + dx, ay + dy))
        .find(|&(x, y)| w.building_at(x, y).is_none() && w.map.buildable(x, y))
        .expect("nowhere beside the near hearth to start from");

    w.apply(
        PlayerId(a),
        &Command::Road {
            from: (start.0 as u8, start.1 as u8),
            to: (target.0 as u8, target.1 as u8),
        },
    )
    .unwrap();

    let road = w.roads.last().unwrap().id;
    assert_eq!(
        w.roads[road.0 as usize].reaches,
        Some(PlayerId(b)),
        "the road did not reach the other city"
    );
    open_the_road(w);
    w.apply(PlayerId(b), &Command::AcceptRoad { road }).unwrap();
    road
}

#[test]
fn two_cities_found_a_road_and_trade_for_three_days() {
    let (mut w, mut nav) = two_cities();
    let road = join_the_cities(&mut w, 0, 1);

    assert!(w.roads[road.0 as usize].joined, "the road was never joined");
    assert!(w.linked(PlayerId(0), PlayerId(1)), "the cities are not linked");
    assert!(w.linked(PlayerId(1), PlayerId(0)), "and a link goes both ways");

    // Let the farms fill the granaries first, or there is nothing to trade.
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }
    assert!(
        w.treasury(PlayerId(0)).food > 0,
        "city 0 has no food to offer: {:?}",
        w.treasury(PlayerId(0))
    );

    // Food one way, wood the other — the plan's own example.
    w.apply(
        PlayerId(0),
        &Command::Trade {
            with: PlayerId(1),
            give: (Good::Food, 20),
            take: (Good::Wood, 20),
        },
    )
    .unwrap();
    let trade = w.trades.last().unwrap().id;
    w.apply(PlayerId(1), &Command::AcceptTrade { trade }).unwrap();
    assert!(w.trades[trade.0 as usize].accepted);

    let before_0 = w.treasury(PlayerId(0));
    let before_1 = w.treasury(PlayerId(1));

    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }

    let after_0 = w.treasury(PlayerId(0));
    let after_1 = w.treasury(PlayerId(1));

    // City 0 gave food and got wood; city 1 the other way round. Wood is the
    // clean signal: neither city produces any, so every unit that moved got
    // there on somebody's back.
    assert!(
        after_0.wood > before_0.wood,
        "city 0 got no wood: {before_0:?} -> {after_0:?}"
    );
    assert!(
        after_1.wood < before_1.wood,
        "city 1 gave up no wood: {before_1:?} -> {after_1:?}"
    );
    assert!(
        after_1.food > 0,
        "city 1 received no food at all: {before_1:?} -> {after_1:?}"
    );

    // And both cities came through, which is the whole point of trading.
    //
    // "Came through" and not "lost nobody", which is what this said until the
    // map generator moved the hearth sites onto a line at a fixed distance
    // from the corner the water comes out of (see `balance::SHORE_DISTANCE`).
    // Founding, joining and three days of trade take the world past the
    // age-one impact day, so this scenario now runs through a flood that it
    // used to sit comfortably clear of, and one of city 1's haulers drowned
    // out on the shore. That is the flood working: nobody built a dike here
    // and nobody was told to go uphill. Losing more than a couple would mean
    // something else.
    for p in [PlayerId(0), PlayerId(1)] {
        let alive = w.population(p);
        assert!(
            alive + 2 >= FOUNDING_CITIZENS,
            "city {} came out of the flood with {alive} of {FOUNDING_CITIZENS}",
            p.0
        );
    }
}

#[test]
fn a_trade_nobody_accepted_moves_nothing() {
    let (mut w, mut nav) = two_cities();
    join_the_cities(&mut w, 0, 1);
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }

    w.apply(
        PlayerId(0),
        &Command::Trade { with: PlayerId(1), give: (Good::Food, 20), take: (Good::Wood, 20) },
    )
    .unwrap();

    let before = w.treasury(PlayerId(0)).wood;
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(
        w.treasury(PlayerId(0)).wood,
        before,
        "a proposal nobody agreed to moved goods anyway"
    );
}

#[test]
fn a_trade_over_a_road_nobody_joined_moves_nothing() {
    let (mut w, mut nav) = two_cities();

    // Lay the road and build it, but never accept it.
    let (ax, ay) = w.map.hearth_sites[0];
    let (bx, by) = w.map.hearth_sites[1];
    let start = (ax + 2, ay);
    let target = (bx + 2, by);
    w.apply(
        PlayerId(0),
        &Command::Road {
            from: (start.0 as u8, start.1 as u8),
            to: (target.0 as u8, target.1 as u8),
        },
    )
    .unwrap();
    open_the_road(&mut w);
    assert!(!w.linked(PlayerId(0), PlayerId(1)), "an unjoined road linked the cities");

    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }
    w.apply(
        PlayerId(0),
        &Command::Trade { with: PlayerId(1), give: (Good::Food, 20), take: (Good::Wood, 20) },
    )
    .unwrap();
    let trade = w.trades.last().unwrap().id;
    w.apply(PlayerId(1), &Command::AcceptTrade { trade }).unwrap();

    let before = w.treasury(PlayerId(0)).wood;
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(
        w.treasury(PlayerId(0)).wood,
        before,
        "goods moved along a road that was never joined"
    );
}

/// Design §6: the flood breaks road cells, "which is what makes rebuilding the
/// link after an age a decision".
#[test]
fn breaking_one_cell_of_the_road_stops_the_trade() {
    let (mut w, mut nav) = two_cities();
    let road = join_the_cities(&mut w, 0, 1);
    assert!(w.linked(PlayerId(0), PlayerId(1)));

    // One cell somewhere in the middle, as a surge would take it.
    let cells = w.roads[road.0 as usize].cells.clone();
    let (bx, by) = cells[cells.len() / 2];
    let broken = w.building_at(bx as i32, by as i32).unwrap().id;
    w.damage_building(broken, Kind::Road.integrity());

    assert!(
        !w.linked(PlayerId(0), PlayerId(1)),
        "one broken cell and the road still counts as a link"
    );
    assert!(w.roads[road.0 as usize].joined, "but the agreement itself survives");

    for _ in 0..TICKS_PER_DAY * 2 {
        w.tick(&mut nav, &[]);
    }

    // Rebuild that cell and the link is back, without anybody having to agree
    // to anything again.
    w.apply(PlayerId(0), &Command::Demolish { building: broken }).unwrap();
    w.apply(PlayerId(0), &Command::Place { kind: Kind::Road, x: bx, y: by }).unwrap();
    let fresh = w.buildings.last().unwrap().id;
    w.build_at(fresh, Kind::Road.build_ticks());
    assert!(w.linked(PlayerId(0), PlayerId(1)), "rebuilding did not restore the link");
}

#[test]
fn a_road_only_the_other_city_may_join() {
    let (mut w, _nav) = two_cities();
    let (ax, ay) = w.map.hearth_sites[0];
    let (bx, by) = w.map.hearth_sites[1];
    w.apply(
        PlayerId(0),
        &Command::Road { from: ((ax + 2) as u8, ay as u8), to: ((bx + 2) as u8, by as u8) },
    )
    .unwrap();
    let road = w.roads.last().unwrap().id;

    // The city that laid it cannot accept its own road, and neither can a
    // player who is not at the far end of it.
    assert!(w.apply(PlayerId(0), &Command::AcceptRoad { road }).is_err());
    w.apply(PlayerId(1), &Command::AcceptRoad { road }).unwrap();
    assert!(
        w.apply(PlayerId(1), &Command::AcceptRoad { road }).is_err(),
        "accepted twice"
    );
}

#[test]
fn a_road_finds_its_way_round_the_rock_and_over_the_water() {
    let (mut w, _nav) = two_cities();
    let road = join_the_cities(&mut w, 0, 1);
    let cells = w.roads[road.0 as usize].cells.clone();

    assert!(cells.len() > 20, "the two cities are 40 cells apart at least");
    for &(x, y) in &cells {
        let g = w.map.ground_at(x as i32, y as i32);
        assert_ne!(g, sim::Ground::Rock, "the road went over a boulder at ({x},{y})");
        let b = w.building_at(x as i32, y as i32).expect("a road cell with nothing on it");
        if g == sim::Ground::Shallows {
            assert_eq!(b.kind, Kind::Bridge, "crossed water without a bridge");
        } else {
            assert_eq!(b.kind, Kind::Road);
        }
    }

    // Every step is orthogonal, so the road is something you can actually walk
    // along rather than a line of cells touching at their corners.
    for pair in cells.windows(2) {
        let d = (pair[0].0 as i32 - pair[1].0 as i32).abs()
            + (pair[0].1 as i32 - pair[1].1 as i32).abs();
        assert_eq!(d, 1, "the road jumps from {:?} to {:?}", pair[0], pair[1]);
    }
}

// ---- phase 2's definition of done -----------------------------------------

/// A city on the lowland in the flood's path, with or without a dike between
/// it and the water.
///
/// Flat ground on purpose. On generated terrain, whether a city drowns depends
/// mostly on how far it happens to sit from the low corner — which is the
/// game, and exactly the wrong variable for a test about dikes.
fn a_city_in_the_path(with_a_dike: bool) -> u32 {
    use sim::map::{Corner, Ground, CELLS};

    let mut w = World::new(SEED, 2);
    for i in 0..CELLS {
        w.map.height[i] = 40;
        w.map.ground[i] = Ground::Grass;
    }
    w.disaster.sources = vec![(Corner::NorthWest, 0)];
    w.disaster.height = 12;

    // Everybody of player 0 stands together on the lowland, twenty cells in
    // from the corner the water comes out of.
    for i in 0..w.citizens.len() {
        if w.citizens[i].owner == PlayerId(0) {
            let n = i as i32;
            w.citizens[i].pos = sim::fx::V2::cell_centre(15 + n % 4, 15 + n / 4);
            w.citizens[i].halt();
        }
    }

    if with_a_dike {
        // An L of dikes across the water's path, four cells upstream of where
        // the people are standing, built to full height.
        //
        // Two things this had to learn. Distance from the corner matters:
        // twenty cells out the water is still near the source's own depth and
        // goes straight over a low dike, so design §5's "a dike two levels
        // high stops an age-1 flood dead" is a claim about where the flood
        // *arrives*, not about standing in front of the source. And a dike is
        // built to full height here rather than to two levels, because at two
        // levels it holds back exactly six units of water while six units is
        // also the depth a citizen drowns in — the band where a two-level dike
        // is both necessary and sufficient is a single unit wide. That is a
        // fine thing for a *game* to be tight about and a hopeless thing to
        // hang a test on. The two arms meet at the
        // corner, so a cell already taken is skipped rather than placed twice.
        let wall = |w: &mut World, x: i32, y: i32| {
            if w.can_place(PlayerId(0), Kind::Dike, x, y).is_err() {
                return;
            }
            let id = w.place(PlayerId(0), Kind::Dike, x, y).unwrap();
            for _ in 0..DIKE_MAX_LEVEL {
                w.deliver_to(id, Good::Stone, w.buildings[id.0 as usize].outstanding().stone);
                w.build_at(id, Kind::Dike.build_ticks());
                if w.buildings[id.0 as usize].level < DIKE_MAX_LEVEL {
                    w.raise_dike(PlayerId(0), id).unwrap();
                }
            }
            assert_eq!(w.buildings[id.0 as usize].level, DIKE_MAX_LEVEL);
            assert!(w.buildings[id.0 as usize].standing_now());
        };
        for y in 4..30 {
            wall(&mut w, 11, y);
        }
        for x in 4..30 {
            wall(&mut w, x, 11);
        }
    }

    let mut nav = Nav::new();
    let home: Vec<sim::fx::V2> =
        w.citizens.iter().map(|c| c.pos).collect();

    while w.day_of_age() < World::IMPACT_DAY {
        for i in 0..w.citizens.len() {
            w.citizens[i].food = NEED_FULL;
            w.citizens[i].rest = NEED_FULL;
            // They stay where they were put; this is about the water, not
            // about whether they wandered off.
            w.citizens[i].pos = home[i];
            w.citizens[i].halt();
        }
        w.tick(&mut nav, &[]);
    }
    for _ in 0..SURGE_TICKS + 600 {
        for c in &mut w.citizens {
            if c.alive() {
                c.food = NEED_FULL;
                c.rest = NEED_FULL;
            }
        }
        w.tick(&mut nav, &[]);
    }
    w.population(PlayerId(0))
}

#[test]
fn a_city_in_the_flood_survives_only_behind_a_dike() {
    // Phase 2's definition of done, and design §5's teaching moment: "A dike
    // two levels high stops an age-1 flood dead; the water goes around."
    let drowned = a_city_in_the_path(false);
    let saved = a_city_in_the_path(true);

    assert!(
        drowned < FOUNDING_CITIZENS,
        "an age-one flood swept over an undefended city and took nobody"
    );
    assert!(
        saved > drowned,
        "the dike saved nobody: {saved} alive behind it against {drowned} without it"
    );
    assert_eq!(saved, FOUNDING_CITIZENS, "the dike was supposed to stop it dead");
}
