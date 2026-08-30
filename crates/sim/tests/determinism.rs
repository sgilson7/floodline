//! Two worlds, one seed, ten thousand ticks, the same checksum throughout.
//!
//! This is the test the whole of §3 exists to pass, and CLAUDE.md's rule is
//! that when it fails nothing else is worked on until it passes again. The
//! plan schedules it at item 10, once there is a command stream to script; it
//! is here at item 3 because the rules it enforces are being written now, and
//! a determinism test added after the fact only tells you the code is
//! consistent with whatever it already does.
//!
//! It grows as `sim` does. Right now it drives what exists — world generation
//! and needs — and the scripted `Command` stream covering every variant
//! arrives with `Command` in item 7.

use sim::balance::{FOUNDING_CITIZENS, TICKS_PER_DAY};
use sim::building::{Good, Kind};
use sim::citizen::{Job, PlayerId};
use sim::citizen::CitizenId;
use sim::command::Command;
use sim::nav::{Dest, Nav};
use sim::BuildingId;
use sim::World;

/// The full length the plan asks for. Ten thousand ticks is fifty in-game
/// days, which is well past the three ages an MVP run lasts.
const TICKS: u32 = 10_000;

/// How often the full world and its checksum are compared. In between, only
/// the state a tick can actually change is compared.
///
/// Checksumming every tick was the first version and it did not finish: the
/// checksum serialises the whole world, sixteen thousand map cells included,
/// and doing that twice a tick for ten thousand ticks across eight
/// configurations is eighty thousand encodings of a twenty-kilobyte structure
/// in a debug build. Comparing the mutable state every tick keeps the precise
/// divergence tick — which is the only useful thing to know about a desync —
/// and the periodic full comparison keeps the checksum itself under test.
const FULL_EVERY: u32 = 500;

#[test]
fn two_worlds_from_one_seed_stay_identical_for_ten_thousand_ticks() {
    for seed in [1u64, 42, 0xC0FFEE, u64::MAX] {
        for players in [2u32, 6] {
            let mut a = World::new(seed, players);
            let mut b = World::new(seed, players);
            let (mut na, mut nb) = (Nav::new(), Nav::new());

            assert_eq!(a.checksum(), b.checksum(), "seed {seed}: differed before tick 1");

            for t in 1..=TICKS {
                a.tick(&mut na, &[]);
                b.tick(&mut nb, &[]);

                // Everything a tick can touch. The map and the occupancy grid
                // are written when the world is built and then only by a
                // placement, so comparing thirty-two thousand cells every tick
                // would be cells of nothing; the periodic full comparison
                // below covers them.
                if a.tick != b.tick
                    || a.rng != b.rng
                    || a.citizens != b.citizens
                    || a.buildings != b.buildings
                {
                    panic!(
                        "seed {seed}, {players} players: diverged at tick {t} — {}",
                        first_difference(&a, &b)
                    );
                }

                if t % FULL_EVERY == 0 || t == TICKS {
                    assert_eq!(
                        a.checksum(),
                        b.checksum(),
                        "seed {seed}, {players} players: checksums differ at tick {t} — {}",
                        first_difference(&a, &b)
                    );
                    assert_eq!(a, b, "seed {seed}, {players} players: worlds differ at tick {t}");
                }
            }
        }
    }
}

/// A run replayed from its own seed reaches the same place, which is what the
/// score screen's "here is the seed" promises.
#[test]
fn a_run_replays_from_its_seed() {
    let mut first = World::new(0xD1FF, 3);
    for _ in 0..TICKS_PER_DAY * 4 {
        first.tick_alone();
    }
    let want = first.checksum();

    let mut again = World::new(first.seed, 3);
    for _ in 0..TICKS_PER_DAY * 4 {
        again.tick_alone();
    }
    assert_eq!(again.checksum(), want);
    assert_eq!(again, first);
}

/// The checksum is worth nothing if it cannot tell two worlds apart, so prove
/// it notices the smallest change there is.
#[test]
fn the_checksum_would_actually_catch_a_divergence() {
    let mut a = World::new(9, 2);
    let mut b = World::new(9, 2);
    for _ in 0..500 {
        a.tick_alone();
        b.tick_alone();
    }
    assert_eq!(a.checksum(), b.checksum());

    // One citizen, one unit of food — the size of a rounding difference
    // between a native peer and a wasm one, which is the divergence this
    // whole design is guarding against.
    b.citizens[0].food += 1;
    assert_ne!(a.checksum(), b.checksum(), "a one-unit difference went unnoticed");
}

/// Different worlds must not collide on a checksum either, or a real desync
/// could show as agreement.
#[test]
fn different_runs_do_not_share_a_checksum() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..200u64 {
        let w = World::new(seed, 2);
        assert!(seen.insert(w.checksum()), "seed {seed} collided with an earlier one");
    }
    // A seed outside the range above, or the two-player case would collide
    // with itself and the test would be reporting on its own arithmetic.
    for players in 2..=6u32 {
        let w = World::new(9_999, players);
        assert!(seen.insert(w.checksum()), "{players} players collided");
    }
}

/// Where two worlds first differ, for a failure message worth reading.
fn first_difference(a: &World, b: &World) -> String {
    if a.tick != b.tick {
        return format!("tick {} vs {}", a.tick, b.tick);
    }
    if a.map != b.map {
        return "the map".to_owned();
    }
    if a.rng != b.rng {
        return "the rng state".to_owned();
    }
    if a.occupancy != b.occupancy {
        return "the occupancy grid".to_owned();
    }
    for (x, y) in a.citizens.iter().zip(&b.citizens) {
        if x != y {
            return format!("citizen {:?}: {x:?} vs {y:?}", x.id);
        }
    }
    if a.buildings.len() != b.buildings.len() {
        return format!("{} buildings vs {}", a.buildings.len(), b.buildings.len());
    }
    for (x, y) in a.buildings.iter().zip(&b.buildings) {
        if x != y {
            return format!("building {:?}: {x:?} vs {y:?}", x.id);
        }
    }
    for p in &a.players {
        if a.population(*p) != b.population(*p) {
            return format!("population of {p:?}");
        }
    }
    "something the encoding sees and this function does not".to_owned()
}

/// Player ids are what ownership checks compare, so they had better be stable.
#[test]
fn player_ids_are_stable() {
    let w = World::new(1, 4);
    assert_eq!(w.players, vec![PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)]);
}


/// The same world, driven through everything that changes it, still agrees.
///
/// The plain tick test above only exercises needs decaying. This one places
/// buildings, hauls to them, builds them, sends citizens walking across the
/// map and demolishes things underneath them — which is where the ordering
/// bugs live. Two peers doing the same things in the same order must end up
/// byte-identical, and the fact that the flow fields are a cache outside
/// `World` must not change that.
#[test]
fn a_scripted_game_runs_the_same_twice() {
    let run = |seed: u64| -> Vec<u64> {
        let mut w = World::new(seed, 3);
        let mut nav = Nav::new();
        let mut marks = Vec::new();

        // Somewhere legal for each player to build, found the same way both
        // times because the world it searches is the same world.
        let mut sites = Vec::new();
        for p in 0..3u8 {
            let (hx, hy) = w.map.hearth_sites[p as usize];
            let mut placed = Vec::new();
            'kinds: for kind in [Kind::Cottage, Kind::Granary, Kind::Farm] {
                for r in 3..25i32 {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dy.abs() != r {
                                continue;
                            }
                            if w.can_place(PlayerId(p), kind, hx + dx, hy + dy).is_ok() {
                                let id = w.place(PlayerId(p), kind, hx + dx, hy + dy).unwrap();
                                placed.push((kind, id));
                                continue 'kinds;
                            }
                        }
                    }
                }
            }
            sites.push(placed);
        }
        marks.push(w.checksum());

        // Haul the materials in, from each city's own hearth.
        for (p, placed) in sites.iter().enumerate() {
            let hearth = w.buildings[p].id;
            for &(kind, id) in placed {
                for g in [Good::Wood, Good::Stone] {
                    let want = kind.cost().get(g);
                    let have = w.buildings[hearth.0 as usize].store.take(g, want);
                    w.deliver_to(id, g, have);
                }
            }
        }
        marks.push(w.checksum());

        // Send everybody somewhere: half to their city's first building, half
        // to a fixed cell, so both kinds of destination are exercised.
        for i in 0..w.citizens.len() {
            let owner = w.citizens[i].owner.0 as usize;
            let dest = if i % 2 == 0 {
                match sites[owner].first() {
                    Some(&(_, id)) => Dest::Building(id),
                    None => Dest::Cell(64, 64),
                }
            } else {
                Dest::Cell(64, 64)
            };
            w.citizens[i].walk_to(dest);
        }

        // Build while they walk, and take one thing away underneath them.
        for t in 0..1200u32 {
            if t % 3 == 0 {
                for placed in &sites {
                    for &(_, id) in placed {
                        w.build_at(id, 1);
                    }
                }
            }
            if t == 600 {
                if let Some(&(_, id)) = sites[1].first() {
                    let _ = w.demolish(PlayerId(1), id);
                }
            }
            w.tick(&mut nav, &[]);
            if t % 200 == 0 {
                marks.push(w.checksum());
            }
        }
        marks.push(w.checksum());
        marks
    };

    for seed in [5u64, 1234, 0xBEEF] {
        let a = run(seed);
        let b = run(seed);
        assert_eq!(a, b, "seed {seed}: a scripted game did not replay");
        // And it actually did something, rather than agreeing about nothing.
        let distinct: std::collections::BTreeSet<u64> = a.iter().copied().collect();
        assert!(distinct.len() > 4, "seed {seed}: the world barely changed: {a:?}");
    }
}

/// A cold cache and a warm one must produce the same world.
///
/// This is the property that lets flow fields live outside `World`: a late
/// joiner rebuilds them from a snapshot and must then agree, tick for tick,
/// with a peer that has had them in memory the whole time.
#[test]
fn a_fresh_nav_cache_navigates_like_a_warm_one() {
    let mut warm = World::new(44, 2);
    let mut nav = Nav::new();

    for i in 0..warm.citizens.len() {
        warm.citizens[i].walk_to(Dest::Cell(64, 64));
    }
    let mut cold = warm.clone();

    for _ in 0..300 {
        warm.tick(&mut nav, &[]);
        // A brand new cache every single tick, as if the peer had just been
        // handed a snapshot.
        cold.tick(&mut Nav::new(), &[]);
    }
    assert_eq!(warm.checksum(), cold.checksum());
    assert_eq!(warm, cold);
    assert!(nav.len() > 0, "the warm cache was never used");
}


/// A city actually being lived in, twice.
///
/// The jobs layer is the most decision-heavy code in `sim`: every tick, every
/// citizen picks a granary, a bed, a load or a site out of ordered lists, and
/// any one of those choices going differently on two peers is a desync. Two
/// days of a working city is a lot of those choices.
#[test]
fn a_city_at_work_runs_the_same_twice() {
    let run = |seed: u64| -> Vec<u64> {
        let mut w = World::new(seed, 2);
        let mut nav = Nav::new();

        // A farm, a granary and two cottages for player 0, finished outright.
        let mut built = Vec::new();
        for kind in [Kind::Farm, Kind::Granary, Kind::Cottage, Kind::Cottage] {
            let (hx, hy) = w.map.hearth_sites[0];
            'place: for r in 3..40i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        if w.can_place(PlayerId(0), kind, hx + dx, hy + dy).is_ok() {
                            let id = w.place(PlayerId(0), kind, hx + dx, hy + dy).unwrap();
                            for g in Good::ALL {
                                let want = kind.cost().get(g);
                                if want > 0 {
                                    w.deliver_to(id, g, want);
                                }
                            }
                            w.build_at(id, kind.build_ticks());
                            built.push((kind, id));
                            break 'place;
                        }
                    }
                }
            }
        }
        let farm = built.iter().find(|(k, _)| *k == Kind::Farm).unwrap().1;

        // Three farmers; everyone else hauls, which is what `None` means.
        let mut n = 0;
        for i in 0..w.citizens.len() {
            if w.citizens[i].owner == PlayerId(0) && n < 3 {
                w.citizens[i].job = Some(Job::Farmer);
                w.citizens[i].workplace = Some(farm);
                n += 1;
            }
        }

        let mut marks = vec![w.checksum()];
        for t in 0..(TICKS_PER_DAY * 2) {
            w.tick(&mut nav, &[]);
            if t % 100 == 0 {
                marks.push(w.checksum());
            }
        }
        marks.push(w.checksum());
        marks
    };

    for seed in [31u64, 77, 404] {
        let a = run(seed);
        let b = run(seed);
        assert_eq!(a, b, "seed {seed}: a working city did not replay");
        let distinct: std::collections::BTreeSet<u64> = a.iter().copied().collect();
        assert!(distinct.len() > 3, "seed {seed}: nothing happened: {a:?}");
    }
}


/// The plan's item 10 in full: a scripted command stream covering every
/// `Command` variant, replayed twice, byte for byte.
///
/// The script is written as transcript lines rather than as constructed
/// values, because that is the form the `bot` crate will read in phase 4 — so
/// this test exercises `Command::parse` on the way in and would notice a
/// variant that could be built but not written down.
#[test]
fn a_scripted_command_stream_covering_every_variant_replays() {
    // Where things go is decided once, from a world built the same way both
    // times, so the script itself is a constant.
    let layout = {
        let w = World::new(31, 2);
        let mut spots = Vec::new();
        let (hx, hy) = w.map.hearth_sites[0];
        for kind in [Kind::Farm, Kind::Granary, Kind::Cottage, Kind::Dike] {
            'place: for r in 3..40i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        if w.can_place(PlayerId(0), kind, hx + dx, hy + dy).is_ok()
                            && !spots.iter().any(|&(_, sx, sy): &(Kind, i32, i32)| {
                                (sx - (hx + dx)).abs() < 4 && (sy - (hy + dy)).abs() < 4
                            })
                        {
                            spots.push((kind, hx + dx, hy + dy));
                            break 'place;
                        }
                    }
                }
            }
        }
        assert_eq!(spots.len(), 4, "could not lay out the script's buildings");
        spots
    };

    // Somewhere to lay a road from and to, beside each hearth rather than on
    // it — a road does not go through a building.
    let (road_from, road_to) = {
        let w = World::new(31, 2);
        let beside = |p: usize| {
            let (hx, hy) = w.map.hearth_sites[p];
            (1..8i32)
                .flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r)].into_iter())
                .map(|(dx, dy)| (hx + dx, hy + dy))
                .find(|&(x, y)| w.building_at(x, y).is_none() && w.map.buildable(x, y))
                .expect("nowhere beside a hearth")
        };
        (beside(0), beside(1))
    };

    // Ids are assigned in placement order after the two hearths.
    let farm = BuildingId(2);
    let granary = BuildingId(3);
    let cottage = BuildingId(4);
    let dike = BuildingId(5);

    let script: Vec<(u32, PlayerId, String)> = vec![
        (1, PlayerId(0), format!("place farm {} {}", layout[0].1, layout[0].2)),
        (1, PlayerId(0), format!("place granary {} {}", layout[1].1, layout[1].2)),
        (2, PlayerId(0), format!("place cottage {} {}", layout[2].1, layout[2].2)),
        (2, PlayerId(0), format!("place dike {} {}", layout[3].1, layout[3].2)),
        // Builders on the dike, so it is standing by the time the script
        // tries to raise it. A construction site only makes progress when
        // somebody is assigned to it — hauling brings the stone, building
        // spends it — which is what the first draft of this script got wrong.
        (3, PlayerId(0), format!("assign #0,#1 @{}", dike.0)),
        (4, PlayerId(0), format!("assign #2 @{}", cottage.0)),
        (5, PlayerId(0), "move #3,#4 64 64".to_owned()),
        (6, PlayerId(0), "ping 64 64".to_owned()),
        (7, PlayerId(1), "ping 10 10".to_owned()),
        (40, PlayerId(0), format!("home #5 @{}", cottage.0)),
        (60, PlayerId(0), "unassign #2".to_owned()),
        (100, PlayerId(0), "pause".to_owned()),
        (100, PlayerId(1), "pause".to_owned()),
        (140, PlayerId(0), "resume".to_owned()),
        (140, PlayerId(1), "resume".to_owned()),
        // The road runs from one city to the other and is accepted, and a
        // standing trade is agreed over it. Placed after the buildings above
        // so their ids are not shifted by the hundred road cells.
        (
            10,
            PlayerId(0),
            format!(
                "road {} {} {} {}",
                road_from.0, road_from.1, road_to.0, road_to.1
            ),
        ),
        (12, PlayerId(1), "acceptroad %0".to_owned()),
        (14, PlayerId(0), "trade !1 food 20 wood 20".to_owned()),
        (16, PlayerId(1), "accepttrade $0".to_owned()),
        (200, PlayerId(0), format!("demolish @{}", granary.0)),
        (300, PlayerId(0), format!("assign #6,#7 @{}", farm.0)),
        (320, PlayerId(0), "unassign #7".to_owned()),
        (700, PlayerId(0), format!("raise @{}", dike.0)),
    ];

    // Every variant really is in there.
    {
        let verbs: std::collections::BTreeSet<&str> =
            script.iter().map(|(_, _, l)| l.split_whitespace().next().unwrap()).collect();
        // Fourteen: every variant `Command` has. If a fifteenth is added and
        // not scripted, this is what says so — which is the whole value of
        // item 10 asking for "every variant" rather than "a good spread".
        assert_eq!(
            verbs.len(),
            14,
            "the script covers {} of the fourteen command variants: {verbs:?}",
            verbs.len()
        );
    }

    let run = || -> Vec<u64> {
        let mut w = World::new(31, 2);
        let mut nav = Nav::new();
        let mut marks = Vec::new();

        for t in 0..1500u32 {
            let now: Vec<(PlayerId, Command)> = script
                .iter()
                .filter(|(at, _, _)| *at == t)
                .map(|(_, p, line)| {
                    (*p, Command::parse(line).unwrap_or_else(|| panic!("bad script line {line:?}")))
                })
                .collect();
            w.tick(&mut nav, &now);
            if t % 100 == 0 {
                marks.push(w.checksum());
            }
        }
        marks.push(w.checksum());
        marks
    };

    let a = run();
    let b = run();
    assert_eq!(a, b, "a scripted game did not replay");

    let distinct: std::collections::BTreeSet<u64> = a.iter().copied().collect();
    assert!(distinct.len() > 8, "the script barely changed anything: {a:?}");

    // And the script actually did what it said, so a future edit that
    // silently stops placing anything is noticed.
    //
    // Sampled part-way through as well as at the end, because this script
    // builds no farm and demolishes its granary: there is no food in this
    // world, and by tick 850 the whole city has starved. That is the rules
    // working — a command naming a dead citizen is refused, and dying clears
    // the job it held — but it means every assertion about a citizen has to
    // be made while there is one.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let mut mid = None;
    for t in 0..1500u32 {
        let now: Vec<(PlayerId, Command)> = script
            .iter()
            .filter(|(at, _, _)| *at == t)
            .map(|(_, p, l)| (*p, Command::parse(l).unwrap()))
            .collect();
        w.tick(&mut nav, &now);
        if t == 400 {
            mid = Some(w.clone());
        }
    }

    let mid = mid.unwrap();
    assert_eq!(mid.population(PlayerId(0)), FOUNDING_CITIZENS, "somebody died early");
    // A site wants builders whatever it is going to become, so assigning to
    // the unbuilt farm made builders rather than farmers. The rule working.
    assert_eq!(mid.citizens[6].workplace, Some(farm), "the assignment did not take");
    assert_eq!(mid.citizens[6].job, Some(Job::Builder));
    assert_eq!(mid.citizens[7].job, None, "#7 was unassigned again");
    assert_eq!(mid.citizens[2].job, None, "#2 was unassigned");
    assert!(mid.pings.is_empty(), "pings from tick 6 were never pruned");

    assert!(w.buildings.len() > 6, "the road laid no cells");
    assert_eq!(w.roads.len(), 1);
    assert!(w.roads[0].joined, "the road was never accepted");
    assert_eq!(w.trades.len(), 1);
    assert!(w.trades[0].accepted, "the trade was never accepted");
    assert!(w.buildings[dike.0 as usize].level > 1, "the dike was never raised");
    assert_eq!(w.buildings[farm.0 as usize].kind, Kind::Farm, "ids drifted");
    assert_eq!(w.population(PlayerId(0)), 0, "a city with no food somehow survived");
    let _ = (CitizenId(0), granary);
}
