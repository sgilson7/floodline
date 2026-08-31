//! A level is one more pair of hands, and a building can be picked up.
//!
//! M7's two commands. `Upgrade` is the only thing gold buys and the only
//! reason to run a trading post; `Move` is the second thought a player has
//! about where they put the granary, and it costs time rather than materials.

use sim::balance::*;
use sim::building::{BuildState, Facing, Good, Kind};
use sim::citizen::{CitizenId, Job, PlayerId};
use sim::command::Command;
use sim::world::{RuleError, World};
use sim::BuildingId;

const ME: PlayerId = PlayerId(0);

/// A standing building of `kind` beside the first city.
fn build(w: &mut World, kind: Kind) -> BuildingId {
    let (hx, hy) = w.map.hearth_sites[0];
    for r in 3..30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(ME, kind, Facing::EastWest, x, y).is_ok() {
                    let id = w.place(ME, kind, Facing::EastWest, x, y).unwrap();
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

/// Put some gold in the city's pocket, the way a mule would.
fn pay(w: &mut World, amount: u16) {
    let hearth = w
        .buildings
        .iter()
        .find(|b| b.owner == ME && b.kind == Kind::Hearth)
        .map(|b| b.id)
        .unwrap();
    w.buildings[hearth.0 as usize].store.add(Good::Gold, amount);
}

#[test]
fn a_level_is_one_more_pair_of_hands() {
    // The whole rule, at every kind that sells one. One sentence a player can
    // hold in their head, and no per-kind arithmetic anywhere.
    for kind in Kind::ALL {
        if !kind.upgradable() {
            continue;
        }
        let mut w = World::new(31, 2);
        let id = build(&mut w, kind);
        pay(&mut w, 500);

        // "One more citizen the building can hold" is one sentence and three
        // shapes: a bed at a cottage, a place at a nursery, a job everywhere
        // else. A kind that holds people some fourth way has to come here and
        // say which, which is the point of asking every kind.
        let holds = |w: &World, id: BuildingId| match kind {
            Kind::Cottage => w.buildings[id.0 as usize].beds(),
            Kind::Nursery => w.buildings[id.0 as usize].places(),
            _ => w.buildings[id.0 as usize]
                .slots_for(Job::at(kind).expect("a kind that holds workers has a job")),
        };
        let before = holds(&w, id);
        w.apply(ME, &Command::Upgrade { building: id }).unwrap();
        let after = holds(&w, id);
        assert_eq!(after, before + 1, "{kind:?} did not gain a pair of hands");
        assert_eq!(w.buildings[id.0 as usize].level, 2);
        // It keeps working while it grows: a farm that stopped feeding people
        // while its fourth farmer was hired would be a strange thing to sell.
        assert!(w.buildings[id.0 as usize].standing_now());
    }
}

#[test]
fn a_level_costs_gold_and_costs_more_each_time() {
    let mut w = World::new(31, 2);
    let farm = build(&mut w, Kind::Farm);
    assert_eq!(
        w.apply(ME, &Command::Upgrade { building: farm }),
        Err(RuleError::TooPoor),
        "a city with no gold bought a level"
    );

    pay(&mut w, UPGRADE_GOLD);
    w.apply(ME, &Command::Upgrade { building: farm }).unwrap();
    assert_eq!(w.treasury(ME).gold, 0, "the gold came out of the city's pocket");

    // The second costs twice the first.
    pay(&mut w, UPGRADE_GOLD);
    assert_eq!(
        w.apply(ME, &Command::Upgrade { building: farm }),
        Err(RuleError::TooPoor)
    );
    pay(&mut w, UPGRADE_GOLD);
    w.apply(ME, &Command::Upgrade { building: farm }).unwrap();
    assert_eq!(w.buildings[farm.0 as usize].level, 3);
}

#[test]
fn a_levelled_farm_takes_a_fourth_worker() {
    // What the level is actually *for*, asked the way a player would find out.
    let mut w = World::new(31, 2);
    let farm = build(&mut w, Kind::Farm);
    let hands: Vec<CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == ME)
        .map(|c| c.id)
        .take(4)
        .collect();

    assert_eq!(w.will_take(ME, farm, &hands), 3, "a farm holds three");
    pay(&mut w, 500);
    w.apply(ME, &Command::Upgrade { building: farm }).unwrap();
    assert_eq!(w.will_take(ME, farm, &hands), 4, "and four once it is levelled");
    w.apply(ME, &Command::Assign { citizens: hands, building: farm }).unwrap();
    assert_eq!(
        w.citizens.iter().filter(|c| c.workplace == Some(farm)).count(),
        4
    );
}

#[test]
fn only_where_hands_go_and_never_past_the_cap() {
    let mut w = World::new(31, 2);
    pay(&mut w, 5_000);

    // A store holds goods, not people. See `Kind::upgradable`.
    let granary = build(&mut w, Kind::Granary);
    assert_eq!(
        w.apply(ME, &Command::Upgrade { building: granary }),
        Err(RuleError::NoJobThere)
    );
    // A dike's levels are height, and are bought with stone.
    let dike = build(&mut w, Kind::Dike);
    assert_eq!(w.apply(ME, &Command::Upgrade { building: dike }), Err(RuleError::NoJobThere));

    let farm = build(&mut w, Kind::Farm);
    for _ in 1..MAX_LEVEL {
        w.apply(ME, &Command::Upgrade { building: farm }).unwrap();
    }
    assert_eq!(w.buildings[farm.0 as usize].level, MAX_LEVEL);
    assert_eq!(w.apply(ME, &Command::Upgrade { building: farm }), Err(RuleError::TooHigh));
}

#[test]
fn a_moved_granary_keeps_its_food_its_id_and_its_farmers() {
    // The elegant half of `Move`: it keeps its id, so everybody pointing at it
    // is still pointing at it and simply walks somewhere else.
    let mut w = World::new(31, 2);
    let farm = build(&mut w, Kind::Farm);
    let granary = build(&mut w, Kind::Granary);
    w.deliver_to(granary, Good::Food, 0);
    w.buildings[granary.0 as usize].store.add(Good::Food, 40);

    let hand = w.citizens.iter().find(|c| c.owner == ME).map(|c| c.id).unwrap();
    w.apply(ME, &Command::Assign { citizens: vec![hand], building: farm }).unwrap();

    let (ox, oy) = (w.buildings[granary.0 as usize].x, w.buildings[granary.0 as usize].y);
    let mut to = None;
    for r in 4..20i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (ox as i32 + dx, oy as i32 + dy);
                if w.can_place(ME, Kind::Granary, Facing::EastWest, x, y).is_ok() {
                    to = Some((x, y));
                }
            }
        }
        if to.is_some() {
            break;
        }
    }
    let (tx, ty) = to.expect("nowhere to move it to");

    w.apply(ME, &Command::Move { building: granary, x: tx as u8, y: ty as u8 }).unwrap();
    let b = &w.buildings[granary.0 as usize];
    assert_eq!(b.id, granary, "it kept its id");
    assert_eq!((b.x as i32, b.y as i32), (tx, ty));
    assert_eq!(b.store.food, 40, "and its food");
    assert_eq!(b.state, BuildState::Site, "and it is a site again");
    assert!(b.outstanding().is_empty(), "with its materials already in it");
    assert!(w.building_at(ox as i32, oy as i32).is_none(), "it left where it was");
    assert_eq!(
        w.citizens.iter().find(|c| c.id == hand).unwrap().workplace,
        Some(farm),
        "the farmer is still a farmer"
    );

    // And it shelters nobody until it is finished: that is the price.
    assert_eq!(w.depth_over(tx, ty), 0);
    w.water.raise_to(tx, ty, depth(4));
    assert_eq!(
        w.depth_over(tx, ty),
        depth(4),
        "a granary being moved kept the water off somebody"
    );
}

#[test]
fn a_building_may_shuffle_one_step_onto_its_own_cells() {
    let mut w = World::new(31, 2);
    let id = build(&mut w, Kind::Cottage);
    let (x, y) = (w.buildings[id.0 as usize].x as i32, w.buildings[id.0 as usize].y as i32);
    // One step, so the old footprint and the new one overlap. Validated
    // ignoring its own cells, or it would refuse to stand where it stands.
    w.apply(ME, &Command::Move { building: id, x: (x + 1) as u8, y: y as u8 }).unwrap();
    assert_eq!(w.buildings[id.0 as usize].x as i32, x + 1);
}

#[test]
fn some_things_stay_where_they_are() {
    let mut w = World::new(31, 2);
    let hearth = w
        .buildings
        .iter()
        .find(|b| b.owner == ME && b.kind == Kind::Hearth)
        .map(|b| b.id)
        .unwrap();
    let (hx, hy) = w.map.hearth_sites[0];
    assert_eq!(
        w.apply(ME, &Command::Move { building: hearth, x: (hx + 8) as u8, y: hy as u8 }),
        Err(RuleError::CannotMove),
        "the hearth is where its people came from"
    );

    let road = build(&mut w, Kind::Road);
    assert_eq!(
        w.apply(ME, &Command::Move { building: road, x: (hx + 9) as u8, y: hy as u8 }),
        Err(RuleError::CannotMove),
        "a road is a cell, and demolish is the verb for those"
    );
}

#[test]
fn a_move_onto_somebody_else_is_refused_and_changes_nothing() {
    let mut w = World::new(31, 2);
    let a = build(&mut w, Kind::Cottage);
    let b = build(&mut w, Kind::Granary);
    let (bx, by) = (w.buildings[b.0 as usize].x, w.buildings[b.0 as usize].y);
    let before = w.buildings[a.0 as usize].clone();

    assert_eq!(
        w.apply(ME, &Command::Move { building: a, x: bx, y: by }),
        Err(RuleError::Occupied)
    );
    assert_eq!(w.buildings[a.0 as usize], before, "a refused move moved something");
}
