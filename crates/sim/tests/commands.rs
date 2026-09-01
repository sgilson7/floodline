//! The one door.
//!
//! Design §7: `World::apply` is the only way the world changes, and every rule
//! lives inside it — ownership above all, so a peer commanding another city is
//! rejected identically on every machine, whether it did so by bug, by desync
//! or by tampering. These are the tests for the rejections; a rule that is
//! enforced on one peer and not another is a desync in a disguise.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::{CitizenId, Job, PlayerId, State};
use sim::command::Command;
use sim::nav::{Dest, Nav};
use sim::world::{RuleError, World};
use sim::BuildingId;

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
    panic!("nowhere for a {kind:?}");
}

fn build(w: &mut World, p: u8, kind: Kind) -> BuildingId {
    let (x, y) = spot(w, p, kind);
    let id = w.place(PlayerId(p), kind, Facing::EastWest, x, y).unwrap();
    for g in Good::ALL {
        let want = kind.cost().get(g);
        if want > 0 {
            w.deliver_to(id, g, want);
        }
    }
    w.build_at(id, kind.build_ticks());
    id
}

/// Citizens of player 0 and of player 1.
fn two_cities() -> (World, CitizenId, CitizenId) {
    let w = World::new(31, 2);
    let mine = w.citizens.iter().find(|c| c.owner == PlayerId(0)).unwrap().id;
    let theirs = w.citizens.iter().find(|c| c.owner == PlayerId(1)).unwrap().id;
    (w, mine, theirs)
}

#[test]
fn a_player_cannot_command_another_players_citizens() {
    let (mut w, mine, theirs) = two_cities();
    let (x, y) = spot(&w, 0, Kind::Cottage);

    for cmd in [
        Command::MoveTo { citizens: vec![theirs], x: x as u8, y: y as u8 },
        Command::Unassign { citizens: vec![theirs] },
        Command::Assign { citizens: vec![theirs], building: BuildingId(0) },
        Command::SetHome { citizens: vec![theirs], cottage: BuildingId(0) },
    ] {
        assert_eq!(
            w.apply(PlayerId(0), &cmd),
            Err(RuleError::NotYours),
            "{cmd:?} was allowed"
        );
    }

    // And a command naming a mix of both is rejected whole, not applied in
    // part. That is the property that keeps two peers agreeing about what a
    // rejection meant.
    let before = w.citizens[mine.0 as usize].clone();
    assert_eq!(
        w.apply(
            PlayerId(0),
            &Command::MoveTo { citizens: vec![mine, theirs], x: x as u8, y: y as u8 }
        ),
        Err(RuleError::NotYours)
    );
    assert_eq!(w.citizens[mine.0 as usize], before, "half the command was applied");
}

#[test]
fn a_player_cannot_command_another_players_buildings() {
    let mut w = World::new(31, 2);
    let theirs = w.buildings.iter().find(|b| b.owner == PlayerId(1)).unwrap().id;

    assert_eq!(
        w.apply(PlayerId(0), &Command::Demolish { building: theirs }),
        Err(RuleError::NotYours)
    );
    assert_eq!(
        w.apply(PlayerId(0), &Command::RaiseDike { dike: theirs }),
        Err(RuleError::NotYours)
    );
}

#[test]
fn a_player_who_is_not_in_the_run_can_do_nothing() {
    let mut w = World::new(31, 2);
    assert_eq!(w.apply(PlayerId(7), &Command::Pause), Err(RuleError::NotYours));
    assert_eq!(w.apply(PlayerId(7), &Command::Ping { x: 1, y: 1 }), Err(RuleError::NotYours));
    assert!(!w.paused());
    assert!(w.pings.is_empty());
}

#[test]
fn commands_naming_things_that_are_not_there_are_refused() {
    let (mut w, mine, _) = two_cities();
    assert_eq!(
        w.apply(PlayerId(0), &Command::Demolish { building: BuildingId(999) }),
        Err(RuleError::NoSuchBuilding)
    );
    assert_eq!(
        w.apply(PlayerId(0), &Command::Unassign { citizens: vec![CitizenId(9999)] }),
        Err(RuleError::NoSuchCitizen)
    );
    assert_eq!(
        w.apply(PlayerId(0), &Command::MoveTo { citizens: vec![mine], x: 200, y: 5 }),
        Err(RuleError::NoSuchCell),
        "a cell off a 128-wide map"
    );
    assert_eq!(
        w.apply(PlayerId(0), &Command::Ping { x: 5, y: 200 }),
        Err(RuleError::NoSuchCell)
    );
}

#[test]
fn the_dead_take_no_orders() {
    let (mut w, mine, _) = two_cities();
    w.citizens[mine.0 as usize].die();
    assert_eq!(
        w.apply(PlayerId(0), &Command::MoveTo { citizens: vec![mine], x: 60, y: 60 }),
        Err(RuleError::NoSuchCitizen)
    );
}

#[test]
fn move_to_sends_them_and_drops_what_they_were_doing() {
    let (mut w, mine, _) = two_cities();
    let mut nav = Nav::new();
    let farm = build(&mut w, 0, Kind::Farm);

    w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine], building: farm })
        .unwrap();
    for _ in 0..50 {
        w.tick(&mut nav, &[]);
    }

    // "Get uphill" has to work even mid-errand — it is the one order that
    // matters during a flood.
    w.apply(PlayerId(0), &Command::MoveTo { citizens: vec![mine], x: 64, y: 64 })
        .unwrap();
    assert_eq!(w.citizens[mine.0 as usize].state, State::Walking);
    assert_eq!(w.citizens[mine.0 as usize].dest, Some(Dest::Cell(64, 64)));
    assert_eq!(w.citizens[mine.0 as usize].errand, None, "still on the old errand");
}

#[test]
fn assigning_picks_the_job_the_building_implies() {
    let mut w = World::new(31, 2);
    let mine: Vec<CitizenId> =
        w.citizens.iter().filter(|c| c.owner == PlayerId(0)).map(|c| c.id).take(4).collect();

    let farm = build(&mut w, 0, Kind::Farm);
    w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine[0]], building: farm })
        .unwrap();
    assert_eq!(w.citizens[mine[0].0 as usize].job, Some(Job::Farmer));
    assert_eq!(w.citizens[mine[0].0 as usize].workplace, Some(farm));

    let granary = build(&mut w, 0, Kind::Granary);
    w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine[1]], building: granary })
        .unwrap();
    assert_eq!(w.citizens[mine[1].0 as usize].job, Some(Job::Hauler));
    assert_eq!(
        w.citizens[mine[1].0 as usize].workplace, None,
        "a hauler is based nowhere: it goes where the work is"
    );

    // A site wants builders, whatever it is going to be.
    let (x, y) = spot(&w, 0, Kind::Cottage);
    let site = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();
    w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine[2]], building: site })
        .unwrap();
    assert_eq!(w.citizens[mine[2].0 as usize].job, Some(Job::Builder));

    // And there is no job at a dike.
    let dike = build(&mut w, 0, Kind::Dike);
    assert_eq!(
        w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine[3]], building: dike }),
        Err(RuleError::NoJobThere)
    );
}

#[test]
fn a_farm_takes_only_as_many_farmers_as_it_has_slots() {
    let mut w = World::new(31, 2);
    let farm = build(&mut w, 0, Kind::Farm);
    let mine: Vec<CitizenId> =
        w.citizens.iter().filter(|c| c.owner == PlayerId(0)).map(|c| c.id).collect();
    let slots = Kind::Farm.job_slots();

    let fits: Vec<CitizenId> = mine[..slots].to_vec();
    w.apply(PlayerId(0), &Command::Assign { citizens: fits, building: farm }).unwrap();

    assert_eq!(
        w.apply(
            PlayerId(0),
            &Command::Assign { citizens: vec![mine[slots]], building: farm }
        ),
        Err(RuleError::Full),
        "a fourth farmer squeezed into three slots"
    );
    // Too many at once is refused too, rather than filling what fits.
    let mut w2 = World::new(31, 2);
    let farm2 = build(&mut w2, 0, Kind::Farm);
    assert_eq!(
        w2.apply(
            PlayerId(0),
            &Command::Assign { citizens: mine[..slots + 1].to_vec(), building: farm2 }
        ),
        Err(RuleError::Full)
    );
    assert!(w2.citizens.iter().all(|c| c.job.is_none()), "part of it was applied");
}

#[test]
fn reassigning_the_same_citizens_is_not_treated_as_crowding() {
    let mut w = World::new(31, 2);
    let farm = build(&mut w, 0, Kind::Farm);
    let mine: Vec<CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == PlayerId(0))
        .map(|c| c.id)
        .take(Kind::Farm.job_slots())
        .collect();

    w.apply(PlayerId(0), &Command::Assign { citizens: mine.clone(), building: farm })
        .unwrap();
    // The same three again. They already hold the slots they would be given.
    w.apply(PlayerId(0), &Command::Assign { citizens: mine, building: farm }).unwrap();
}

#[test]
fn unassigning_puts_them_back_to_hauling_and_off_the_roster() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let farm = build(&mut w, 0, Kind::Farm);
    let mine = w.citizens.iter().find(|c| c.owner == PlayerId(0)).unwrap().id;

    w.apply(PlayerId(0), &Command::Assign { citizens: vec![mine], building: farm })
        .unwrap();
    for _ in 0..400 {
        w.tick(&mut nav, &[]);
    }
    assert!(w.buildings[farm.0 as usize].workers.contains(&mine), "never got to work");

    w.apply(PlayerId(0), &Command::Unassign { citizens: vec![mine] }).unwrap();
    assert_eq!(w.citizens[mine.0 as usize].job, None);
    assert!(
        !w.buildings[farm.0 as usize].workers.contains(&mine),
        "still on the farm's roster after being taken off the job"
    );
}

#[test]
fn set_home_respects_the_number_of_beds() {
    let mut w = World::new(31, 2);
    let cottage = build(&mut w, 0, Kind::Cottage);
    let mine: Vec<CitizenId> =
        w.citizens.iter().filter(|c| c.owner == PlayerId(0)).map(|c| c.id).collect();
    let beds = Kind::Cottage.beds();

    w.apply(
        PlayerId(0),
        &Command::SetHome { citizens: mine[..beds].to_vec(), cottage },
    )
    .unwrap();
    assert_eq!(
        w.apply(PlayerId(0), &Command::SetHome { citizens: vec![mine[beds]], cottage }),
        Err(RuleError::Full)
    );

    // And a cottage is the only thing you can live in.
    let granary = build(&mut w, 0, Kind::Granary);
    assert_eq!(
        w.apply(
            PlayerId(0),
            &Command::SetHome { citizens: vec![mine[beds]], cottage: granary }
        ),
        Err(RuleError::NotACottage)
    );
}

#[test]
fn a_ping_lands_on_a_tick_and_fades() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    w.tick(&mut nav, &[(PlayerId(1), Command::Ping { x: 40, y: 40 })]);

    assert_eq!(w.pings.len(), 1);
    assert_eq!(w.pings[0].by, PlayerId(1));
    assert_eq!((w.pings[0].x, w.pings[0].y), (40, 40));

    for _ in 0..PING_LIFETIME {
        w.tick(&mut nav, &[]);
    }
    assert!(w.pings.is_empty(), "a ping outlived its welcome");
}

#[test]
fn a_pause_needs_everyone_to_lift_it() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let before = w.checksum();

    w.apply(PlayerId(0), &Command::Pause).unwrap();
    w.apply(PlayerId(1), &Command::Pause).unwrap();
    assert!(w.paused());

    for _ in 0..100 {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(w.tick, 0, "the clock ran while the game was paused");

    // One player resuming is not enough (design §6).
    w.apply(PlayerId(0), &Command::Resume).unwrap();
    assert!(w.paused(), "one player lifted everybody's pause");
    w.tick(&mut nav, &[]);
    assert_eq!(w.tick, 0);

    w.apply(PlayerId(1), &Command::Resume).unwrap();
    assert!(!w.paused());
    w.tick(&mut nav, &[]);
    assert_eq!(w.tick, 1);

    // Pausing twice is not two pauses to lift.
    w.apply(PlayerId(0), &Command::Pause).unwrap();
    w.apply(PlayerId(0), &Command::Pause).unwrap();
    w.apply(PlayerId(0), &Command::Resume).unwrap();
    assert!(!w.paused());

    let _ = before;
}

#[test]
fn commands_arriving_through_tick_do_what_they_would_alone() {
    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let (x, y) = spot(&w, 0, Kind::Cottage);

    w.tick(
        &mut nav,
        &[(PlayerId(0), Command::Place { kind: Kind::Cottage, facing: Facing::EastWest, x: x as u8, y: y as u8 })],
    );
    assert!(w.building_at(x, y).is_some(), "the command never arrived");

    // A rejected one changes nothing and does not stop the tick.
    let t = w.tick;
    w.tick(
        &mut nav,
        &[(PlayerId(1), Command::Place { kind: Kind::Cottage, facing: Facing::EastWest, x: x as u8, y: y as u8 })],
    );
    assert_eq!(w.tick, t + 1, "a rejected command stopped the world");
    assert_eq!(w.building_at(x, y).map(|b| b.owner), Some(PlayerId(0)));
}

#[test]
fn a_rejected_command_leaves_the_world_byte_identical() {
    // The property the lockstep rests on: every peer rejects the same command
    // for the same reason and none of them is left slightly different.
    let mut w = World::new(31, 2);
    let theirs = w.citizens.iter().find(|c| c.owner == PlayerId(1)).unwrap().id;
    let before = w.checksum();

    for cmd in [
        Command::MoveTo { citizens: vec![theirs], x: 64, y: 64 },
        Command::Demolish { building: BuildingId(500) },
        Command::Place { kind: Kind::Hearth, facing: Facing::EastWest, x: 10, y: 10 },
        Command::Assign { citizens: vec![theirs], building: BuildingId(0) },
        Command::Ping { x: 250, y: 250 },
        Command::RaiseDike { dike: BuildingId(0) },
    ] {
        assert!(w.apply(PlayerId(0), &cmd).is_err(), "{cmd:?} was allowed");
        assert_eq!(w.checksum(), before, "{cmd:?} changed the world before failing");
    }
}

#[test]
fn every_refusal_says_something_a_player_can_read() {
    // The list is the enum, written out, so a rule added without a sentence
    // fails here rather than reaching a player as silence.
    use sim::world::RuleError::*;
    let all = [
        NotYours, OffMap, Occupied, WrongGround, OneHearthOnly, NoSuchBuilding, NoSuchCitizen,
        NotStanding, TooHigh, NoJobThere, Full, NotACottage, NoSuchCell, NoRoute, NoSuchRoad,
        NoSuchTrade, NotYourRoad, NoSuchPartner, AlreadyAccepted, NoRockHere,
    ];
    let mut seen: Vec<&str> = Vec::new();
    for e in &all {
        let text = e.to_message();
        assert!(!text.is_empty(), "{e:?} has nothing to say");
        // macroquad's built-in font is ASCII and draws a hollow box for
        // anything else; `gui` lints its own strings for this and cannot see
        // these, which are drawn under the map exactly the same way.
        assert!(text.is_ascii(), "{e:?} is outside ASCII and will draw boxes: {text:?}");
        assert!(
            text.chars().next().unwrap().is_lowercase(),
            "{e:?} reads like a heading, not a line under the cursor: {text:?}"
        );
        assert!(text.len() < 44, "{e:?} is too long for the panel: {text:?}");
        assert!(!seen.contains(&text), "{e:?} says the same as another: {text:?}");
        seen.push(text);
    }

    // And a real refusal comes back with one attached.
    let mut w = sim::World::new(7, 2);
    let err = w.apply(sim::PlayerId(0), &sim::Command::Place {
        kind: sim::building::Kind::Hearth, facing: Facing::EastWest, x: 10, y: 10,
    }).unwrap_err();
    assert_eq!(err.to_message(), "one hearth to a city");
}

#[test]
fn people_told_to_go_somewhere_stay_there() {
    // Design §3.2 calls "get uphill" the one order that matters during a
    // flood, and it did not work: a citizen that reached where it was sent
    // arrived with nothing to do, `find_work` gave it something, and it walked
    // back to the farm it had been told to leave — inside a tick, and a day
    // before the water came. The order stands until the player says otherwise.
    use sim::building::Kind;
    use sim::nav::Nav;
    use sim::{Command, PlayerId, World};

    let mut w = World::new(31, 2);
    let mut nav = Nav::new();
    let me = PlayerId(0);
    let (hx, hy) = w.map.hearth_sites[0];

    // Somewhere to work, so there is something to be pulled back to.
    'found: for r in 3..30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Granary, Facing::EastWest, x, y).is_ok() {
                    w.apply(me, &Command::Place { kind: Kind::Granary, facing: Facing::EastWest, x: x as u8, y: y as u8 })
                        .unwrap();
                    break 'found;
                }
            }
        }
    }

    let mine: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me).map(|c| c.id).collect();
    // Somewhere they can actually stand, about fifteen cells off, and nowhere
    // any errand would take them.
    let far = {
        let mut found = None;
        'ring: for r in 14..24i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    // Off the edges: a flow field seeded in the last column
                    // does not always reach a citizen standing in it, which is
                    // a fact about the map's border and not about the order.
                    if x < 6 || y < 6 || x > 121 || y > 121 {
                        continue;
                    }
                    if w.map.buildable(x, y) && w.building_at(x, y).is_none() {
                        found = Some((x as u8, y as u8));
                        break 'ring;
                    }
                }
            }
        }
        found.expect("nowhere within twenty cells of the hearth to stand")
    };
    w.apply(me, &Command::MoveTo { citizens: mine.clone(), x: far.0, y: far.1 }).unwrap();

    // Where they settle, and then that they stay settled. Asserted as "they
    // stopped moving" rather than "they reached that exact cell" because the
    // cell picked above may be rock or shallows, and a flow field cannot take
    // anybody to a cell nobody can stand on — which is a fact about the map
    // and not about the order.
    for _ in 0..600 {
        w.tick(&mut nav, &[]);
    }
    let settled: Vec<(i32, i32)> = mine.iter().map(|id| w.citizens[id.0 as usize].pos.cell()).collect();
    for _ in 0..600 {
        w.tick(&mut nav, &[]);
    }
    for (n, id) in mine.iter().enumerate() {
        let c = &w.citizens[id.0 as usize];
        let (x, y) = c.pos.cell();
        let d = (x - settled[n].0).abs() + (y - settled[n].1).abs();
        assert!(
            d <= 1,
            "#{} was sent somewhere and then wandered {d} cells away again: \
             job {:?} errand {:?}",
            id.0,
            c.job,
            c.errand
        );
        assert!(c.held, "#{} is no longer holding its ground", id.0);
        // And they went at least somewhere: the order was carried out.
        assert!(
            (x - hx).abs() + (y - hy).abs() > 6,
            "#{} never left the hearth",
            id.0
        );
    }

    // And "back to hauling" releases them.
    w.apply(me, &Command::Unassign { citizens: mine.clone() }).unwrap();
    assert!(w.citizens[mine[0].0 as usize].held == false);
    for _ in 0..400 {
        w.tick(&mut nav, &[]);
    }
    let moved = mine.iter().enumerate().any(|(n, id)| {
        let c = &w.citizens[id.0 as usize];
        let (x, y) = c.pos.cell();
        (x - settled[n].0).abs() + (y - settled[n].1).abs() > 4
    });
    assert!(moved, "nobody went back to work after being released");
}

#[test]
fn a_building_says_how_many_it_will_take_before_the_command_is_refused() {
    // The bug this exists to stop: select a whole city of eight, right-click a
    // farm, and `Assign` answers `Full` — not "three of them start farming",
    // but nobody does anything at all, because a command is all-or-nothing.
    // The only sign is a line under the map that fades after three seconds,
    // and the city starves on day four with an empty farm standing in it.
    use sim::building::Kind;
    use sim::{Command, PlayerId, World};

    let mut w = World::new(31, 2);
    let me = PlayerId(0);
    let (hx, hy) = w.map.hearth_sites[0];

    let put = |w: &mut World, kind: Kind| {
        for r in 3..30i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    if w.can_place(me, kind, Facing::EastWest, x, y).is_ok() {
                        w.apply(me, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).unwrap();
                        return w.buildings.last().unwrap().id;
                    }
                }
            }
        }
        panic!("nowhere for a {kind:?}");
    };
    let farm = put(&mut w, Kind::Farm);
    let cottage = put(&mut w, Kind::Cottage);

    let all: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me).map(|c| c.id).collect();
    assert_eq!(all.len(), sim::balance::FOUNDING_CITIZENS as usize);

    // A site wants builders, and there are four builder slots.
    assert_eq!(w.will_take(me, farm, &all), sim::balance::BUILDER_SLOTS);
    // A cottage sleeps four.
    assert_eq!(w.will_house(me, cottage, &all), 0, "it is not built yet");

    // Finish them both.
    for id in [farm, cottage] {
        for g in sim::building::Good::ALL {
            let want = w.buildings[id.0 as usize].outstanding().get(g);
            if want > 0 {
                w.deliver_to(id, g, want);
            }
        }
        let ticks = w.buildings[id.0 as usize].kind.build_ticks();
        assert!(w.build_at(id, ticks));
    }

    // Now the farm has three job slots and the cottage four beds.
    assert_eq!(w.will_take(me, farm, &all), 3);
    assert_eq!(w.will_house(me, cottage, &all), 4);

    // And what it says fits, fits — where the whole selection does not.
    assert!(w.apply(me, &Command::Assign { citizens: all.clone(), building: farm }).is_err());
    let three: Vec<sim::CitizenId> = all.iter().copied().take(3).collect();
    w.apply(me, &Command::Assign { citizens: three.clone(), building: farm }).unwrap();

    // Slots already held by somebody not in the list are gone from the budget.
    let others: Vec<sim::CitizenId> = all.iter().copied().skip(3).collect();
    assert_eq!(w.will_take(me, farm, &others), 0, "three farmers already hold all three");
    // But naming the people who are already there frees their own slots again.
    assert_eq!(w.will_take(me, farm, &all), 3);

    // Somebody else's building takes nobody, and neither does a hole.
    assert_eq!(w.will_take(PlayerId(1), farm, &all), 0);
    assert_eq!(w.will_take(me, sim::BuildingId(999), &all), 0);
}

#[test]
fn a_building_is_where_it_was_clicked() {
    // M12.4, reproduced before anything is fixed. City 0 placed a forester
    // with `click-cell 75 97`; `right-click-cell 75 97` did nothing at all -
    // no refusal, no message, the people went idle - and `76 98` worked
    // instantly. Two game-days lost with a forester standing empty while the
    // amber line said "nobody is cutting wood".
    //
    // The suspect: `Building::footprint`'s origin is the *top-left* cell. If
    // `place` stores the clicked cell as the origin while the ghost is drawn
    // centred on the cursor, the building lands up and left of where the
    // player aimed and `building_at(clicked)` finds nothing.
    use sim::building::{Facing, Kind};
    use sim::command::Command;
    use sim::world::World;
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut missed: Vec<String> = Vec::new();

    for kind in Kind::ALL {
        if matches!(kind, Kind::Hearth) {
            continue;
        }
        for facing in [Facing::EastWest, Facing::NorthSouth] {
            let mut w = World::new(31, 2);
            let (hx, hy) = w.map.hearth_sites[0];
            let mut put = None;
            'ring: for r in 3..40i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        let (x, y) = (hx + dx, hy + dy);
                        if w.can_place(me, kind, facing, x, y).is_ok() {
                            w.apply(
                                me,
                                &Command::Place {
                                    kind,
                                    facing,
                                    x: x as u8,
                                    y: y as u8,
                                },
                            )
                            .unwrap();
                            put = Some((x, y));
                            break 'ring;
                        }
                    }
                }
            }
            let Some((x, y)) = put else { continue };
            match w.building_at(x, y) {
                Some(b) if b.kind == kind => {}
                other => missed.push(format!(
                    "{kind:?} {facing:?} placed at ({x},{y}) is not there: {:?}",
                    other.map(|b| b.kind)
                )),
            }
        }
    }

    assert!(
        missed.is_empty(),
        "a click that placed a building did not put it under the click:\n  {}",
        missed.join("\n  ")
    );
}

#[test]
fn a_refusal_says_which_problem_it_is() {
    // Two refusals lied, and both sent an M11.9 player looking for the wrong
    // thing. A cottage that was still a construction site said **"no beds left
    // there"** - which is what a *full* cottage says - and a nursery said
    // **"no room left there"**, when a nursery is not a workplace at all and
    // has no capacity to run out of.
    //
    // Neither was a missing word. `RuleError` already had "it is not built
    // yet" and "there is no work there"; the panel had its own sentence for
    // every kind of nought and used it for all of them. `room_for` and
    // `beds_for` are the same questions `will_take` and `will_house` ask, with
    // the reason kept.
    use sim::building::{Facing, Kind};
    use sim::command::Command;
    use sim::world::{RuleError, World};
    use sim::PlayerId;

    let me = PlayerId(0);
    let them = PlayerId(1);
    let mut w = World::new(31, 2);
    let mine: Vec<sim::CitizenId> =
        w.citizens.iter().filter(|c| c.owner == me && !c.is_child()).map(|c| c.id).take(2).collect();

    let place = |w: &mut World, kind: Kind| -> sim::building::BuildingId {
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
                        return w.buildings.last().unwrap().id;
                    }
                }
            }
        }
        panic!("nowhere for a {kind:?}");
    };

    // A cottage that is not built yet is not a full cottage.
    let cottage = place(&mut w, Kind::Cottage);
    assert_eq!(
        w.beds_for(me, cottage, &mine),
        Err(RuleError::NotStanding),
        "an unbuilt cottage should say it is not built yet"
    );
    assert_eq!(RuleError::NotStanding.to_message(), "it is not built yet");

    // Built, and it has beds.
    for g in sim::building::Good::ALL {
        let want = w.buildings[cottage.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(cottage, g, want);
        }
    }
    assert!(w.build_at(cottage, Kind::Cottage.build_ticks()));
    assert!(w.beds_for(me, cottage, &mine).is_ok(), "a built cottage has beds");

    // A nursery is not a workplace, and does not have "no room".
    let nursery = place(&mut w, Kind::Nursery);
    for g in sim::building::Good::ALL {
        let want = w.buildings[nursery.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(nursery, g, want);
        }
    }
    assert!(w.build_at(nursery, Kind::Nursery.build_ticks()));
    assert_eq!(
        w.room_for(me, nursery, &mine),
        Err(RuleError::NoJobThere),
        "a nursery is not a workplace and should say so"
    );
    assert_eq!(RuleError::NoJobThere.to_message(), "there is no work there");

    // A full building is the only thing that says there is no room.
    let quarry = place(&mut w, Kind::Quarry);
    for g in sim::building::Good::ALL {
        let want = w.buildings[quarry.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(quarry, g, want);
        }
    }
    assert!(w.build_at(quarry, Kind::Quarry.build_ticks()));
    w.apply(me, &Command::Assign { citizens: mine.clone(), building: quarry }).unwrap();
    let more: Vec<sim::CitizenId> = w
        .citizens
        .iter()
        .filter(|c| c.owner == me && !c.is_child() && !mine.contains(&c.id))
        .map(|c| c.id)
        .take(1)
        .collect();
    assert_eq!(
        w.room_for(me, quarry, &more),
        Err(RuleError::Full),
        "a quarry with both slots taken is the one case that *is* no room"
    );
    assert_eq!(RuleError::Full.to_message(), "there is no room");

    // And somebody else's building is neither.
    assert_eq!(w.room_for(them, quarry, &more), Err(RuleError::NotYours));
}

#[test]
fn a_city_with_nobody_left_cannot_build() {
    // City 0 in the M12 three-player run, with all eight of its people dead:
    // *"I can still build. With zero citizens I pressed 1 and dropped a
    // cottage, then pressed 7 and dragged a twelve-cell dike. Both went down.
    // Both took my treasury."* It was then told a stretch of its wall had
    // given way.
    //
    // Materials are hauled and sites are raised by people, so a city of nought
    // that orders a building has ordered something that can never happen.
    //
    // This is not reachable while the whole *run* is over - `main.rs` draws a
    // score screen instead of the panel - but a city that dies while its
    // neighbours play on keeps a full panel, and at two to six seats that is
    // an ordinary state.
    use sim::building::{Facing, Kind};
    use sim::command::Command;
    use sim::world::{RuleError, World};
    use sim::PlayerId;

    let me = PlayerId(0);
    let mut w = World::new(31, 3);
    let (hx, hy) = w.map.hearth_sites[0];

    // Somewhere this city could build, while it still has people.
    let mut spot = None;
    'ring: for r in 3..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(me, Kind::Cottage, Facing::EastWest, x, y).is_ok() {
                    spot = Some((x, y));
                    break 'ring;
                }
            }
        }
    }
    let (x, y) = spot.expect("nowhere for a cottage");

    // The city dies. Its neighbours play on, so nothing else changes.
    for c in w.citizens.iter_mut().filter(|c| c.owner == me) {
        c.food = 0;
        c.die();
    }
    assert_eq!(w.population(me), 0);
    assert!(w.finished().is_none(), "the run should not be over: two cities are alive");

    let before = w.treasury(me);
    assert_eq!(
        w.can_place(me, Kind::Cottage, Facing::EastWest, x, y),
        Err(RuleError::NobodyLeft),
        "a city with nobody left was allowed to order a building"
    );
    assert!(w
        .apply(
            me,
            &Command::Place { kind: Kind::Cottage, facing: Facing::EastWest, x: x as u8, y: y as u8 }
        )
        .is_err());
    assert_eq!(w.treasury(me), before, "it was charged for a building nobody can raise");
    assert_eq!(RuleError::NobodyLeft.to_message(), "there is nobody left to do it");

    // And a city that still has somebody is unaffected.
    let them = PlayerId(1);
    let (tx, ty) = w.map.hearth_sites[1];
    let mut ok = false;
    'ring2: for r in 3..40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if w.can_place(them, Kind::Cottage, Facing::EastWest, tx + dx, ty + dy).is_ok() {
                    ok = true;
                    break 'ring2;
                }
            }
        }
    }
    assert!(ok, "a living city was refused as well");
}
