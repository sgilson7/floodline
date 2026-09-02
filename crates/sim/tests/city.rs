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
fn a_starving_citizen_stops_what_it_is_doing_and_goes_to_eat() {
    // Named for dropping the load until M13, which is not what happens any
    // more and was never what this test checked: it asserts that somebody eats.
    // `a_hungry_hauler_keeps_what_it_is_carrying` is the other half.
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

#[test]
fn a_farm_feeds_a_founding_party_several_times_over() {
    // `FARM_TICKS_PER_UNIT`, held to what the game actually does — which
    // nothing did until now. Tripling a farm's output in M12 moved **not one
    // of the two hundred and eighty-nine tests in the suite**, because not one
    // of them asked what a farm feeds. Three playtests found the number by
    // hand instead, and all three found the same thing: feeding the city was
    // the whole game, and walling, growing and getting uphill were paid for in
    // days nobody had.
    //
    // Bounded on both sides, like the forester and the quarry above. Too
    // little and food is the only clock; too much and there is no clock at
    // all, and a granary nobody has to think about is a granary that should
    // not be on the map.
    use sim::balance::{FOOD_A_DAY, NEED_FULL, TICKS_PER_DAY};
    use sim::building::{Facing, Good, Goods, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
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
                if w.can_place(me, Kind::Farm, Facing::EastWest, x, y).is_ok() {
                    w.apply(
                        me,
                        &Command::Place {
                            kind: Kind::Farm,
                            facing: Facing::EastWest,
                            x: x as u8,
                            y: y as u8,
                        },
                    )
                    .unwrap();
                    placed = Some(w.buildings.last().unwrap().id);
                    break 'ring;
                }
            }
        }
    }
    let id = placed.expect("nowhere for a farm on seed 31");
    for g in Good::ALL {
        let want = w.buildings[id.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(id, g, want);
        }
    }
    assert!(w.build_at(id, Kind::Farm.build_ticks()));

    let all: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me).map(|c| c.id).collect();
    let room = w.will_take(me, id, &all);
    assert_eq!(room, Kind::Farm.job_slots(), "a farm did not offer its slots");
    let take: Vec<sim::CitizenId> = all.into_iter().take(room).collect();
    w.apply(me, &Command::Assign { citizens: take, building: id }).unwrap();

    // A day.
    //
    // **What this arranges** — the rule M11 paid for: hunger is held off, so
    // this asks what a farm makes rather than whether the same eight people
    // can also feed themselves while making it; and the farm's output buffer
    // is emptied every tick, which is what a hauler that keeps up does. Both
    // matter, and the second one is not a formality. A city starts with a
    // Hearth, and a Hearth deliberately holds **no food** (design §3.3 gives
    // it no larder), so until a granary is standing there is nowhere on the
    // map to put a farm's output: the buffer fills at `FARM_BUFFER` and the
    // farmers stop. Measured without draining, a farm makes exactly sixty
    // units in a day at *any* value of this constant, which is the buffer and
    // not the rate. That is a real thing about the game and it is not what
    // this test is for.
    let mut made: u16 = 0;
    for _ in 0..TICKS_PER_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
        let b = &mut w.buildings[id.0 as usize];
        made += b.store.get(Good::Food);
        b.store = Goods::NONE;
    }
    let fed = made as u32 / FOOD_A_DAY;
    println!("  a day of one farm: {made} food, which keeps {fed} people");

    assert!(
        fed >= 16,
        "one farm feeds {fed} people a day, and a founding party is eight — \
         food is the only clock again"
    );
    assert!(
        fed <= 48,
        "one farm feeds {fed} people a day, which is a granary nobody has to \
         think about"
    );
}

// ---- M12.B: the builder's hut ---------------------------------------------

/// Put `kind` down somewhere near the hearth it will actually stand, and give
/// it every material it asks for. Returns the site.
fn site_near_hearth(w: &mut sim::World, me: sim::PlayerId, kind: sim::building::Kind) -> sim::building::BuildingId {
    use sim::building::{Facing, Good};
    use sim::command::Command;
    let (hx, hy) = w.map.hearth_sites[0];
    for r in 3..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, kind, Facing::EastWest, x, y).is_ok() {
                    w.apply(
                        me,
                        &Command::Place { kind, facing: Facing::EastWest, x: x as u8, y: y as u8 },
                    )
                    .unwrap();
                    let id = w.buildings.last().unwrap().id;
                    for g in Good::ALL {
                        let want = w.buildings[id.0 as usize].outstanding().get(g);
                        if want > 0 {
                            w.deliver_to(id, g, want);
                        }
                    }
                    return id;
                }
            }
        }
    }
    panic!("nowhere for a {kind:?} on this seed");
}

#[test]
fn a_hut_names_builders_and_nobody_ever_stands_in_it() {
    // `Job::Builder` was the one job with no building behind it, so a player
    // could point four people at *one site* and never say "these are my
    // builders". The hut says it.
    //
    // It is a roster and not a bench, and that is the load-bearing part: a
    // building whose workers stand in it would be four people doing nothing at
    // the exact moment the city needs a wall. `Job::Builder` is not
    // `stationed`, so nobody is ever added to its `workers`, and `find_work`
    // lets go of the hut the first time it looks.
    use sim::building::Kind;
    use sim::citizen::Job;
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    let hut = site_near_hearth(&mut w, me, Kind::BuildersHut);
    assert!(
        w.buildings[hut.0 as usize].outstanding().is_empty(),
        "a hut is free, so it should want no materials at all"
    );
    assert!(w.build_at(hut, Kind::BuildersHut.build_ticks()));

    // Six, which is more than BUILDER_SLOTS. A hut caps nothing — naming six
    // builders is a thing a player is allowed to want, and the cap that means
    // something is on how many can crowd one site.
    let all: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me && !c.is_child()).map(|c| c.id).collect();
    let six: Vec<sim::CitizenId> = all.into_iter().take(6).collect();
    assert_eq!(
        w.will_take(me, hut, &six),
        six.len(),
        "a hut turned somebody away; it is a roster, it has no room to run out of"
    );
    w.apply(me, &Command::Assign { citizens: six.clone(), building: hut }).unwrap();
    for id in &six {
        assert_eq!(
            w.citizens[id.0 as usize].job,
            Some(Job::Builder),
            "assigning to a hut did not make a builder"
        );
    }

    for _ in 0..300 {
        w.tick(&mut nav, &[]);
    }

    assert!(
        w.buildings[hut.0 as usize].workers.is_empty(),
        "somebody is on the hut's roster: it is a bench after all"
    );
    for id in &six {
        let c = &w.citizens[id.0 as usize];
        assert_ne!(c.workplace, Some(hut), "{:?} is standing in the hut", c.id);
        assert_eq!(c.job, Some(Job::Builder), "a builder stopped being one with nothing to do");
    }
}

#[test]
fn a_hut_builder_builds_before_it_hauls_and_an_unassigned_citizen_does_not() {
    // The ordering *is* the feature. An unassigned citizen already builds when
    // there is nothing to carry — `find_haul` first, `take_a_site` second —
    // which is what keeps an unattended city from dying with the materials on
    // the ground. What it will not do is put the wall before the sacks, and
    // that is exactly what M11.9 needed and could only get by unassigning the
    // whole city.
    //
    // **What this arranges**: two sites. One has every material it needs, so
    // it can be built and cannot be hauled to. The other has none, so it can
    // be hauled to and cannot be built. A city with both in front of it has to
    // choose, and the choice is the thing under test.
    use sim::building::{Facing, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);

    let ticks_to_finish = |with_a_hut: bool| -> u32 {
        let mut w = World::new(31, 2);
        let mut nav = Nav::new();

        // Buildable: materials complete, so `take_a_site` will have it.
        let ready = site_near_hearth(&mut w, me, Kind::Granary);

        // Haulable, and enough of it to keep a city busy: four cottages placed
        // and deliberately left wanting. `site_near_hearth` fills a site, so
        // these are placed by hand.
        let (hx, hy) = w.map.hearth_sites[0];
        let mut hungry = 0;
        'ring: for r in 6..40i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    if w.can_place(me, Kind::Cottage, Facing::EastWest, x, y).is_ok() {
                        w.apply(
                            me,
                            &Command::Place {
                                kind: Kind::Cottage,
                                facing: Facing::EastWest,
                                x: x as u8,
                                y: y as u8,
                            },
                        )
                        .unwrap();
                        hungry += 1;
                        if hungry == 4 {
                            break 'ring;
                        }
                    }
                }
            }
        }
        assert_eq!(hungry, 4, "nowhere for the cottages that make the hauling");

        if with_a_hut {
            let hut = site_near_hearth(&mut w, me, Kind::BuildersHut);
            assert!(w.build_at(hut, Kind::BuildersHut.build_ticks()));
            let four: Vec<sim::CitizenId> = w
                .citizens
                .iter()
                .filter(|c| c.owner == me && !c.is_child())
                .map(|c| c.id)
                .take(4)
                .collect();
            w.apply(me, &Command::Assign { citizens: four, building: hut }).unwrap();
        }

        for t in 0..3000u32 {
            for c in &mut w.citizens {
                c.food = sim::balance::NEED_FULL;
                c.rest = sim::balance::NEED_FULL;
            }
            w.tick(&mut nav, &[]);
            if w.buildings[ready.0 as usize].standing_now() {
                return t;
            }
        }
        u32::MAX
    };

    let without = ticks_to_finish(false);
    let with = ticks_to_finish(true);
    println!("  ticks to finish the granary: {without} unassigned, {with} with a hut");
    assert!(
        with < without,
        "a hut's builders finished the site no sooner than unassigned citizens did \
         ({with} against {without}) — the ordering is the whole feature"
    );
}

// ---- M12.C: the cookery ----------------------------------------------------

#[test]
fn a_cookery_turns_food_into_meals_and_a_meal_feeds_twice_as_far() {
    // The only building in the game that eats a good to make one. Everything
    // else turns worker-ticks into something out of nothing, which is why
    // `produce` had no notion of an input until now.
    //
    // **What this arranges**: the cookery's larder is filled by hand and its
    // output drained by hand, which is what a hauler that keeps up does. The
    // conversion is the thing under test, not the logistics — and the
    // logistics have their own answer, which is that a cookery with nowhere to
    // send meals stops at `COOKERY_BUFFER`, exactly as a farm does.
    use sim::balance::{COOK_TICKS_PER_UNIT, FOOD_PER_MEAL, MEAL_WORTH, NEED_FULL, TICKS_PER_DAY};
    use sim::building::{Good, Goods, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    let cookery = site_near_hearth(&mut w, me, Kind::Cookery);
    assert!(w.build_at(cookery, Kind::Cookery.build_ticks()));
    let two: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me && !c.is_child()).map(|c| c.id).take(2).collect();
    assert_eq!(w.will_take(me, cookery, &two), 2, "a cookery did not offer its slots");
    w.apply(me, &Command::Assign { citizens: two, building: cookery }).unwrap();

    let mut made: u32 = 0;
    let mut eaten: u32 = 0;
    for _ in 0..TICKS_PER_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        // A hauler that keeps up, in both directions.
        let b = &mut w.buildings[cookery.0 as usize];
        let had = b.store.food;
        b.store.set(Good::Food, sim::balance::COOKERY_BUFFER);
        eaten += (sim::balance::COOKERY_BUFFER - had) as u32;
        w.tick(&mut nav, &[]);
        let b = &mut w.buildings[cookery.0 as usize];
        made += b.store.meal as u32;
        b.store.set(Good::Meal, 0);
    }
    println!("  a day of one cookery: {made} meals from {eaten} food");

    // Two cooks at COOK_TICKS_PER_UNIT, and one unit of food per meal.
    let expected = 2 * TICKS_PER_DAY / COOK_TICKS_PER_UNIT;
    assert!(
        made.abs_diff(expected) <= expected / 10,
        "a day of two cooks made {made} meals, and the rate says about {expected}"
    );
    assert!(
        eaten >= made * FOOD_PER_MEAL as u32,
        "{made} meals came out of {eaten} food, which is less than they cost"
    );

    // And what a meal is worth. A citizen eating one is fed `MEAL_WORTH` times
    // as far as the same unit of raw food would have fed it.
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let granary = site_near_hearth(&mut w, me, Kind::Granary);
    assert!(w.build_at(granary, Kind::Granary.build_ticks()));

    let fill_from = |w: &mut World, nav: &mut Nav, g: Good| -> u16 {
        w.buildings[granary.0 as usize].store = Goods::NONE;
        w.buildings[granary.0 as usize].store.set(g, 200);
        let who = w.citizens.iter().position(|c| c.owner == me && !c.is_child()).unwrap();
        w.citizens[who].food = 1;
        let before = w.buildings[granary.0 as usize].store.get(g);
        for _ in 0..TICKS_PER_DAY {
            w.tick(nav, &[]);
            if w.citizens[who].food >= sim::balance::FED_ENOUGH {
                break;
            }
        }
        before - w.buildings[granary.0 as usize].store.get(g)
    };
    let on_food = fill_from(&mut w, &mut nav, Good::Food);
    let on_meals = fill_from(&mut w, &mut nav, Good::Meal);
    println!("  filling one citizen took {on_food} food or {on_meals} meals");
    assert!(on_food > 0 && on_meals > 0, "nobody ate at all");
    assert!(
        on_meals * MEAL_WORTH <= on_food + MEAL_WORTH,
        "{on_meals} meals against {on_food} food: a meal is not worth {MEAL_WORTH} of it"
    );
}

#[test]
fn somebody_both_hungry_and_tired_goes_to_one_of_them() {
    // Reported from a played game: *"a bunch of citizens start getting caught
    // in a loop where they bounce back and forth between going to eat and
    // going to bed, they just end up gyrating in place."*
    //
    // `assign_errands` checks hunger, then tiredness, and each branch only
    // guards against the errand it sets itself. So a citizen that is hungry
    // **and** tired sets `ToEat` on one tick; on the next, `heading_to_eat` is
    // true so hunger is skipped, and `tired() && !heading_to_bed` is true
    // because its errand is `ToEat` - so it abandons the meal and walks toward
    // a bed. On the tick after that, hunger fires again for the same reason.
    // It flips every tick and never covers the ground to either.
    //
    // `abandon()` puts down whatever is being carried, so a hauler caught in
    // this drops its load on the floor every tick as well.
    //
    // **What this arranges**: one citizen, both needs below their thresholds,
    // a standing granary with food in it and a standing cottage to sleep in,
    // and nothing else to do. That is the state; the loop is the game's.
    use sim::balance::{HUNGRY, NEED_FULL, TIRED};
    use sim::building::{Good, Kind};
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    let granary = site_near_hearth(&mut w, me, Kind::Granary);
    assert!(w.build_at(granary, Kind::Granary.build_ticks()));
    w.deliver_to(granary, Good::Food, 200);
    w.buildings[granary.0 as usize].store.set(Good::Food, 200);

    let cottage = site_near_hearth(&mut w, me, Kind::Cottage);
    assert!(w.build_at(cottage, Kind::Cottage.build_ticks()));

    let who = w.citizens.iter().position(|c| c.owner == me && !c.is_child()).unwrap();
    // Everybody else out of the way, so nothing else competes for the granary
    // or the bed and this is one citizen's decision.
    for (i, c) in w.citizens.iter_mut().enumerate() {
        if i != who {
            c.held = true;
        }
    }
    w.citizens[who].food = HUNGRY - 1;
    w.citizens[who].rest = TIRED - 1;

    // **Standing between them, with the two in different directions.** With
    // the granary and the bed both a step away the citizen arrives before it
    // can change its mind, which is why this did not reproduce at first. The
    // report is of people *gyrating in place*, and gyration needs somewhere
    // else to be pulled toward.
    let (gx, gy) = w.buildings[granary.0 as usize].centre();
    let (cx, cy) = w.buildings[cottage.0 as usize].centre();
    let away = (gx + (gx - cx) * 6, gy + (gy - cy) * 6);
    let away = (away.0.clamp(2, sim::map::MAP_W - 3), away.1.clamp(2, sim::map::MAP_H - 3));
    w.citizens[who].pos = sim::fx::V2::cell_centre(away.0, away.1);
    println!("  granary at {gx},{gy}  cottage at {cx},{cy}  citizen at {},{}", away.0, away.1);

    let mut flips = 0;
    let mut last = w.citizens[who].errand;
    let mut arrived = false;
    for _ in 0..600 {
        w.tick(&mut nav, &[]);
        let now = w.citizens[who].errand;
        if now != last {
            flips += 1;
            last = now;
        }
        let s = w.citizens[who].state;
        if s == sim::State::Eating || s == sim::State::Sleeping {
            arrived = true;
            break;
        }
        // Held off the other way: if it is fed and rested it stopped needing
        // either and the question is moot.
        if w.citizens[who].food >= NEED_FULL && w.citizens[who].rest >= NEED_FULL {
            break;
        }
    }

    println!("  errand changed {flips} times before arriving: {arrived}");
    assert!(
        arrived,
        "a citizen that was hungry and tired never reached a granary or a bed \
         in six hundred ticks; its errand changed {flips} times"
    );
    assert!(
        flips < 20,
        "it changed its mind {flips} times getting there - that is the gyration"
    );
}

#[test]
fn a_supplied_site_with_nobody_on_it_gets_built() {
    // Both M12.11 players poured hundreds of stone into dike segments that
    // read `being built` for two entire ages and never finished one. City 0
    // put 440 stone into fifteen of them, had idle hands beside them the whole
    // time, and was told at the end that a wall it had never owned had given
    // way. Four playtests have now asked whether a wall is worth building and
    // nobody has ever owned one.
    //
    // **What this arranges**: a city with a working farm - so there is always
    // something to carry, which is the ordinary state of a city and the whole
    // point - and dike sites that already have every stone they need, so the
    // only thing standing between them and a wall is somebody's hands.
    // Nobody is assigned to anything; these are the unassigned citizens whose
    // rule says they build when there is nothing to haul.
    use sim::balance::{NEED_FULL, TICKS_PER_DAY};
    use sim::building::{Facing, Good, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    // A granary and a manned farm: an ordinary city with hauling to do.
    let granary = site_near_hearth(&mut w, me, Kind::Granary);
    assert!(w.build_at(granary, Kind::Granary.build_ticks()));
    let farm = site_near_hearth(&mut w, me, Kind::Farm);
    assert!(w.build_at(farm, Kind::Farm.build_ticks()));
    let three: Vec<sim::CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && !c.is_child())
        .map(|c| c.id)
        .take(3)
        .collect();
    w.apply(me, &Command::Assign { citizens: three, building: farm }).unwrap();

    // Four dike segments, every stone delivered, nobody assigned.
    let (hx, hy) = w.map.hearth_sites[0];
    let mut dikes = Vec::new();
    'ring: for r in 5..40i32 {
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
                    let id = w.buildings.last().unwrap().id;
                    for g in Good::ALL {
                        let want = w.buildings[id.0 as usize].outstanding().get(g);
                        if want > 0 {
                            w.deliver_to(id, g, want);
                        }
                    }
                    assert!(w.buildings[id.0 as usize].outstanding().is_empty());
                    dikes.push(id);
                    if dikes.len() == 4 {
                        break 'ring;
                    }
                }
            }
        }
    }
    assert_eq!(dikes.len(), 4, "nowhere for the wall");

    // A day. Hunger held off, so this is a question about what people choose
    // to do and not about whether they can stay upright.
    for _ in 0..TICKS_PER_DAY {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }

    let built = dikes.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();
    let progress: u32 = dikes.iter().map(|id| w.buildings[id.0 as usize].progress).sum();
    let need = Kind::Dike.build_ticks();
    println!(
        "  after a day: {built} of 4 segments standing, {progress} builder-ticks of the \
         {} one segment needs",
        need
    );

    assert!(
        progress > 0,
        "a whole day, five idle citizens, four segments with every stone \
         already delivered, and not one builder-tick was applied to any of them"
    );
    assert!(
        built > 0,
        "a day of an unattended city and not one of four fully supplied \
         segments was finished ({progress} builder-ticks against {need} for one)"
    );
}

#[test]
#[ignore]
fn what_happens_to_a_wall_a_player_actually_draws() {
    // The M12.11 reproduction, and unlike the test above it does *not* hand
    // the stone to the sites. Both players drew a long wall with the drag
    // gesture, had plenty of stone at the hearth, and watched the segments sit
    // on `being built` for two ages. City 1 hovered one and read
    // `dike: waiting for 30 stone - nobody is carrying to it` while it had 620
    // stone in the bank.
    //
    // **What this arranges**: a city as a player leaves it - a granary, a
    // manned farm, the starting stone at the hearth - and a fifteen-segment
    // wall ordered in one gesture, which is what the drag tool produces.
    // Nobody is assigned to build. Everything else is the game.
    use sim::balance::{NEED_FULL, TICKS_PER_DAY};
    use sim::building::{Facing, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    let granary = site_near_hearth(&mut w, me, Kind::Granary);
    assert!(w.build_at(granary, Kind::Granary.build_ticks()));
    let farm = site_near_hearth(&mut w, me, Kind::Farm);
    assert!(w.build_at(farm, Kind::Farm.build_ticks()));
    let three: Vec<sim::CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && !c.is_child())
        .map(|c| c.id)
        .take(3)
        .collect();
    w.apply(me, &Command::Assign { citizens: three, building: farm }).unwrap();

    let (hx, hy) = w.map.hearth_sites[0];
    let mut dikes = Vec::new();
    'ring: for r in 6..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Dike, Facing::EastWest, x, y).is_ok() {
                    if w.apply(me, &Command::Place { kind: Kind::Dike,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).is_ok() {
                        dikes.push(w.buildings.last().unwrap().id);
                        if dikes.len() == 15 {
                            break 'ring;
                        }
                    }
                }
            }
        }
    }
    println!("  {} segments ordered, stone in hand {}", dikes.len(), w.treasury(me).stone);

    for day in 1..=4 {
        for _ in 0..TICKS_PER_DAY {
            for c in &mut w.citizens {
                c.food = NEED_FULL;
                c.rest = NEED_FULL;
            }
            w.tick(&mut nav, &[]);
        }
        let standing = dikes.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();
        let supplied = dikes
            .iter()
            .filter(|id| w.buildings[id.0 as usize].outstanding().is_empty())
            .count();
        let ticks: u32 = dikes.iter().map(|id| w.buildings[id.0 as usize].progress).sum();
        let building = w
            .citizens
            .iter()
            .filter(|c| c.alive() && c.workplace.is_some_and(|b| dikes.contains(&b)))
            .count();
        println!(
            "  day {day}: {standing} standing, {supplied} fully supplied, \
             {ticks} builder-ticks, {building} on the wall, stone left {}",
            w.treasury(me).stone
        );
    }
}

#[test]
fn a_city_that_evacuated_is_told_its_people_are_still_standing_there() {
    // **This is why nobody has ever finished a wall.**
    //
    // `MoveTo` - "choose everybody, send them uphill", which is the order the
    // panel itself tells a player to give when the water comes - sets `held`
    // on every citizen it moves. A held citizen is skipped by `find_work`
    // entirely and is never given anything to do again until the player says
    // so. That is deliberate and it is right: an order to stand on a hill must
    // not be quietly undone by the ordinary work loop.
    //
    // What is not right is that **nothing tells the player**. City 0 in the
    // M12.11 run evacuated, went back to its business, and found four of its
    // seven citizens still standing on the rock three game-days later - by
    // opening the people tab out of curiosity. Its fifteen dike segments sat
    // fully supplied and unbuilt for two ages beside idle hands, and the amber
    // line spent that time recommending a trading post.
    //
    // **What this arranges**: a city with a supplied wall and every citizen
    // sent somewhere, which is a city one tick after an evacuation.
    use sim::balance::{NEED_FULL, TICKS_PER_DAY};
    use sim::building::{Facing, Good, Kind};
    use sim::command::Command;
    use sim::nav::Nav;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();

    let granary = site_near_hearth(&mut w, me, Kind::Granary);
    assert!(w.build_at(granary, Kind::Granary.build_ticks()));

    let (hx, hy) = w.map.hearth_sites[0];
    let mut dikes = Vec::new();
    'ring: for r in 5..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Dike, Facing::EastWest, x, y).is_ok()
                    && w.apply(me, &Command::Place { kind: Kind::Dike,
                        facing: Facing::EastWest, x: x as u8, y: y as u8 }).is_ok()
                {
                    let id = w.buildings.last().unwrap().id;
                    for g in Good::ALL {
                        let want = w.buildings[id.0 as usize].outstanding().get(g);
                        if want > 0 {
                            w.deliver_to(id, g, want);
                        }
                    }
                    dikes.push(id);
                    if dikes.len() == 4 {
                        break 'ring;
                    }
                }
            }
        }
    }
    assert_eq!(dikes.len(), 4);

    // The evacuation the panel asks for.
    let everybody: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me && c.alive()).map(|c| c.id).collect();
    w.apply(me, &Command::MoveTo { citizens: everybody.clone(), x: hx as u8, y: (hy - 3) as u8 })
        .unwrap();

    for _ in 0..TICKS_PER_DAY * 2 {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }

    let standing = dikes.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();
    let waiting = w.citizens.iter().filter(|c| c.owner == me && c.alive() && c.held).count();
    println!("  two days after an evacuation: {standing} of 4 supplied segments built, {waiting} still waiting where they were sent");

    // The behaviour itself, pinned. It is correct and it is the trap.
    assert_eq!(waiting, everybody.len(), "an evacuation should hold everybody it moved");
    assert_eq!(
        standing, 0,
        "held citizens should not go back to work on their own - that is the whole point of `held`"
    );

    // **And the city must be told**, which is the part that was missing and
    // which lives in `gui`: see `tutorial::tests::a_city_still_standing_where_it_was_sent_is_told`.
}

// ---- M13: why nobody has ever finished a wall ------------------------------
//
// Four playtests asked whether a dike is worth building and none could answer,
// because no player has ever owned a finished one. `playtest.rs`'s
// `what_a_walling_city_spends_its_days_on` watched the `dike` script day by day
// and found three faults, none of which had been guessed at in three
// milestones of handovers. These are the three, one test each.

/// Dike sites near the hearth, ordered but **not** supplied. Returns their ids.
fn unsupplied_wall(w: &mut World, me: PlayerId, want: usize) -> Vec<BuildingId> {
    let (hx, hy) = w.map.hearth_sites[0];
    let mut dikes = Vec::new();
    for r in 5..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Dike, Facing::EastWest, x, y).is_ok()
                    && w.apply(
                        me,
                        &Command::Place {
                            kind: Kind::Dike,
                            facing: Facing::EastWest,
                            x: x as u8,
                            y: y as u8,
                        },
                    )
                    .is_ok()
                {
                    dikes.push(w.buildings.last().unwrap().id);
                    if dikes.len() == want {
                        return dikes;
                    }
                }
            }
        }
    }
    panic!("nowhere for a wall on this seed");
}

/// Every stone the city can account for: at rest, in somebody's arms, or
/// delivered to something. Nothing here quarries, so this may never fall.
fn all_the_stone(w: &World, me: PlayerId) -> u16 {
    w.treasury(me).stone
        + w.in_hand(me).stone
        + w.buildings.iter().filter(|b| b.owner == me).map(|b| b.delivered.stone).sum::<u16>()
}

#[test]
fn a_hungry_hauler_keeps_what_it_is_carrying() {
    // **Seven hundred and ten stone of seven hundred and twenty, in one day.**
    //
    // `Citizen::abandon` cleared `carrying`, and the hunger and exhaustion
    // branches of `assign_errands` both called it, so every time a body
    // overruled a delivery the load stopped existing. `jobs.rs` called that
    // "the real cost of having left it too late" and nothing had ever put a
    // number on it. The number is: on the day a starving city's granary first
    // has food in it, every hauler in the city drops its load at once, and
    // `what_a_walling_city_spends_its_days_on` watched the whole stone reserve
    // of a city go that way between one day and the next.
    //
    // **What this arranges**: a wall to supply, then the exact moment — a
    // citizen holding stone, made hungry with somewhere to eat. Waiting for
    // hunger and a full load to coincide on their own is what the old version
    // of this test did, and on this seed they never did.
    let (mut w, mut nav) = a_working_city();
    let me = PlayerId(0);
    unsupplied_wall(&mut w, me, 6);

    // Run until somebody is carrying stone to the wall.
    let mut carrying = None;
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        carrying = w
            .citizens
            .iter()
            .find(|c| c.owner == me && c.alive() && c.carrying.stone > 0)
            .map(|c| c.id);
        if carrying.is_some() {
            break;
        }
    }
    let who = carrying.expect("a day and nobody ever picked up a stone for the wall");
    let load = w.citizens[who.0 as usize].carrying.stone;
    let all = all_the_stone(&w, me);

    // Somewhere to eat, and a stomach that says go now. Both are needed: the
    // hunger branch does nothing at all when there is no food in the city, and
    // that case is `somebody_both_hungry_and_tired_goes_to_one_of_them`.
    let granary = w
        .buildings
        .iter()
        .find(|b| b.owner == me && b.kind == Kind::Granary)
        .map(|b| b.id)
        .expect("no granary");
    w.deliver_to(granary, Good::Food, 100);
    w.citizens[who.0 as usize].food = 0;
    w.tick(&mut nav, &[]);

    assert_eq!(
        w.citizens[who.0 as usize].carrying.stone, load,
        "hunger took the stone out of somebody's arms"
    );
    assert_eq!(all_the_stone(&w, me), all, "stone left the game when somebody got hungry");

    // And the delivery finishes. `find_haul` looks at the arms before anything
    // else, which is what makes the interruption a delay rather than a loss.
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        if w.citizens[who.0 as usize].carrying.stone == 0 {
            break;
        }
    }
    assert_eq!(
        w.citizens[who.0 as usize].carrying.stone, 0,
        "a day after the meal and the load is still in its arms"
    );
    assert_eq!(all_the_stone(&w, me), all, "the load arrived somewhere that was not a building");
}

#[test]
fn a_builder_at_a_site_with_no_stone_goes_and_gets_it() {
    // **The whole wall, in one line of `work_at`.** `Building::build` returns
    // false while anything is outstanding, and the site arm did not look: the
    // citizen was set to `Working`, its errand stayed `ToWork`, `busy()` stayed
    // true, and `find_work` never ran again. An assigned builder at an
    // unsupplied site worked at nothing for the rest of the game — and since
    // the hands it took were the city's haulers, nothing ever brought the stone
    // that would have released it.
    //
    // In the `dike` script that was five of eight people standing at seven
    // segments for four days at `0% done`, with ninety stone owed and six
    // hundred in the store twenty cells away.
    //
    // **What this arranges**: a manned farm, a granary, six unsupplied
    // segments, and *everybody who is not farming assigned to build*, so there
    // is not one unassigned hauler in the city. Fed and rested every tick: this
    // is a question about what people choose to do.
    let (mut w, mut nav) = a_working_city();
    let me = PlayerId(0);
    let dikes = unsupplied_wall(&mut w, me, 6);

    let free: Vec<sim::CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && c.alive() && c.job.is_none())
        .map(|c| c.id)
        .collect();
    assert!(free.len() >= 2, "nobody spare to put on the wall");
    // Spread over segments, because one segment takes `BUILDER_SLOTS` and no
    // more — a whole city assigned to one dike is refused as `Full`, which is
    // the rule doing its job.
    for (n, who) in free.chunks(BUILDER_SLOTS).enumerate() {
        w.apply(me, &Command::Assign { citizens: who.to_vec(), building: dikes[n] }).unwrap();
    }

    for _ in 0..TICKS_PER_DAY * 2 {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }

    let standing = dikes.iter().filter(|id| w.buildings[id.0 as usize].standing_now()).count();
    let delivered: u16 =
        dikes.iter().map(|id| w.buildings[id.0 as usize].delivered.stone).sum();
    println!("  after two days: {standing} of 6 standing, {delivered} stone delivered");
    assert!(
        delivered > 0,
        "two days, a store full of stone, and the people assigned to the wall \
         brought it none of it"
    );
    assert!(
        standing > 0,
        "two days and not one segment stands: {delivered} stone delivered"
    );
}

#[test]
fn a_city_that_employs_everybody_does_not_starve_beside_its_own_farm() {
    // A farm fills a sixty-unit buffer and stops, a citizen can only eat at a
    // granary, and a citizen with a job never hauls. So a city that employed
    // everybody starved beside its own working farms — both M12.11 players hit
    // it, and M12 answered it with a sentence in the panel telling them the
    // shortage was haulers.
    //
    // The sentence is right and is not enough. Hunger outranks the job: that is
    // this module's own first paragraph and design §3.2's order, and it cannot
    // outrank the job only when somebody else has already done the carrying.
    //
    // **What this arranges**: three farms, every citizen a farmer, an empty
    // granary — so there is no unassigned citizen anywhere and nobody can
    // reach `find_haul` at all. The food exists, in the fields, sixty units at
    // a time. Nothing touches anybody's stomach.
    let (mut w, mut nav) = a_working_city();
    let me = PlayerId(0);
    let farms: Vec<BuildingId> = {
        let mut all: Vec<BuildingId> = w
            .buildings
            .iter()
            .filter(|b| b.owner == me && b.kind == Kind::Farm)
            .map(|b| b.id)
            .collect();
        while all.len() < 3 {
            all.push(build(&mut w, 0, Kind::Farm));
        }
        all
    };
    let spare: Vec<sim::CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && c.alive() && c.job.is_none())
        .map(|c| c.id)
        .collect();
    let slots = Kind::Farm.slots_for(Job::Farmer);
    for (n, who) in spare.chunks(slots).enumerate() {
        w.apply(me, &Command::Assign { citizens: who.to_vec(), building: farms[n + 1] }).unwrap();
    }
    assert!(
        w.citizens.iter().all(|c| c.owner != me || !c.alive() || c.job == Some(Job::Farmer)),
        "somebody is still a hauler, which is the case this is not about"
    );
    assert_eq!(w.treasury(me).food, 0, "the granary should start empty");

    let alive = |w: &World| w.citizens.iter().filter(|c| c.owner == me && c.alive()).count();
    let at_the_start = alive(&w);
    for _ in 0..TICKS_PER_DAY * 4 {
        w.tick(&mut nav, &[]);
    }

    println!(
        "  after four days: {} of {at_the_start} alive, {} food in the granary",
        alive(&w),
        w.treasury(me).food,
    );
    assert_eq!(
        alive(&w),
        at_the_start,
        "somebody starved in a city of farmers with three working farms"
    );
    assert!(
        w.treasury(me).food > 0,
        "four days and not one unit of food ever reached the granary"
    );
}

#[test]
fn a_new_job_does_not_destroy_what_somebody_was_carrying() {
    // `unassign_one` — which every `Assign` goes through, not only "back to
    // hauling" — used to `abandon`, and `abandon` deletes the load. So
    // "you four, build the wall" quietly burned twenty stone for every hauler
    // that happened to be holding some, and the panel showed the stone gone
    // with nothing to explain it. Measured at forty to fifty stone a run in
    // `what_a_walling_city_spends_its_days_on`, against a supply of seven
    // hundred and twenty that nothing in a run replaces.
    //
    // `MoveTo` still drops what it is carrying, and that stays: "get uphill"
    // means drop it. This is about a change of job.
    //
    // **What this arranges**: a hauler mid-delivery, then a job.
    let (mut w, mut nav) = a_working_city();
    let me = PlayerId(0);
    let dikes = unsupplied_wall(&mut w, me, 6);

    let mut carrying = None;
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        carrying = w
            .citizens
            .iter()
            .find(|c| c.owner == me && c.alive() && c.carrying.stone > 0)
            .map(|c| c.id);
        if carrying.is_some() {
            break;
        }
    }
    let who = carrying.expect("a day and nobody ever picked up a stone for the wall");
    let all = all_the_stone(&w, me);

    w.apply(me, &Command::Assign { citizens: vec![who], building: dikes[0] }).unwrap();
    assert_eq!(all_the_stone(&w, me), all, "being given a job destroyed the load");

    // And it arrives: `find_work` takes what is in the arms somewhere it is
    // wanted before it starts anything new, whatever the new job is.
    for _ in 0..TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        if w.citizens[who.0 as usize].carrying.stone == 0 {
            break;
        }
    }
    assert_eq!(
        w.citizens[who.0 as usize].carrying.stone, 0,
        "a day later and the new builder is still holding the load it was given a job over"
    );
    assert_eq!(all_the_stone(&w, me), all, "the load went somewhere that was not a building");
}
