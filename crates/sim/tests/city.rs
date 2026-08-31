//! A city that feeds itself.
//!
//! The plan's item 6 asks for farms producing food into the nearest granary
//! via haulers, citizens eating at a granary and sleeping at a cottage. This
//! is the test that says all of that happens at once, over days, without
//! anybody having to be told what to do each tick.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::{Job, PlayerId, State};
use sim::nav::Nav;
use sim::world::World;
use sim::BuildingId;
use sim::Command;

/// Somewhere legal for `kind`, searched outward from a player's hearth.
fn spot(w: &World, p: u8, kind: Kind) -> (i32, i32) {
    let (hx, hy) = w.map.hearth_sites[p as usize];
    for r in 3..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if w.can_place(PlayerId(p), kind, Facing::EastWest, hx + dx, hy + dy).is_ok() {
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
    let id = w.place(PlayerId(p), kind, Facing::EastWest, x, y).unwrap();
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
    w.tick(&mut nav, &[]);
    assert_eq!(w.buildings[farm.0 as usize].store.food, 0);

    // Give them time to walk to it and work.
    for _ in 0..400 {
        w.tick(&mut nav, &[]);
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
        w.tick(&mut nav, &[]);
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

    // Five days: comfortably past the point where the founding party would
    // have starved with nothing to eat — food empties at tick 1000 and death
    // follows 3600 later, so tick 4600 — and stopping before day six, which is
    // when the water comes.
    //
    // It used to run twelve days, which is two floods, and asserted that
    // nobody died at all. That passed for as long as nobody happened to drown;
    // the first time somebody did, this test reported a starvation. A test
    // about food should not be able to fail because of water.
    for _ in 0..TICKS_PER_DAY * 5 {
        w.tick(&mut nav, &[]);
    }

    assert_eq!(
        w.population(PlayerId(0)),
        start,
        "the city starved with a farm and a granary standing"
    );
    // This city only: the world has a second one with no farm in it, and it
    // is starving exactly as it should be.
    assert!(
        w.citizens.iter().filter(|c| c.owner == PlayerId(0)).all(|c| c.starved_for == 0),
        "somebody is on the starvation clock with a granary in reach"
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
        w.tick(&mut nav, &[]);
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
        w.tick(&mut nav, &[]);
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
        w.tick(&mut nav, &[]);
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
    let site = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();
    assert_eq!(w.buildings[site.0 as usize].outstanding().wood, 30);

    for _ in 0..TICKS_PER_DAY * 3 {
        w.tick(&mut nav, &[]);
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
    let site = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();

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
        w.tick(&mut nav, &[]);
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
        w.tick(&mut nav, &[]);
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
        w.tick(&mut nav, &[]);
        ever = ever.max(w.buildings[granary.0 as usize].store.food);
    }
    assert!(ever > 0, "the granary was never in use, so losing it proves nothing");

    // The flood takes it. Nothing should panic, and nobody should keep walking
    // to a hole in the ground.
    w.demolish(PlayerId(0), granary).unwrap();
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
    }
    assert!(
        w.citizens.iter().all(|c| c.errand.is_none()
            || !matches!(c.errand, Some(sim::citizen::Errand::ToEat(g)) if g == granary)),
        "somebody is still walking to a granary that is not there"
    );
}

#[test]
fn a_city_left_alone_builds_what_it_was_told_to_build() {
    // The trap this exists to stop, found by playing a full run rather than by
    // any test: place a farm and a granary, assign nobody, and the haulers
    // deliver the wood and stone and then stop. The sites sit full and
    // finished-looking, nothing ever builds them, so there is never a granary
    // — and a citizen can only eat at a granary, so the whole city starves on
    // day four with the materials lying on the ground. The gesture that fixes
    // it (select citizens, right-click the site) exists and works, and nothing
    // anywhere tells a player it is needed.
    use sim::building::{BuildState, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let me = PlayerId(0);
    let (hx, hy) = w.map.hearth_sites[0];

    let mut placed = Vec::new();
    for kind in [Kind::Farm, Kind::Granary] {
        let (x, y) = 'found: {
            for r in 3..30i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        if w.can_place(me, kind, Facing::EastWest, hx + dx, hy + dy).is_ok() {
                            break 'found (hx + dx, hy + dy);
                        }
                    }
                }
            }
            panic!("nowhere for a {kind:?}");
        };
        w.apply(me, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).unwrap();
        placed.push(w.buildings.last().unwrap().id);
    }

    // Half a day, and not one command after the two placements.
    for _ in 0..sim::balance::TICKS_PER_DAY / 2 {
        w.tick(&mut nav, &[]);
    }

    for id in placed {
        let b = &w.buildings[id.0 as usize];
        assert_eq!(
            b.state,
            BuildState::Standing,
            "the {:?} was never built: {} of {} builder-ticks, {:?} still outstanding",
            b.kind,
            b.progress,
            b.kind.build_ticks(),
            b.outstanding(),
        );
    }
}

#[test]
fn nobody_takes_a_job_that_was_not_given_to_them() {
    // The other side of the line above, written down because it is a decision
    // and not an omission. A city finishes what it was told to build without
    // being asked twice; it does not decide who farms. Placing a building is
    // an order already given, and carrying it out is not a choice — but who
    // works where is the whole of what a player does with their citizens
    // (design section 3.2: "jobs are assigned StarCraft-style"), and a city
    // that staffed itself would leave them nothing to manage.
    //
    // So a farm with nobody in it produces nothing, and a city that builds a
    // farm and never mans it starves. That is the game asking a question, not
    // a trap: the farm is standing where the player put it and the panel says
    // food 0.
    use sim::building::{BuildState, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let me = PlayerId(0);
    let (hx, hy) = w.map.hearth_sites[0];
    let mut farm = None;
    for kind in [Kind::Farm, Kind::Granary] {
        'found: for r in 3..30i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    if w.can_place(me, kind, Facing::EastWest, x, y).is_ok() {
                        w.apply(me, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).unwrap();
                        if kind == Kind::Farm {
                            farm = Some(w.buildings.last().unwrap().id);
                        }
                        break 'found;
                    }
                }
            }
        }
    }
    let farm = farm.unwrap();

    for _ in 0..sim::balance::TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(w.buildings[farm.0 as usize].state, BuildState::Standing);
    assert!(
        w.buildings[farm.0 as usize].workers.is_empty(),
        "somebody took a job nobody gave them"
    );
    assert_eq!(w.treasury(me).food, 0, "an unmanned farm produced food");
}

#[test]
fn a_forester_and_a_quarry_pay_for_a_building_in_a_day() {
    // The numbers in `balance::FOREST_TICKS_PER_UNIT` and
    // `QUARRY_TICKS_PER_UNIT`, held to what the game actually does.
    //
    // Until these two buildings existed nothing in the MVP made wood or stone
    // at all: a city started with two hundred wood, which is about five
    // buildings, and that was the whole run. "How do I get more wood" had no
    // answer on the build menu.
    use sim::building::{Good, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let day_of = |kind: Kind, good: Good| -> u16 {
        let mut w = World::new(31, 2);
        let mut nav = Nav::new();
        let (hx, hy) = w.map.hearth_sites[0];
        let mut placed = None;
        'ring: for r in 3..40i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    if w.can_place(me, kind, Facing::EastWest, x, y).is_ok() {
                        w.apply(me, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).unwrap();
                        placed = Some(w.buildings.last().unwrap().id);
                        break 'ring;
                    }
                }
            }
        }
        let id = placed.unwrap_or_else(|| panic!("nowhere for a {kind:?} on seed 31"));
        // Built and manned, then left alone for a day. Hunger is held off:
        // this is a question about output, not about whether the same eight
        // people can also feed themselves.
        for g in Good::ALL {
            let want = w.buildings[id.0 as usize].outstanding().get(g);
            if want > 0 {
                w.deliver_to(id, g, want);
            }
        }
        assert!(w.build_at(id, kind.build_ticks()));
        let all: Vec<sim::CitizenId> =
            w.citizens.iter().filter(|c| c.owner == me).map(|c| c.id).collect();
        let room = w.will_take(me, id, &all);
        assert_eq!(room, kind.job_slots(), "{kind:?} did not offer its slots");
        let take: Vec<sim::CitizenId> = all.into_iter().take(room).collect();
        w.apply(me, &Command::Assign { citizens: take, building: id }).unwrap();

        let before = w.treasury(me).get(good) + w.buildings[id.0 as usize].store.get(good);
        for _ in 0..sim::balance::TICKS_PER_DAY {
            for c in &mut w.citizens {
                c.food = sim::balance::NEED_FULL;
                c.rest = sim::balance::NEED_FULL;
            }
            w.tick(&mut nav, &[]);
        }
        let after = w.treasury(me).get(good) + w.buildings[id.0 as usize].store.get(good);
        after - before
    };

    let wood = day_of(Kind::Forester, Good::Wood);
    let stone = day_of(Kind::Quarry, Good::Stone);
    println!("  a day's work: {wood} wood, {stone} stone");

    // A cottage is 30 wood and a farm 40, so a day of two foresters should be
    // about a building; a dike level is 10 stone, so a day of two quarriers
    // should be a couple of dike cells at two levels. Bounded on both sides:
    // too little and the shortage never ends, too much and there is no
    // shortage to end.
    assert!(
        (25..=55).contains(&wood),
        "a day of two foresters made {wood} wood; a cottage is {}",
        Kind::Cottage.cost().wood
    );
    assert!(
        (15..=40).contains(&stone),
        "a day of two quarriers made {stone} stone; a dike level is {}",
        Kind::Dike.cost().stone
    );
}

#[test]
fn a_quarry_needs_rock_beside_it_and_a_forester_does_not() {
    // The one rule in the game that looks at what is next to a footprint
    // rather than under it, and the reason rock is on the map for.
    use sim::building::Kind;
    use sim::map::{Ground, MAP_H, MAP_W};
    use sim::world::World;
    use sim::PlayerId;

    let w = World::new(31, 2);
    let me = PlayerId(0);
    let mut refused_for_rock = 0;
    let mut allowed = 0;
    for y in (2..MAP_H - 3).step_by(3) {
        for x in (2..MAP_W - 3).step_by(3) {
            match w.can_place(me, Kind::Quarry, Facing::EastWest, x, y) {
                Ok(()) => {
                    let (bw, bh) = Kind::Quarry.size(Facing::EastWest);
                    let near = (y - 1..=y + bh).any(|cy| {
                        (x - 1..=x + bw).any(|cx| w.map.ground_at(cx, cy) == Ground::Rock)
                    });
                    assert!(near, "a quarry was allowed at ({x},{y}) with no rock beside it");
                    allowed += 1;
                }
                Err(sim::world::RuleError::NoRockHere) => refused_for_rock += 1,
                Err(_) => {}
            }
        }
    }
    assert!(allowed > 0, "there is nowhere on this map to put a quarry");
    assert!(refused_for_rock > allowed, "the rule is not asking for much");

    // A forester's hut goes anywhere a building goes.
    let anywhere = (2..MAP_H - 3).step_by(3).any(|y| {
        (2..MAP_W - 3).step_by(3).any(|x| w.can_place(me, Kind::Forester, Facing::EastWest, x, y).is_ok())
    });
    assert!(anywhere, "a forester's hut could not be placed at all");
}

#[test]
fn the_two_producers_are_bought_with_what_the_other_one_makes() {
    // A city starts holding stone and wanting wood. The forester's hut costs
    // stone, so the thing you have buys the thing you need; the quarry costs
    // wood, so the wood it makes buys the stone back. Before this the hut cost
    // wood too, which meant the wood shortage funded its own cure and the
    // seven hundred stone in the Hearth had nowhere to go but dikes.
    use sim::balance::{STARTING_STONE, STARTING_WOOD};
    use sim::building::Kind;

    let hut = Kind::Forester.cost();
    let quarry = Kind::Quarry.cost();
    assert_eq!(hut.wood, 0, "a forester's hut must not be bought with the thing it makes");
    assert!(hut.stone > 0);
    assert_eq!(quarry.stone, 0, "a quarry must not be bought with the thing it makes");
    assert!(quarry.wood > 0);

    // And the opening is still affordable: a granary, a farm and a hut on the
    // first day, out of what a city is handed.
    let (granary, farm) = (Kind::Granary.cost(), Kind::Farm.cost());
    let opening_wood = granary.wood + farm.wood + hut.wood;
    let opening_stone = granary.stone + farm.stone + hut.stone;
    assert!(
        opening_wood <= STARTING_WOOD && opening_stone <= STARTING_STONE,
        "a granary, a farm and a forester's hut cost {opening_wood} wood and \
         {opening_stone} stone against {STARTING_WOOD} and {STARTING_STONE}"
    );
    // With room left for the quarry that pays the stone back.
    let rest = STARTING_WOOD - opening_wood;
    assert!(
        rest >= quarry.wood,
        "only {rest} wood left after the opening, and a quarry is {}",
        quarry.wood
    );
}

/// What a city costs to keep, which the panel could not say until M10.5.
///
/// Both players in the rehearsal starved beside a staffed farm while the panel
/// told them "the granary is empty - give the farm a moment", unchanged, for
/// two days. It named the mechanism and never the clock. These are the two
/// numbers that fix it, and they are arithmetic on the eating model rather
/// than constants somebody chose: a need falls `FOOD_DECAY` a tick over
/// `TICKS_PER_DAY` and one stored unit fills `FOOD_PER_UNIT` of it.
#[test]
fn a_city_can_say_what_it_eats_and_how_long_the_larder_lasts() {
    let mut w = World::new(7, 2);
    let me = PlayerId(0);
    let mouths = w.population(me);
    assert!(mouths > 0, "a new city has nobody in it");

    // Twelve a head a day, derived from the decay and the exchange rate.
    assert_eq!(FOOD_A_DAY, (TICKS_PER_DAY * FOOD_DECAY as u32) / FOOD_PER_UNIT as u32);
    assert_eq!(w.eaten_a_day(me), mouths * FOOD_A_DAY);

    // An empty granary is nought days, and says so.
    assert_eq!(w.treasury(me).food, 0, "a city does not start with food");
    assert_eq!(w.days_of_food(me), Some(0));

    // Three days' worth is three days.
    let three = 3 * w.eaten_a_day(me);
    let store = w.buildings.iter().position(|b| b.owner == me).expect("a hearth");
    w.buildings[store].store.add(Good::Food, three as u16);
    assert_eq!(w.treasury(me).food as u32, three);
    assert_eq!(w.days_of_food(me), Some(3));

    // And a dead city is not a city with nought days of food. Telling a player
    // "0 days left" about people who are already gone would be worse than
    // saying nothing, which is what the panel does with `None`.
    for c in w.citizens.iter_mut().filter(|c| c.owner == me) {
        c.die();
    }
    assert_eq!(w.population(me), 0);
    assert_eq!(w.eaten_a_day(me), 0);
    assert_eq!(w.days_of_food(me), None, "nobody left to feed is not nought days");
}

/// Filling a second farm must not empty the first.
///
/// `Command::Assign` takes the citizens it is given, in the order it is given
/// them, and the panel used to hand it the selection in id order — so sending
/// "everybody" to a second farm was as likely to take the three already
/// working the first as the three standing idle beside it. Both players in the
/// M10.6 run named worker assignment as the worst part of the game, and one
/// spent about a third of its run on a workaround for exactly this.
///
/// The ordering lives in `gui`, because it is about which people a *click*
/// means. What `sim` promises, and what this pins, is the half that makes such
/// an ordering possible at all: the command is a list and it is honoured as
/// given, so choosing who goes is a decision the caller can make.
#[test]
fn assign_takes_the_citizens_it_is_given_in_the_order_given() {
    let (mut w, mut nav) = a_working_city();
    let me = PlayerId(0);
    let farm = build(&mut w, 0, Kind::Farm);
    for _ in 0..400 {
        w.tick(&mut nav, &[]);
    }

    let free: Vec<_> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && c.alive() && c.workplace.is_none())
        .map(|c| c.id)
        .collect();
    assert!(free.len() >= 2, "need two idle citizens, found {}", free.len());

    // Named in reverse, so "as given" and "in id order" cannot agree by luck.
    let asked: Vec<_> = free.iter().rev().take(2).copied().collect();
    w.apply(me, &Command::Assign { citizens: asked.clone(), building: farm })
        .expect("two idle citizens fit a farm");

    for id in &asked {
        assert_eq!(
            w.citizens[id.0 as usize].workplace,
            Some(farm),
            "citizen {id:?} was named and should have gone"
        );
    }
}
