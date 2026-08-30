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

use sim::balance::TICKS_PER_DAY;
use sim::citizen::PlayerId;
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

            assert_eq!(a.checksum(), b.checksum(), "seed {seed}: differed before tick 1");

            for t in 1..=TICKS {
                a.tick();
                b.tick();

                // Everything a tick can touch. The map is generated once and
                // not written again until phase 2 brings water, so comparing
                // it here would be sixteen thousand cells of nothing.
                if a.tick != b.tick || a.rng != b.rng || a.citizens != b.citizens {
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
        first.tick();
    }
    let want = first.checksum();

    let mut again = World::new(first.seed, 3);
    for _ in 0..TICKS_PER_DAY * 4 {
        again.tick();
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
        a.tick();
        b.tick();
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
    for (x, y) in a.citizens.iter().zip(&b.citizens) {
        if x != y {
            return format!("citizen {:?}: {x:?} vs {y:?}", x.id);
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
