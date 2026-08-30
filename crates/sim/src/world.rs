//! The world, and the one number that says whether two of them agree.
//!
//! Everything a run consists of hangs off `World`, including the `Rng`, and
//! the only way any of it changes is `tick` and `apply`. Design §7's rule —
//! "`gui` never constructs a `World` change except by handing a `Command` to
//! the lockstep" — is why the mutating methods are the short list they are.

use crate::balance::*;
use crate::citizen::{Citizen, CitizenId, PlayerId};
use crate::fx::V2;
use crate::map::Map;
use crate::names::NAMES;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct World {
    /// The seed this run was generated from. Kept so the score screen can show
    /// it and somebody can replay the map that drowned them.
    pub seed: u64,
    /// The one generator. Nothing else in `sim` may hold one.
    pub rng: Rng,
    pub tick: u32,
    pub map: Map,
    /// Indexed by `CitizenId`. The dead stay in it, so an id never moves.
    pub citizens: Vec<Citizen>,
    pub players: Vec<PlayerId>,
}

impl World {
    /// A fresh run: a map for `players` players, and a founding party at each
    /// Hearth site.
    pub fn new(seed: u64, players: u32) -> World {
        let players = players.clamp(2, 6);
        let mut rng = Rng::new(seed);
        let map = Map::generate(&mut rng, players);

        let mut citizens = Vec::new();
        for p in 0..players {
            let (hx, hy) = map.hearth_sites[p as usize];
            for _ in 0..FOUNDING_CITIZENS {
                // Spread the party around its hearth so eight people do not
                // start inside one another.
                let dx = rng.range(-2, 2);
                let dy = rng.range(-2, 2);
                let name = rng.below(NAMES.len() as u32) as u16;
                let id = CitizenId(citizens.len() as u16);
                citizens.push(Citizen::new(
                    id,
                    PlayerId(p as u8),
                    name,
                    V2::cell_centre(hx + dx, hy + dy),
                ));
            }
        }

        World {
            seed,
            rng,
            tick: 0,
            map,
            citizens,
            players: (0..players).map(|p| PlayerId(p as u8)).collect(),
        }
    }

    /// The number two peers compare every tick.
    ///
    /// FNV-1a over the `postcard` encoding of the whole world, rather than a
    /// hand-written hash of the fields that "matter". A hand-written one drifts
    /// the moment somebody adds a field and forgets to hash it, and the field
    /// they forget is the one that diverges. This way, anything that is part of
    /// the world is part of the checksum by construction — and the encoding is
    /// the same encoding a late joiner receives, so if the checksum agrees the
    /// snapshot will too.
    pub fn checksum(&self) -> u64 {
        let bytes = postcard::to_allocvec(self).expect("a World is always encodable");
        fnv1a(&bytes)
    }

    /// The living population of one city.
    pub fn population(&self, owner: PlayerId) -> u32 {
        self.citizens.iter().filter(|c| c.owner == owner && c.alive()).count() as u32
    }

    /// Which in-game day it is, counting from one.
    pub fn day(&self) -> u32 {
        self.tick / TICKS_PER_DAY + 1
    }

    /// One step of the simulation.
    ///
    /// Iteration is over a `Vec` in index order, which is the only order there
    /// is. That is not a stylistic preference: a `HashMap` here would give two
    /// peers two different orders and the flood would push their citizens in
    /// two different directions.
    pub fn tick(&mut self) {
        for c in &mut self.citizens {
            c.tick_needs();
        }
        self.tick += 1;
    }
}

/// FNV-1a, 64-bit. Small, has no state to get wrong, and — the part that
/// matters — is defined entirely in terms of `wrapping_mul`, so it produces
/// the same number on every target.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citizen::State;

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        // The reference values for FNV-1a 64. Written down rather than
        // recorded from this implementation, so the test can actually fail.
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn the_same_seed_builds_the_same_world() {
        for seed in [1u64, 42, 0xFACE, u64::MAX] {
            let a = World::new(seed, 3);
            let b = World::new(seed, 3);
            assert_eq!(a.checksum(), b.checksum(), "seed {seed}");
            assert_eq!(a, b);
        }
    }

    #[test]
    fn different_seeds_build_different_worlds() {
        assert_ne!(World::new(1, 2).checksum(), World::new(2, 2).checksum());
    }

    #[test]
    fn the_checksum_notices_every_field() {
        // The point of hashing the encoding rather than a chosen list of
        // fields: touch anything and the number moves.
        let base = World::new(7, 2);

        let mut w = base.clone();
        w.tick += 1;
        assert_ne!(w.checksum(), base.checksum(), "tick");

        let mut w = base.clone();
        w.citizens[0].food -= 1;
        assert_ne!(w.checksum(), base.checksum(), "a citizen's food");

        let mut w = base.clone();
        w.citizens[3].pos.x += crate::fx::Fx(1);
        assert_ne!(w.checksum(), base.checksum(), "a citizen's position");

        let mut w = base.clone();
        w.citizens[5].name += 1;
        assert_ne!(w.checksum(), base.checksum(), "a citizen's name");

        let mut w = base.clone();
        w.map.height[999] = w.map.height[999].wrapping_add(1);
        assert_ne!(w.checksum(), base.checksum(), "one cell of terrain");

        let mut w = base.clone();
        w.rng.next_u64();
        assert_ne!(w.checksum(), base.checksum(), "the rng having been drawn from");
    }

    #[test]
    fn the_checksum_is_stable_across_encodings() {
        let w = World::new(11, 2);
        let a = w.checksum();
        let bytes = postcard::to_allocvec(&w).unwrap();
        let round_tripped: World = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(round_tripped, w, "a world survives its own snapshot");
        assert_eq!(round_tripped.checksum(), a, "and checksums the same afterwards");
    }

    #[test]
    fn a_snapshot_is_small_enough_to_send() {
        // Design §8 budgets 50–150 KB for a late joiner's Welcome at 500
        // citizens. This is the empty end of that: mostly the 16 k map cells.
        let w = World::new(1, 6);
        let bytes = postcard::to_allocvec(&w).unwrap().len();
        assert!(bytes < 150_000, "a fresh six-player world encodes to {bytes} bytes");
    }

    #[test]
    fn a_founding_party_lands_at_its_hearth() {
        let w = World::new(5, 4);
        assert_eq!(w.citizens.len(), 4 * FOUNDING_CITIZENS as usize);
        for (p, &(hx, hy)) in w.map.hearth_sites.iter().enumerate() {
            let owned: Vec<&Citizen> =
                w.citizens.iter().filter(|c| c.owner == PlayerId(p as u8)).collect();
            assert_eq!(owned.len(), FOUNDING_CITIZENS as usize);
            for c in owned {
                let (cx, cy) = c.pos.cell();
                assert!(
                    (cx - hx).abs() <= 2 && (cy - hy).abs() <= 2,
                    "citizen {:?} started at ({cx},{cy}), hearth is ({hx},{hy})",
                    c.id
                );
                assert_eq!(c.state, State::Idle);
                assert_eq!(c.food, NEED_FULL);
            }
        }
    }

    #[test]
    fn ids_are_indices() {
        let w = World::new(2, 5);
        for (i, c) in w.citizens.iter().enumerate() {
            assert_eq!(c.id, CitizenId(i as u16));
        }
    }

    #[test]
    fn needs_fall_and_hunger_eventually_kills() {
        let mut w = World::new(3, 2);
        let start = w.citizens[0].food;

        // Nobody eats, because nothing has been built to eat at yet.
        for _ in 0..100 {
            w.tick();
        }
        assert_eq!(w.tick, 100);
        assert_eq!(w.citizens[0].food, start - FOOD_DECAY * 100);
        assert!(w.citizens[0].alive());

        // Empty by tick 250, then three days of starving.
        let empty_at = (NEED_FULL / FOOD_DECAY) as u32;
        while w.tick < empty_at {
            w.tick();
        }
        assert_eq!(w.citizens[0].food, 0);
        assert_eq!(w.citizens[0].state, State::Starving);

        // Death lands exactly STARVE_TICKS ticks after the food ran out —
        // stated against `starved_for` rather than against a tick arithmetic
        // expression, because the first version of this test got that
        // arithmetic wrong by one and blamed the code.
        while w.citizens[0].starved_for < STARVE_TICKS - 1 {
            w.tick();
        }
        assert!(w.citizens[0].alive(), "alive with one tick of the three days left");
        w.tick();
        assert_eq!(w.citizens[0].state, State::Dead);
        assert_eq!(w.citizens[0].starved_for, STARVE_TICKS);
        assert_eq!(w.tick, empty_at + STARVE_TICKS - 1);
        assert_eq!(w.population(PlayerId(0)), 0);
    }

    #[test]
    fn eating_before_the_clock_runs_out_saves_a_citizen() {
        let mut w = World::new(3, 2);
        let empty_at = (NEED_FULL / FOOD_DECAY) as u32;
        while w.tick < empty_at + STARVE_TICKS / 2 {
            w.tick();
        }
        assert_eq!(w.citizens[0].state, State::Starving);
        assert!(w.citizens[0].starved_for > 0);

        w.citizens[0].eat(NEED_FULL);
        w.tick();
        assert_eq!(w.citizens[0].state, State::Idle);
        assert_eq!(w.citizens[0].starved_for, 0);

        // And the clock starts from the beginning next time, rather than
        // resuming where it left off.
        while w.citizens[0].food > 0 {
            w.tick();
        }
        assert_eq!(w.citizens[0].starved_for, 1);
    }

    #[test]
    fn eating_takes_only_what_there_is_room_for() {
        let mut c = World::new(1, 2).citizens.remove(0);
        assert_eq!(c.eat(500), 0, "a full citizen takes nothing");
        c.food = NEED_FULL - 10;
        assert_eq!(c.eat(500), 10);
        assert_eq!(c.food, NEED_FULL);
        c.food = 0;
        assert_eq!(c.eat(7), 7);
        assert_eq!(c.food, 7);
    }

    #[test]
    fn the_dead_stop_changing() {
        let mut c = World::new(1, 2).citizens.remove(0);
        c.state = State::Dead;
        let before = c.clone();
        for _ in 0..1000 {
            c.tick_needs();
        }
        assert_eq!(c, before, "a corpse does not get hungrier");
    }

    #[test]
    fn a_day_is_a_day() {
        let mut w = World::new(1, 2);
        assert_eq!(w.day(), 1);
        for _ in 0..TICKS_PER_DAY {
            w.tick();
        }
        assert_eq!(w.day(), 2);
    }
}
