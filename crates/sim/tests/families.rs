//! Households, children, and the nursery that decides whether there are any.
//!
//! Design §3.2's one line — "two adult citizens sharing a cottage for a day
//! become a household" — and the two rules that hang off it: being fed is the
//! gate, and no nursery means no children.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::{CitizenId, PlayerId};
use sim::command::Command;
use sim::nav::Nav;
use sim::world::{RuleError, World};
use sim::BuildingId;

const ME: PlayerId = PlayerId(0);

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

/// A city with a cottage two people share, a nursery, and food in the larder.
fn a_family() -> (World, Nav, BuildingId, BuildingId) {
    let mut w = World::new(31, 2);
    let cottage = build(&mut w, Kind::Cottage);
    let nursery = build(&mut w, Kind::Nursery);
    let granary = build(&mut w, Kind::Granary);
    w.buildings[granary.0 as usize].store.add(Good::Food, 400);

    w.apply(
        ME,
        &Command::SetHome { citizens: vec![CitizenId(0), CitizenId(1)], cottage },
    )
    .unwrap();
    (w, Nav::new(), cottage, nursery)
}

/// Keep everybody fed and rested, so the test is about families and not about
/// whether a city of eight can feed itself.
fn keep(w: &mut World) {
    for c in &mut w.citizens {
        if c.alive() {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
    }
}

fn run(w: &mut World, nav: &mut Nav, ticks: u32) {
    for _ in 0..ticks {
        keep(w);
        w.tick(nav, &[]);
    }
}

fn mine(w: &World) -> Vec<&sim::Household> {
    w.households.iter().filter(|h| h.owner == ME && h.alive()).collect()
}

#[test]
fn two_adults_sharing_a_cottage_become_a_household_after_a_day() {
    let (mut w, mut nav, cottage, _) = a_family();
    run(&mut w, &mut nav, 10);
    assert_eq!(mine(&w).len(), 1, "the two of them are not a household");
    assert!(!mine(&w)[0].settled(), "and not settled after ten ticks");
    assert_eq!(mine(&w)[0].members, [CitizenId(0), CitizenId(1)]);
    assert_eq!(mine(&w)[0].cottage, cottage);

    run(&mut w, &mut nav, TICKS_PER_DAY);
    assert!(mine(&w)[0].settled(), "a day of sharing a cottage is a household");
}

#[test]
fn a_hungry_city_does_not_grow() {
    // Being fed is the whole gate, which is what makes the granary decide the
    // *size* of a village rather than only whether it survives.
    let (mut w, mut nav, _, _) = a_family();
    for _ in 0..TICKS_PER_DAY / 2 {
        keep(&mut w);
        w.tick(&mut nav, &[]);
    }
    let along = mine(&w)[0].together;
    assert!(along > 0);

    // Half a day of hunger.
    for _ in 0..10 {
        for c in &mut w.citizens {
            c.food = 0;
        }
        w.tick(&mut nav, &[]);
    }
    assert_eq!(
        mine(&w)[0].together,
        0,
        "a hungry day paused the progress instead of losing it"
    );
}

#[test]
fn a_fed_household_with_a_nursery_has_a_child() {
    let (mut w, mut nav, _, nursery) = a_family();
    let grown = w.citizens.len();
    run(&mut w, &mut nav, TICKS_PER_DAY + CHILD_TICKS + 10);

    assert!(w.citizens.len() > grown, "no child was born");
    let child = &w.citizens[grown];
    assert!(child.is_child(), "the newcomer is not a child");
    assert_eq!(child.owner, ME);
    assert_eq!(child.nursery, Some(nursery), "a child is kept at the nursery");
    assert_eq!(mine(&w)[0].children, vec![child.id]);

    // And it is not a worker.
    assert_eq!(
        w.apply(ME, &Command::Assign { citizens: vec![child.id], building: nursery }),
        Err(RuleError::TooYoung)
    );
}

#[test]
fn no_nursery_no_children() {
    // The rule that makes growing a city a decision rather than something that
    // happens to a player.
    let mut w = World::new(31, 2);
    let cottage = build(&mut w, Kind::Cottage);
    let granary = build(&mut w, Kind::Granary);
    w.buildings[granary.0 as usize].store.add(Good::Food, 400);
    w.apply(ME, &Command::SetHome { citizens: vec![CitizenId(0), CitizenId(1)], cottage })
        .unwrap();
    let mut nav = Nav::new();
    let grown = w.citizens.len();

    run(&mut w, &mut nav, TICKS_PER_DAY + CHILD_TICKS * 2);
    assert!(mine(&w)[0].settled(), "they did settle");
    assert_eq!(w.citizens.len(), grown, "a child was born with nowhere to keep it");
}

#[test]
fn a_full_nursery_holds_no_more() {
    let (mut w, mut nav, _, nursery) = a_family();
    // A cottage with room for everybody, so beds are not the limit under test.
    let cottage = mine(&w).first().map(|h| h.cottage);
    let _ = cottage;
    for _ in 0..sim::balance::NURSERY_PLACES + 2 {
        run(&mut w, &mut nav, CHILD_TICKS + TICKS_PER_DAY);
    }
    let kept = w
        .citizens
        .iter()
        .filter(|c| c.alive() && c.nursery == Some(nursery))
        .count();
    assert!(
        kept <= w.buildings[nursery.0 as usize].places(),
        "{kept} children in a nursery with {} places",
        w.buildings[nursery.0 as usize].places()
    );
}

#[test]
fn a_child_comes_of_age_and_goes_to_work() {
    let (mut w, mut nav, _, _) = a_family();
    let grown = w.citizens.len();
    run(&mut w, &mut nav, TICKS_PER_DAY + CHILD_TICKS + 10);
    assert!(w.citizens.len() > grown, "no child was born");
    let child = w.citizens[grown].id;
    assert!(w.citizens[child.0 as usize].is_child());

    run(&mut w, &mut nav, COMING_OF_AGE);
    assert!(!w.citizens[child.0 as usize].is_child(), "it never grew up");
    assert_eq!(w.citizens[child.0 as usize].nursery, None, "and it left the nursery");

    let farm = build(&mut w, Kind::Farm);
    w.apply(ME, &Command::Assign { citizens: vec![child], building: farm }).unwrap();
}

#[test]
fn a_household_ends_when_its_cottage_does() {
    let (mut w, mut nav, cottage, _) = a_family();
    run(&mut w, &mut nav, TICKS_PER_DAY + 10);
    assert!(!mine(&w).is_empty());

    w.damage_building(cottage, Kind::Cottage.integrity());
    run(&mut w, &mut nav, 2);
    assert!(mine(&w).is_empty(), "a household outlived its house");
    // Ended, not removed: an id means one thing for a whole run.
    assert_eq!(w.households.len(), 1);
}

#[test]
fn two_peers_raise_the_same_children() {
    // Births are the first thing in the game that adds to `World::citizens`
    // while it is running, and ids are indices into that vector in half a
    // dozen places. If two peers ever appended in a different order, or a
    // different number of times, this is what says so.
    let script = |w: &mut World| {
        let cottage = build(w, Kind::Cottage);
        build(w, Kind::Nursery);
        let granary = build(w, Kind::Granary);
        w.buildings[granary.0 as usize].store.add(Good::Food, 2000);
        w.apply(ME, &Command::SetHome { citizens: vec![CitizenId(0), CitizenId(1)], cottage })
            .unwrap();
    };
    let mut a = World::new(31, 2);
    let mut b = World::new(31, 2);
    script(&mut a);
    script(&mut b);
    let (mut na, mut nb) = (Nav::new(), Nav::new());

    let grown = a.citizens.len();
    for t in 0..10_000u32 {
        keep(&mut a);
        keep(&mut b);
        a.tick(&mut na, &[]);
        b.tick(&mut nb, &[]);
        if t % 500 == 0 {
            assert_eq!(a.checksum(), b.checksum(), "the two worlds parted at tick {t}");
        }
    }
    assert_eq!(a.checksum(), b.checksum());
    assert!(a.citizens.len() > grown, "nobody was born, so nothing was checked");
    assert!(
        a.citizens.iter().any(|c| !c.is_child()) && a.citizens[grown].is_child()
            || a.citizens[grown].age == sim::Age::Adult,
        "the first child neither stayed a child nor grew up"
    );
}

/// How a city grows, fed and unfed, over three ages.
///
/// A measurement, not an assertion: `CHILD_TICKS` and `COMING_OF_AGE` are set
/// from this. What is being looked for is growth a player can watch without
/// the population running away from the food that caused it.
#[test]
#[ignore]
fn how_a_city_grows() {
    println!();
    println!("  the household timer is {CHILD_TICKS} ticks and coming of age is {COMING_OF_AGE}");
    println!();
    println!("  fed?     seed   day 6   day 12   day 18   children   grown   nurseries");

    for fed in [true, false] {
        for seed in [31u64, 97, 1_000_003] {
            let mut w = World::new(seed, 2);
            let cottage = build(&mut w, Kind::Cottage);
            let second = build(&mut w, Kind::Cottage);
            let nursery = build(&mut w, Kind::Nursery);
            let granary = build(&mut w, Kind::Granary);
            w.buildings[granary.0 as usize].store.add(Good::Food, 30_000);
            w.apply(
                ME,
                &Command::SetHome { citizens: vec![CitizenId(0), CitizenId(1)], cottage },
            )
            .unwrap();
            w.apply(
                ME,
                &Command::SetHome { citizens: vec![CitizenId(2), CitizenId(3)], cottage: second },
            )
            .unwrap();

            let mut nav = Nav::new();
            let mut marks = Vec::new();
            for day in 1..=18u32 {
                for _ in 0..TICKS_PER_DAY {
                    if fed {
                        keep(&mut w);
                    } else {
                        // Fed enough to live, never enough to grow.
                        for c in &mut w.citizens {
                            if c.alive() {
                                c.food = CHILD_FOOD - 1;
                                c.rest = NEED_FULL;
                            }
                        }
                    }
                    w.tick(&mut nav, &[]);
                }
                if day % 6 == 0 {
                    marks.push(w.population(ME));
                }
            }
            let children =
                w.citizens.iter().filter(|c| c.alive() && c.is_child()).count();
            // Saturating, because the floods in a three-age run take people:
            // this counts the ones who grew up *net* of the ones who drowned,
            // which is the number a player actually has.
            let grown = w
                .citizens
                .iter()
                .filter(|c| c.alive() && !c.is_child())
                .count()
                .saturating_sub(FOUNDING_CITIZENS as usize);
            println!(
                "  {:<5}   {seed:>6}   {:>5}   {:>6}   {:>6}   {:>8}   {:>5}   {:>9}",
                if fed { "yes" } else { "no" },
                marks[0],
                marks[1],
                marks[2],
                children,
                grown,
                w.buildings[nursery.0 as usize].places(),
            );
        }
    }
}

/// A household in an ordinary city settles, without anybody force-feeding it.
///
/// The regression guard for the bug M11.9's playtest found. `CHILD_FOOD` was
/// `FED_ENOUGH`, which is the level a citizen *stops eating* at, so both
/// members had to hold a level they were above for one tick in every cycle —
/// for `TICKS_PER_DAY` consecutive ticks. No household ever got past 99 of the
/// 1 200 it needed and no city in three playtests ever grew past eight.
///
/// `how_a_city_grows` sets `c.food = NEED_FULL` every tick and so has never
/// been able to see this. Nothing here feeds anybody: the city eats the way a
/// played city eats.
#[test]
fn a_household_in_a_fed_city_settles_without_being_force_fed() {
    let (mut w, mut nav, _, _) = a_family();
    let mut settled_at = None;
    for t in 0..3 * TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        if settled_at.is_none() && w.households.iter().any(|h| h.alive() && h.settled()) {
            settled_at = Some(t);
        }
    }
    let at = settled_at.expect(
        "no household settled in three days of an ordinary fed city -          CHILD_FOOD is probably above the trough of a citizen's food cycle again",
    );
    assert!(
        at < 2 * TICKS_PER_DAY,
        "settling took {at} ticks, which is more than the day it is meant to"
    );
}

/// What a household in an ordinary, well-fed city actually manages.
///
/// M11.9's directed playtest asked two players to grow a city above eight.
/// Neither could: both built cottages, a nursery and a full granary, both
/// formed households, and both watched them read "settling in" for four days
/// and never bear a child. Three playtests in, no city has ever exceeded the
/// eight it was founded with.
///
/// `how_a_city_grows` says growth works, and it does — but it force-feeds
/// `c.food = NEED_FULL` every tick, so it has never tested the condition a
/// played city has to meet. This measures that condition instead.
#[test]
#[ignore]
fn what_a_fed_household_actually_manages() {
    let (mut w, mut nav, _, _) = a_family();
    let mut lowest = u16::MAX;
    let mut highest = 0u16;
    let mut best_together = 0u32;

    for _ in 0..4 * TICKS_PER_DAY {
        w.tick(&mut nav, &[]);
        for c in w.citizens.iter().filter(|c| c.alive() && !c.is_child()) {
            lowest = lowest.min(c.food);
            highest = highest.max(c.food);
        }
        for h in w.households.iter().filter(|h| h.alive()) {
            best_together = best_together.max(h.together);
        }
    }

    println!("  a citizen's food over four days: {lowest} to {highest}");
    println!("  CHILD_FOOD is {CHILD_FOOD}, and FED_ENOUGH {FED_ENOUGH}");
    println!("  the best any household managed: together = {best_together}");
    println!("  it needs {TICKS_PER_DAY} consecutive ticks to settle");
    println!(
        "  households {}, children {}",
        w.households.len(),
        w.citizens.iter().filter(|c| c.is_child()).count()
    );
}
