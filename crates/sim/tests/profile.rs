//! Plan item 2.4: profile `tick()` at 500 citizens with the flood running.
//!
//! Ignored by default — it is a measurement, not an assertion, and it is only
//! meaningful in release. Run it with:
//!
//! ```text
//! cargo test -p sim --release --test profile -- --ignored --nocapture
//! ```
//!
//! The target is under 20 ms a tick on native, so there is headroom for wasm,
//! which is slower. At `TICKS_PER_SECOND` a tick has `1000 / rate` ms to play
//! with — 50 ms since M11.1 doubled the clock, where it was 100 — and a 20 ms
//! budget still leaves rendering and networking the greater part of it.

use sim::balance::*;
use sim::building::{Facing, Good, Kind};
use sim::citizen::{Citizen, CitizenId, PlayerId};
use sim::fx::V2;
use sim::nav::Nav;
use sim::world::World;
use std::time::Instant;

/// A six-player world padded out to `n` citizens, with a farm and a granary
/// each, wound forward `days` days.
fn a_busy_world(n: usize, days: u32) -> (World, Nav) {
    let mut w = World::new(31, 6);

    for p in 0..6u8 {
        for kind in [Kind::Farm, Kind::Granary, Kind::Cottage] {
            let (hx, hy) = w.map.hearth_sites[p as usize];
            'place: for r in 3..30i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        if w.can_place(PlayerId(p), kind, Facing::EastWest, hx + dx, hy + dy).is_ok() {
                            let id = w.place(PlayerId(p), kind, Facing::EastWest, hx + dx, hy + dy).unwrap();
                            for g in Good::ALL {
                                let want = kind.cost().get(g);
                                if want > 0 {
                                    w.deliver_to(id, g, want);
                                }
                            }
                            w.build_at(id, kind.build_ticks());
                            break 'place;
                        }
                    }
                }
            }
        }
    }

    // Pad the population out to `n`, spread around the hearths.
    let mut rng = sim::Rng::new(7);
    while w.citizens.len() < n {
        let p = (w.citizens.len() % 6) as u8;
        let (hx, hy) = w.map.hearth_sites[p as usize];
        let id = CitizenId(w.citizens.len() as u16);
        let (dx, dy) = (rng.range(-10, 10), rng.range(-10, 10));
        let name = rng.below(256) as u16;
        w.citizens.push(Citizen::new(id, PlayerId(p), name, V2::cell_centre(hx + dx, hy + dy)));
    }

    let mut nav = Nav::new();
    while w.day_of_age() < days {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(&mut nav, &[]);
    }
    (w, nav)
}

fn time_ticks(w: &mut World, nav: &mut Nav, ticks: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..ticks {
        for c in &mut w.citizens {
            c.food = NEED_FULL;
            c.rest = NEED_FULL;
        }
        w.tick(nav, &[]);
    }
    start.elapsed().as_secs_f64() * 1000.0 / ticks as f64
}

#[test]
#[ignore]
fn tick_at_five_hundred_citizens_with_the_flood_running() {
    // Two days in, so the surge is still four days away. Winding to the impact
    // day and calling it dry was the first version of this and measured two
    // floods: the very next tick starts the water.
    let (mut dry, mut nav) = a_busy_world(500, 2);
    let dry_ms = time_ticks(&mut dry, &mut nav, 200);
    assert_eq!(dry.water.volume(), 0, "the 'dry' run was not dry");

    let (mut wet, mut wnav) = a_busy_world(500, World::IMPACT_DAY);
    // Past the first tick of the impact day, so the water is running.
    let _ = time_ticks(&mut wet, &mut wnav, 5);
    assert!(wet.water.volume() > 0, "the flood should be running");
    let wet_ms = time_ticks(&mut wet, &mut wnav, 200);

    println!();
    println!("  citizens          {}", wet.citizens.len());
    println!("  buildings         {}", wet.buildings.len());
    println!("  wet cells         {}", wet.water.wet_cells());
    println!("  tick, no water    {dry_ms:6.2} ms");
    println!("  tick, flooding    {wet_ms:6.2} ms   (budget 20.00 ms)");
    println!("  the automaton     {:6.2} ms", wet_ms - dry_ms);
    println!();

    assert!(
        wet_ms < 20.0,
        "a tick with the flood running took {wet_ms:.2} ms, over the 20 ms budget"
    );
}
