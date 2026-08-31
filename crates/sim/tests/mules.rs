//! Trade on the road: the trading post, the trader, and the cart it sends out.
//!
//! Design §6 says trade is barter along a road you can watch, and it stays
//! that: `Command::Trade` is untouched. This is the other half M6 asked for —
//! a post whose mules carry wood to another city and come back with gold —
//! and gold is a deliberate departure from §6's "no currency in version one".

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::{CitizenId, Job, PlayerId};
use sim::command::Command;
use sim::mule::Leg;
use sim::nav::Nav;
use sim::world::{RuleError, World};
use sim::BuildingId;

const ME: PlayerId = PlayerId(0);

/// A world with a standing trading post beside the first city, and the
/// citizens to man it.
fn with_a_post() -> (World, Nav, BuildingId) {
    let mut w = World::new(31, 2);
    let (hx, hy) = w.map.hearth_sites[0];
    let mut post = None;
    'ring: for r in 3..30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if w.can_place(ME, Kind::TradingPost, Facing::EastWest, x, y).is_ok() {
                    post = Some(w.place(ME, Kind::TradingPost, Facing::EastWest, x, y).unwrap());
                    break 'ring;
                }
            }
        }
    }
    let post = post.expect("nowhere for a trading post");
    for g in Good::ALL {
        let want = w.buildings[post.0 as usize].outstanding().get(g);
        if want > 0 {
            w.deliver_to(post, g, want);
        }
    }
    assert!(w.build_at(post, Kind::TradingPost.build_ticks()));
    (w, Nav::new(), post)
}

fn mine(w: &World) -> Vec<&sim::Mule> {
    w.mules.iter().filter(|m| m.owner == ME && m.alive()).collect()
}

#[test]
fn a_trader_is_a_mule_and_unassigning_one_retires_it() {
    let (mut w, _nav, post) = with_a_post();
    assert!(mine(&w).is_empty(), "a post with nobody in it sends nothing out");

    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();
    assert_eq!(w.citizens[0].job, Some(Job::Trader));
    assert_eq!(mine(&w).len(), 1, "one trader is one mule");

    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(1)], building: post }).unwrap();
    assert_eq!(mine(&w).len(), 2);

    // And the slots are the trade rate.
    assert_eq!(
        w.apply(ME, &Command::Assign { citizens: vec![CitizenId(2)], building: post }),
        Err(RuleError::Full)
    );

    w.apply(ME, &Command::Unassign { citizens: vec![CitizenId(1)] }).unwrap();
    assert_eq!(mine(&w).len(), 1, "unassigning a trader retires its mule");
    w.apply(ME, &Command::Unassign { citizens: vec![CitizenId(0)] }).unwrap();
    assert!(mine(&w).is_empty());

    // Retired, not removed: an id means one thing for a whole run.
    assert_eq!(w.mules.len(), 2);
}

#[test]
fn a_post_the_flood_takes_stops_sending_carts_out() {
    let (mut w, mut nav, post) = with_a_post();
    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();
    assert_eq!(mine(&w).len(), 1);

    w.damage_building(post, Kind::TradingPost.integrity());
    w.move_mules(&mut nav);
    assert!(mine(&w).is_empty(), "a post in ruins still had a mule on the road");
}

#[test]
fn a_mule_carries_wood_out_and_brings_gold_back() {
    // The whole loop, and the only thing in the game that makes gold.
    let (mut w, mut nav, post) = with_a_post();
    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();

    let theirs = w.trade_partner(ME).expect("two cities, so there is a partner");
    let wood_before = w.treasury(ME).wood;
    let their_wood_before = w.buildings[theirs.0 as usize].store.wood;
    assert_eq!(w.treasury(ME).gold, 0, "nothing else makes gold");

    let mut sold = false;
    let mut paid = false;
    for _ in 0..TICKS_PER_DAY * 3 {
        w.move_mules(&mut nav);
        let m = &w.mules[0];
        if m.leg == Leg::Home && !sold {
            sold = true;
            assert!(
                w.buildings[theirs.0 as usize].store.wood > their_wood_before,
                "the other city was never handed the wood"
            );
            assert_eq!(m.carrying.gold, MULE_PAY, "and it was paid for it");
        }
        if sold && w.treasury(ME).gold > 0 {
            paid = true;
            break;
        }
    }
    assert!(sold, "the mule never got there");
    assert!(paid, "the mule never got home with the gold");
    assert_eq!(w.treasury(ME).gold, MULE_PAY);
    assert!(w.treasury(ME).wood < wood_before, "the wood came out of the city's own store");
}

#[test]
fn a_post_with_nothing_to_sell_waits() {
    let (mut w, mut nav, post) = with_a_post();
    // Empty the city out.
    for i in 0..w.buildings.len() {
        w.buildings[i].store = sim::building::Goods::NONE;
    }
    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();
    for _ in 0..200 {
        w.move_mules(&mut nav);
    }
    assert!(!w.mules[0].carrying_any(), "it loaded a cart out of an empty store");
    assert_eq!(w.mules[0].dest, None, "and it set off anyway");
}

#[test]
fn a_mule_with_nowhere_to_take_it_says_so() {
    // The panel reads `Leg::Stuck`. Without it a player watching a cart stand
    // in the yard has no way to find out why.
    let (mut w, mut nav, post) = with_a_post();
    // The other city is gone.
    let theirs: Vec<BuildingId> = w
        .buildings
        .iter()
        .filter(|b| b.owner == PlayerId(1) && b.kind == Kind::Hearth)
        .map(|b| b.id)
        .collect();
    for id in theirs {
        w.damage_building(id, Kind::Hearth.integrity());
    }
    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();
    for _ in 0..50 {
        w.move_mules(&mut nav);
    }
    assert_eq!(w.mules[0].leg, Leg::Stuck);
    assert!(w.mules[0].carrying_any(), "and it is still holding the load");
}

#[test]
fn a_mule_out_of_its_depth_loses_the_load() {
    // Design §6 of a hauler, and a mule is a hauler with four legs.
    let (mut w, mut nav, post) = with_a_post();
    w.apply(ME, &Command::Assign { citizens: vec![CitizenId(0)], building: post }).unwrap();
    for _ in 0..300 {
        w.move_mules(&mut nav);
        if w.mules[0].carrying_any() && w.mules[0].leg == Leg::Out {
            break;
        }
    }
    assert!(w.mules[0].carrying_any(), "the mule never picked anything up");

    let (x, y) = w.mules[0].pos.cell();
    w.water.raise_to(x, y, SWIM_DEPTH);
    w.flood_bodies();
    assert!(!w.mules[0].carrying_any(), "it swam with the cargo");
    assert_eq!(w.mules[0].leg, Leg::Home, "and it turned for home rather than walking on");
}

#[test]
fn two_peers_send_the_same_carts_to_the_same_places() {
    // Mules are world state like everything else: they are in the checksum,
    // so a mule that took a different turning on one machine is a desync the
    // lockstep would catch.
    let mut a = World::new(31, 2);
    let mut b = World::new(31, 2);
    let (mut na, mut nb) = (Nav::new(), Nav::new());

    let post_at = {
        let (hx, hy) = a.map.hearth_sites[0];
        let mut found = None;
        'ring: for r in 3..30i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if a.can_place(ME, Kind::TradingPost, Facing::EastWest, hx + dx, hy + dy)
                        .is_ok()
                    {
                        found = Some((hx + dx, hy + dy));
                        break 'ring;
                    }
                }
            }
        }
        found.expect("nowhere for a post")
    };

    let script: Vec<(u32, Command)> = vec![
        (
            1,
            Command::Place {
                kind: Kind::TradingPost,
                facing: Facing::EastWest,
                x: post_at.0 as u8,
                y: post_at.1 as u8,
            },
        ),
        // Late enough that the post is standing. Assigning to a *site* makes
        // builders, not traders, and a builder sends no cart out — which is
        // the rule working and was the first draft of this test not knowing
        // it.
        (
            3_000,
            Command::Assign { citizens: vec![CitizenId(0), CitizenId(1)], building: BuildingId(2) },
        ),
        (6_000, Command::Unassign { citizens: vec![CitizenId(1)] }),
    ];

    // Fed, on both worlds identically. Ten thousand ticks is eight days and
    // this city has no farm; without it everybody starves by day three and the
    // test measures two empty maps agreeing.
    let feed = |w: &mut World| {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
    };

    for t in 0..10_000u32 {
        let now: Vec<(PlayerId, Command)> =
            script.iter().filter(|(at, _)| *at == t).map(|(_, c)| (ME, c.clone())).collect();
        feed(&mut a);
        feed(&mut b);
        a.tick(&mut na, &now);
        b.tick(&mut nb, &now);
        if t % 500 == 0 {
            assert_eq!(a.checksum(), b.checksum(), "the two worlds parted at tick {t}");
        }
    }
    assert_eq!(a.checksum(), b.checksum());
    assert!(a.mules.iter().any(|m| m.alive()), "no mule survived the run to be checked");
    assert!(
        a.mules.iter().any(|m| !m.alive()),
        "the unassignment at tick 6000 retired nothing"
    );
}
