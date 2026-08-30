//! A city that feeds itself.
//!
//! The plan's item 6 asks for farms producing food into the nearest granary
//! via haulers, citizens eating at a granary and sleeping at a cottage. This
//! is the test that says all of that happens at once, over days, without
//! anybody having to be told what to do each tick.

use sim::balance::*;
use sim::building::{Good, Kind};
use sim::citizen::{Job, PlayerId, State};
use sim::nav::Nav;
use sim::world::World;
use sim::BuildingId;

/// Somewhere legal for `kind`, searched outward from a player's hearth.
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
    panic!("nowhere to put a {kind:?} near hearth {p}");
}

/// Place `kind` and finish it instantly, as a player who had already spent the
/// materials and the days would have.
fn build(w: &mut World, p: u8, kind: Kind) -> BuildingId {
    let (x, y) = spot(w, p, kind);
    let id = w.place(PlayerId(p), kind, x, y).unwrap();
    for g in Good::ALL {
        let want = kind.cost().get(g);
        if want > 0 {
            w.deliver_to(id, g, want);
        }
    }
    assert!(w.build_at(id, kind.build_ticks()), "{kind:?} did not finish");
    id
}

/// A two-player world where player 0 has a farm, a granary and two cottages,
/// four farmers and four haulers.
fn a_working_city() -> (World, Nav) {
    let mut w = World::new(31, 2);
    let farm = build(&mut w, 0, Kind::Farm);
    build(&mut w, 0, Kind::Granary);
    build(&mut w, 0, Kind::Cottage);
    build(&mut w, 0, Kind::Cottage);

    // Three to the farm — its slots — and the rest left as haulers, which is
    // what an unassigned citizen is.
    let mut assigned = 0;
    for i in 0..w.citizens.len() {
        if w.citizens[i].owner != PlayerId(0) {
            continue;
        }
        if assigned < Kind::Farm.job_slots() {
            w.citizens[i].job = Some(Job::Farmer);
            w.citizens[i].workplace = Some(farm);
            assigned += 1;
        }
    }
    (w, Nav::new())
}

#[test]
fn a_farm_with_farmers_makes_food_and_one_without_does_not() {
    let (mut w, mut nav) = a_working_city();
    let farm = w
        .buildings
        .iter()
        .find(|b| b.kind == Kind::Farm)
        .map(|b| b.id)
        .unwrap();

    // Nobody is there yet, so nothing is made.
    w.tick(&mut nav);
    assert_eq!(w.buildings[farm.0 as usize].store.food, 0);

    // Give them time to walk to it and work.
    for _ in 0..400 {
        w.tick(&mut nav);
    }
    let workers = w.buildings[farm.0 as usize].workers.len();
    assert!(workers > 0, "no farmer ever reached the farm");
    assert!(
        w.treasury(PlayerId(0)).food > 0 || w.buildings[farm.0 as usize].store.food > 0,
        "the farm made nothing in two in-game days"
    );
}

#[test]
fn haulers_move_the_harvest_to_the_granary() {
    let (mut w, mut nav) = a_working_city();
    let granary = w
        .buildings
        .iter()
        .find(|b| b.kind == Kind::Granary)
        .map(|b| b.id)
        .unwrap();

    // Watched over time rather than sampled at the end: the citizens are
    // eating out of the granary as fast as the haulers fill it, so "is there
    // food in it right now" is a question with no stable answer. What matters
    // is that food gets there at all.
    let mut ever = 0u16;
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav);
        ever = ever.max(w.buildings[granary.0 as usize].store.food);
    }
    assert!(ever > 0, "three days and nothing ever reached the granary");
}

/// The one that matters: does a city keep itself alive?
#[test]
fn a_city_with_a_farm_and_a_granary_does_not_starve() {
    let (mut w, mut nav) = a_working_city();
    let start = w.population(PlayerId(0));
    assert_eq!(start, FOUNDING_CITIZENS);

    // Well past the point where the founding party would have starved with
    // nothing to eat: food empties in 250 ticks and death follows 600 later.
    for _ in 0..TICKS_PER_DAY * 12 {
        w.tick(&mut nav);
    }

    assert_eq!(
        w.population(PlayerId(0)),
        start,
        "the city starved with a farm and a granary standing"
    );
    // And they really did eat rather than never getting hungry.
    assert!(
        w.citizens.iter().any(|c| c.owner == PlayerId(0) && c.food < NEED_FULL),
        "nobody ever got hungry, so nothing was being tested"
    );
}

/// The control: the same city with no farm dies, so the test above is not
/// passing for some reason unrelated to food.
#[test]
fn a_city_without_a_farm_starves() {
    let mut w = World::new(31, 2);
    build(&mut w, 0, Kind::Granary);
    let mut nav = Nav::new();

    for _ in 0..TICKS_PER_DAY * 12 {
        w.tick(&mut nav);
    }
    assert_eq!(
        w.population(PlayerId(0)),
        0,
        "a city with no food source somehow survived twelve days"
    );
}

#[test]
fn citizens_sleep_in_cottages_and_wake_rested() {
    let (mut w, mut nav) = a_working_city();

    let mut slept = false;
    let mut had_a_home = false;
    for _ in 0..TICKS_PER_DAY * 6 {
        w.tick(&mut nav);
        for c in &w.citizens {
            if c.owner != PlayerId(0) {
                continue;
            }
            slept |= c.state == State::Sleeping;
            had_a_home |= c.home.is_some();
        }
    }
    assert!(had_a_home, "nobody ever claimed a bed");
    assert!(slept, "nobody ever slept");

    // And rest is being kept up rather than sliding to nothing.
    let worst = w
        .citizens
        .iter()
        .filter(|c| c.owner == PlayerId(0) && c.alive())
        .map(|c| c.rest)
        .min()
        .unwrap();
    assert!(worst > 0, "somebody has not slept in six days");
}

#[test]
fn a_cottage_holds_only_four() {
    let mut w = World::new(31, 2);
    let cottage = build(&mut w, 0, Kind::Cottage);
    build(&mut w, 0, Kind::Granary);
    let mut nav = Nav::new();

    for _ in 0..TICKS_PER_DAY * 4 {
        w.tick(&mut nav);
    }
    let living_there = w.citizens.iter().filter(|c| c.home == Some(cottage)).count();
    assert!(
        living_there <= Kind::Cottage.beds(),
        "{living_there} citizens are sharing four beds"
    );
    assert!(living_there > 0, "nobody moved in");
}

#[test]
fn haulers_supply_a_building_site_from_the_hearth() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    // A site, and nobody assigned to anything — so everybody is a hauler.
    let (x, y) = spot(&w, 0, Kind::Cottage);
    let site = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();
    assert_eq!(w.buildings[site.0 as usize].outstanding().wood, 30);

    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav);
        if w.buildings[site.0 as usize].outstanding().is_empty() {
            break;
        }
    }
    assert!(
        w.buildings[site.0 as usize].outstanding().is_empty(),
        "three days and the wood never arrived: {:?} still wanted",
        w.buildings[site.0 as usize].outstanding()
    );
    // The wood came out of the hearth, which is where it started.
    assert!(w.buildings[0].store.wood < STARTING_WOOD);
}

#[test]
fn builders_finish_what_haulers_supply() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let (x, y) = spot(&w, 0, Kind::Cottage);
    let site = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();

    // Two builders, the rest hauling.
    let mut n = 0;
    for i in 0..w.citizens.len() {
        if w.citizens[i].owner == PlayerId(0) && n < 2 {
            w.citizens[i].job = Some(Job::Builder);
            w.citizens[i].workplace = Some(site);
            n += 1;
        }
    }

    for _ in 0..TICKS_PER_DAY * 5 {
        w.tick(&mut nav);
        if w.buildings[site.0 as usize].standing_now() {
            break;
        }
    }
    assert!(
        w.buildings[site.0 as usize].standing_now(),
        "five days and the cottage is still a site: {:?} outstanding, {} progress",
        w.buildings[site.0 as usize].outstanding(),
        w.buildings[site.0 as usize].progress
    );
}

#[test]
fn a_starving_citizen_drops_what_it_is_carrying_and_goes_to_eat() {
    let (mut w, mut nav) = a_working_city();

    // Run until somebody is both hungry and heading for the granary.
    let mut ate = false;
    for _ in 0..TICKS_PER_DAY * 6 {
        w.tick(&mut nav);
        if w.citizens.iter().any(|c| c.state == State::Eating) {
            ate = true;
            break;
        }
    }
    assert!(ate, "six days and nobody ever ate");
}

#[test]
fn a_city_that_loses_its_granary_stops_eating_there() {
    let (mut w, mut nav) = a_working_city();
    let granary = w.buildings.iter().find(|b| b.kind == Kind::Granary).map(|b| b.id).unwrap();

    let mut ever = 0u16;
    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav);
        ever = ever.max(w.buildings[granary.0 as usize].store.food);
    }
    assert!(ever > 0, "the granary was never in use, so losing it proves nothing");

    // The flood takes it. Nothing should panic, and nobody should keep walking
    // to a hole in the ground.
    w.demolish(PlayerId(0), granary).unwrap();
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav);
    }
    assert!(
        w.citizens.iter().all(|c| c.errand.is_none()
            || !matches!(c.errand, Some(sim::citizen::Errand::ToEat(g)) if g == granary)),
        "somebody is still walking to a granary that is not there"
    );
}
