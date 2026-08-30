//! Ages, the disaster that ends each one, and what the run was worth.
//!
//! Design §4's shape, in order: the disaster is decided at age start from the
//! seed and *not shown*; the village gets a day's notice; it strikes on the
//! last day of the age; whatever is left is what the next age starts from.
//!
//! The escalation table is data rather than code (design §4), so a later
//! disaster is a row rather than a branch. Only the flood exists — the plan is
//! explicit that nothing else is MVP — but fire and plague are meant to reuse
//! §3's machinery with different rules, and this is the shape that lets them.

use crate::balance::*;
use crate::citizen::PlayerId;
use crate::map::{Corner, Map};
use crate::rng::Rng;
use crate::world::World;
use serde::{Deserialize, Serialize};

/// What comes at the end of an age. One variant, for now, and the plan says so.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DisasterKind {
    Flood,
}

/// The disaster drawn for one age.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Disaster {
    pub kind: DisasterKind,
    /// Where the water comes from, and how many ticks into the impact day it
    /// starts there. Age 3 has two, half a day apart (design §4).
    pub sources: Vec<(Corner, u32)>,
    /// Surge height, in the same units as terrain.
    pub height: u16,
}

impl Disaster {
    /// The age's row of design §4's table.
    ///
    /// | age | disaster | intensity |
    /// | 1 | flood from the low corner | height 12 |
    /// | 2 | flood | height 18 |
    /// | 3 | flood, two corners | 18 each, offset half a day |
    /// | 4 | flood, random corner (can be the high one) | height 24 |
    /// | 5+ | height +6 per age | |
    ///
    /// Ages 1–3 always come out of the lowest corner, so the first floods are
    /// learnable — that is design §5's first sentence and the reason the low
    /// corner is a thing the map generator guarantees.
    pub fn draw(age: u32, map: &Map, rng: &mut Rng) -> Disaster {
        let height = match age {
            0 | 1 => 12,
            2 | 3 => 18,
            n => 24 + 6 * (n.saturating_sub(4)) as u16,
        };

        let sources = match age {
            0 | 1 | 2 => vec![(map.low_corner, 0)],
            3 => {
                // A second corner, drawn from the other three, half a day
                // behind the first.
                let others: Vec<Corner> =
                    Corner::ALL.into_iter().filter(|&c| c != map.low_corner).collect();
                let second = others[rng.below(others.len() as u32) as usize];
                vec![(map.low_corner, 0), (second, TICKS_PER_DAY / 2)]
            }
            _ => vec![(Corner::ALL[rng.below(4) as usize], 0)],
        };

        Disaster { kind: DisasterKind::Flood, sources, height }
    }
}

/// What the village knows.
///
/// Design §4: the disaster is decided at age start and not shown. A watchtower
/// would reveal which corner and how bad, some days early — there is no
/// watchtower in the MVP, so all anyone gets is the Hearth's one day of
/// "the elders are uneasy", which is a feeling rather than a forecast.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Omen {
    /// Days yet.
    Quiet,
    /// Something comes tomorrow. No detail, because nobody is watching for it.
    Uneasy,
    /// It is happening.
    Impact,
    /// The age's disaster has passed; what is left is what is left.
    Aftermath,
}

/// One city's line on the score screen.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CityScore {
    pub player: PlayerId,
    pub peak_population: u32,
    pub final_population: u32,
    pub survived: bool,
}

/// What the run was worth (design §4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Score {
    /// Shown so the same map can be replayed, and argued about.
    pub seed: u64,
    pub ages_survived: u32,
    pub days: u32,
    pub cities: Vec<CityScore>,
    /// Whether anybody was still standing at the end.
    pub anyone_left: bool,
}

impl World {
    /// Which age it is, counting from one.
    pub fn age(&self) -> u32 {
        self.age
    }

    /// Which day of the current age it is, counting from one.
    pub fn day_of_age(&self) -> u32 {
        (self.tick - self.age_start_tick) / TICKS_PER_DAY + 1
    }

    /// The day of an age on which the disaster strikes: the last one.
    pub const IMPACT_DAY: u32 = DAYS_PER_AGE;

    /// What the village knows right now.
    pub fn omen(&self) -> Omen {
        if self.finished.is_some() {
            return Omen::Aftermath;
        }
        match self.day_of_age() {
            d if d == Self::IMPACT_DAY => Omen::Impact,
            d if d + 1 == Self::IMPACT_DAY => Omen::Uneasy,
            _ => Omen::Quiet,
        }
    }

    /// Whether the water should be running this tick, and from where.
    ///
    /// Phase 2 injects the surge; this is the timing. A source is live from
    /// its offset into the impact day until `SURGE_TICKS` after it.
    pub fn surging_from(&self) -> Vec<Corner> {
        if self.omen() != Omen::Impact {
            return Vec::new();
        }
        let into_day = (self.tick - self.age_start_tick) % TICKS_PER_DAY;
        self.disaster
            .sources
            .iter()
            .filter(|&&(_, offset)| {
                into_day >= offset && into_day < offset + SURGE_TICKS
            })
            .map(|&(c, _)| c)
            .collect()
    }

    /// Whether the run is over, and when it ended.
    pub fn finished(&self) -> Option<u32> {
        self.finished
    }

    /// The score screen.
    pub fn score(&self) -> Score {
        let cities = self
            .players
            .iter()
            .map(|&p| CityScore {
                player: p,
                peak_population: self.peak_population[p.0 as usize],
                final_population: self.population(p),
                survived: self.population(p) > 0,
            })
            .collect::<Vec<_>>();

        Score {
            seed: self.seed,
            // A city that dies during age 3 survived two ages, not three.
            ages_survived: self.age.saturating_sub(1),
            days: self.tick / TICKS_PER_DAY + 1,
            anyone_left: cities.iter().any(|c| c.survived),
            cities,
        }
    }

    /// The clock: peak populations, the end of an age, and the end of the run.
    ///
    /// Called at the end of a tick, so "day 6 of age 1" means the whole of
    /// that day has been simulated before age 2 begins.
    pub(crate) fn tick_clock(&mut self) {
        for (i, &p) in self.players.iter().enumerate() {
            let now = self.population(p);
            if now > self.peak_population[i] {
                self.peak_population[i] = now;
            }
        }

        // The run ends when the last city falls. It is allowed to be sudden
        // (design §4).
        if self.players.iter().all(|&p| self.population(p) == 0) {
            self.finished = Some(self.tick);
            return;
        }

        let elapsed = self.tick - self.age_start_tick;
        if elapsed >= DAYS_PER_AGE * TICKS_PER_DAY {
            if self.age >= MAX_AGE {
                // The MVP's run ends after age 3 whether or not anybody is
                // left standing.
                self.finished = Some(self.tick);
                return;
            }
            self.age += 1;
            self.age_start_tick = self.tick;
            // Drawn at age start, from the one Rng, and not shown.
            self.disaster = Disaster::draw(self.age, &self.map, &mut self.rng);
        }
    }
}
