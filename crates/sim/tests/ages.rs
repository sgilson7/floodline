//! The clock: ages, the disaster drawn for each, the day's notice, the score.
//!
//! Written against the constants rather than against numbers, so that tuning
//! `TICKS_PER_DAY` or `DAYS_PER_AGE` — which design §11 explicitly leaves open
//! — changes the game without breaking the tests that describe it.

use sim::age::{DisasterKind, Omen};
use sim::balance::*;
use sim::citizen::PlayerId;
use sim::nav::Nav;
use sim::world::World;
use sim::Disaster;

/// Run to a given tick, ignoring everything else.
fn run_to(w: &mut World, tick: u32) {
    let mut nav = Nav::new();
    while w.tick < tick && w.finished().is_none() {
        w.tick(&mut nav, &[]);
    }
}

/// A world nobody starves in, so the clock can be tested without the famine
/// ending the run first.
fn immortal(seed: u64) -> World {
    let mut w = World::new(seed, 2);
    for c in &mut w.citizens {
        c.food = NEED_FULL;
    }
    w
}

/// Keep everybody fed, so the run lasts as long as the ages do.
fn feed(w: &mut World) {
    for c in &mut w.citizens {
        if c.alive() {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
    }
}

fn run_fed(w: &mut World, ticks: u32) {
    let mut nav = Nav::new();
    for _ in 0..ticks {
        if w.finished().is_some() {
            break;
        }
        feed(w);
        w.tick(&mut nav, &[]);
    }
}

#[test]
fn a_run_starts_in_age_one_on_day_one() {
    let w = World::new(31, 2);
    assert_eq!(w.age(), 1);
    assert_eq!(w.day_of_age(), 1);
    assert_eq!(w.day(), 1);
    assert_eq!(w.omen(), Omen::Quiet);
    assert_eq!(w.finished(), None);
}

#[test]
fn age_ones_flood_comes_out_of_the_low_corner() {
    // Design §5: ages 1–3 always come out of the lowest corner, so the first
    // floods are learnable.
    for seed in 0..40u64 {
        let w = World::new(seed, 2);
        assert_eq!(w.disaster.kind, DisasterKind::Flood);
        assert_eq!(w.disaster.height, 12, "age one is height 12");
        assert_eq!(w.disaster.sources.len(), 1);
        assert_eq!(w.disaster.sources[0].0, w.map.low_corner, "seed {seed}");
        assert_eq!(w.disaster.sources[0].1, 0, "and it starts at once");
    }
}

#[test]
fn the_table_escalates_the_way_design_four_says() {
    let w = World::new(7, 2);
    let heights: Vec<u16> = (1..=6)
        .map(|age| Disaster::draw(age, &w.map, &mut w.rng.clone()).height)
        .collect();
    assert_eq!(heights, vec![12, 18, 18, 24, 30, 36]);

    // Ages one and two, one corner, the low one.
    for age in 1..=2 {
        let d = Disaster::draw(age, &w.map, &mut w.rng.clone());
        assert_eq!(d.sources.len(), 1);
        assert_eq!(d.sources[0].0, w.map.low_corner);
    }

    // Age three: two corners, offset by half a day.
    let d = Disaster::draw(3, &w.map, &mut w.rng.clone());
    assert_eq!(d.sources.len(), 2, "age three floods from two corners");
    assert_eq!(d.sources[0].0, w.map.low_corner);
    assert_ne!(d.sources[1].0, w.map.low_corner, "and the second is somewhere else");
    assert_eq!(d.sources[1].1, TICKS_PER_DAY / 2, "half a day behind");

    // Age four and beyond can come from anywhere, the high corner included.
    let corners: std::collections::BTreeSet<_> = (0..40u64)
        .map(|s| Disaster::draw(4, &w.map, &mut sim::Rng::new(s)).sources[0].0)
        .collect();
    assert!(corners.len() > 1, "age four always picked the same corner");
}

#[test]
fn the_disaster_is_a_function_of_the_seed() {
    for seed in [1u64, 99, 0xF00D] {
        let a = World::new(seed, 3);
        let b = World::new(seed, 3);
        assert_eq!(a.disaster, b.disaster, "seed {seed}");
    }
    // And two seeds do not have to agree.
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..40u64 {
        seen.insert(World::new(seed, 2).disaster.sources[0].0);
    }
    assert!(seen.len() > 1, "every seed floods from the same corner");
}

#[test]
fn the_village_gets_one_days_notice_and_no_more() {
    let mut w = immortal(31);

    // Walked day by day, asserting the whole rule at each one rather than
    // stepping to the interesting days — an off-by-one in the walk is then an
    // off-by-one in the assertion too, and shows up immediately.
    for day in 1..=DAYS_PER_AGE {
        assert_eq!(w.day_of_age(), day, "the walk lost count");
        let expected = if day == World::IMPACT_DAY {
            Omen::Impact
        } else if day + 1 == World::IMPACT_DAY {
            // "The elders are uneasy." One day, and no detail: there is no
            // watchtower in the MVP, so nobody knows which corner or how bad.
            Omen::Uneasy
        } else {
            Omen::Quiet
        };
        assert_eq!(w.omen(), expected, "day {day} of the age");

        if day < DAYS_PER_AGE {
            run_fed(&mut w, TICKS_PER_DAY);
        }
    }
}

#[test]
fn the_water_runs_on_the_impact_day_and_not_before() {
    let mut w = immortal(31);
    assert!(w.surging_from().is_empty(), "the flood started on day one");

    // To the first tick of the impact day.
    run_fed(&mut w, TICKS_PER_DAY * (World::IMPACT_DAY - 1));
    assert_eq!(w.day_of_age(), World::IMPACT_DAY);

    let sources = w.surging_from();
    assert_eq!(sources, vec![w.map.low_corner], "the surge did not start");

    // It runs for a while and then stops, whether because the surge is over or
    // because the age is.
    let mut ran = 0;
    let mut nav = Nav::new();
    while !w.surging_from().is_empty() && w.finished().is_none() {
        feed(&mut w);
        w.tick(&mut nav, &[]);
        ran += 1;
        assert!(ran < TICKS_PER_DAY * 2, "the surge never stopped");
    }
    assert!(ran > 0);
}

#[test]
fn ages_follow_one_another_and_the_run_ends_after_the_last() {
    let mut w = immortal(31);
    let age_ticks = TICKS_PER_DAY * DAYS_PER_AGE;

    for age in 1..MAX_AGE {
        assert_eq!(w.age(), age);
        let before = w.disaster.clone();
        run_fed(&mut w, age_ticks);
        assert_eq!(w.age(), age + 1, "the age did not turn over");
        assert_eq!(w.day_of_age(), 1, "the new age did not start on day one");
        assert_ne!(
            (w.disaster.height, w.disaster.sources.len()),
            (before.height, before.sources.len()),
            "age {} drew the same disaster as age {age}",
            age + 1
        );
    }

    assert_eq!(w.age(), MAX_AGE);
    assert!(w.finished().is_none(), "ended early");
    run_fed(&mut w, age_ticks);
    assert!(w.finished().is_some(), "the run did not end after the last age");
}

#[test]
fn a_finished_run_stops_dead() {
    let mut w = immortal(31);
    run_fed(&mut w, TICKS_PER_DAY * DAYS_PER_AGE * MAX_AGE + 10);
    let end = w.finished().expect("the run should be over");
    let after = w.checksum();

    // Nothing at all happens afterwards, on any peer.
    let mut nav = Nav::new();
    for _ in 0..500 {
        w.tick(&mut nav, &[]);
    }
    assert_eq!(w.finished(), Some(end));
    assert_eq!(w.checksum(), after, "the world moved after the run ended");
}

#[test]
fn the_run_ends_when_the_last_city_falls() {
    // Nobody is fed, so both cities starve. Design §4: it is allowed to be
    // sudden.
    let mut w = World::new(31, 2);
    run_to(&mut w, TICKS_PER_DAY * DAYS_PER_AGE * MAX_AGE);

    assert!(w.finished().is_some(), "everybody starved and the run went on");
    assert_eq!(w.population(PlayerId(0)), 0);
    assert_eq!(w.population(PlayerId(1)), 0);
    assert!(w.age() < MAX_AGE, "it should have ended before the ages ran out");
}

#[test]
fn the_score_says_what_happened() {
    let mut w = World::new(31, 2);
    let start = w.score();
    assert_eq!(start.seed, 31, "the seed is shown so the map can be replayed");
    assert_eq!(start.ages_survived, 0, "nothing survived yet");
    assert_eq!(start.cities.len(), 2);
    for c in &start.cities {
        assert_eq!(c.peak_population, FOUNDING_CITIZENS);
        assert!(c.survived);
    }
    assert!(start.anyone_left);

    // Starve them.
    run_to(&mut w, TICKS_PER_DAY * DAYS_PER_AGE * MAX_AGE);
    let end = w.score();
    assert_eq!(end.seed, 31);
    assert!(!end.anyone_left);
    for c in &end.cities {
        assert_eq!(c.final_population, 0);
        assert!(!c.survived);
        assert_eq!(
            c.peak_population, FOUNDING_CITIZENS,
            "the peak is remembered after everybody is gone"
        );
    }
}

#[test]
fn peak_population_is_a_high_water_mark() {
    let mut w = World::new(31, 2);
    assert_eq!(w.peak_population[0], FOUNDING_CITIZENS);

    let victim = w.citizens.iter().find(|c| c.owner == PlayerId(0)).unwrap().id;
    w.citizens[victim.0 as usize].die();
    let mut nav = Nav::new();
    w.tick(&mut nav, &[]);

    assert_eq!(w.population(PlayerId(0)), FOUNDING_CITIZENS - 1);
    assert_eq!(
        w.peak_population[0], FOUNDING_CITIZENS,
        "the peak went down when somebody died"
    );
}

#[test]
fn a_surviving_city_is_scored_alongside_a_fallen_one() {
    // Design §6: a city with no living citizens is out; each player's own city
    // is scored alongside.
    let mut w = World::new(31, 2);
    for i in 0..w.citizens.len() {
        if w.citizens[i].owner == PlayerId(1) {
            w.citizens[i].die();
        }
    }
    let mut nav = Nav::new();
    w.tick(&mut nav, &[]);

    let s = w.score();
    assert!(s.anyone_left, "the run is not over while one city stands");
    assert!(s.cities[0].survived);
    assert!(!s.cities[1].survived);
    assert_eq!(w.finished(), None);
}
