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
use crate::map::Map;
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
    /// How many ticks into the impact day each pulse of the surge starts.
    ///
    /// **These used to be corners.** The flood came out of the low corner of
    /// the map and age three came out of two of them; there is a river now and
    /// it has one upstream mouth, so what varies between ages is how many
    /// pulses come down it and when — not where they come from. Age three has
    /// two, about half a day apart (design §4).
    pub sources: Vec<u32>,
    /// Surge height, in the same units as terrain.
    pub height: u16,
}

impl Disaster {
    /// The age's row of design §4's table.
    ///
    /// | age | disaster | intensity |
    /// | 1 | flood down the river | height 12 |
    /// | 2 | flood | height 18 |
    /// | 3 | flood, two pulses | 18 each, about half a day apart |
    /// | 4 | as three | height 24 |
    /// | 5+ | height +6 per age | |
    ///
    /// **Design §4's second and third columns said "corner".** Every flood
    /// comes down the same channel now, so what escalates is height and how
    /// many pulses there are. That keeps the first floods learnable, which is
    /// design §5's first sentence and the reason the map guarantees a river
    /// that runs from the high side to the low one.
    pub fn draw(age: u32, map: &Map, rng: &mut Rng) -> Disaster {
        let height = match age {
            0 | 1 => 12,
            2 | 3 => 18,
            n => 24 + 6 * (n.saturating_sub(4)) as u16,
        };

        let _ = map;
        let sources = match age {
            0 | 1 | 2 => vec![0],
            // A second pulse down the same river, about half a day behind the
            // first — a little early or late, so the day cannot be memorised
            // to the tick the way a fixed offset can.
            _ => vec![0, (TICKS_PER_DAY as i32 / 2 + rng.range(-60, 60)).max(1) as u32],
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

/// Why a run stopped.
///
/// Not decoration: it is what decides whether the last age counts. A city that
/// drowned half way through age three survived two ages; a city that was still
/// standing when age three ran out survived three. Both end in age three with
/// the same tick on the clock, so the reason has to be remembered rather than
/// inferred.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Ending {
    /// Every city is gone. Design §4: "It is allowed to be sudden."
    LastCityFell,
    /// The map outlasted its ages — the MVP stops after age `MAX_AGE`.
    AgesRanOut,
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
    /// `None` while the run is still going.
    pub ending: Option<Ending>,
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
    ///
    /// Never past the last day of an age. An age rolls over when its impact
    /// day ends, so under way this can only reach `DAYS_PER_AGE` — except on
    /// the very last tick of the very last age, where the run finishes and
    /// there is no next age to roll into. The panel drew `day 7 of 6` on the
    /// final frame of the M10.6 run, which is where this was found.
    pub fn day_of_age(&self) -> u32 {
        ((self.tick - self.age_start_tick) / TICKS_PER_DAY + 1).min(DAYS_PER_AGE)
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
    /// How many pulses of the surge are pouring right now.
    ///
    /// A count rather than a list of places: they all come out of the same
    /// mouth, and two pulses overlapping is a bigger flood rather than a
    /// second one somewhere else.
    pub fn surging(&self) -> usize {
        if self.omen() != Omen::Impact {
            return 0;
        }
        let into_day = (self.tick - self.age_start_tick) % TICKS_PER_DAY;
        self.disaster
            .sources
            .iter()
            .filter(|&&offset| into_day >= offset && into_day < offset + SURGE_TICKS)
            .count()
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
            // Ages *completed*. Mid-run and after a collapse that is one fewer
            // than the age on the clock, because the current age is not over.
            // When the ages themselves ran out, the last one was finished and
            // counts — which is the whole reason `Ending` is recorded.
            ages_survived: match self.ending {
                Some(Ending::AgesRanOut) => self.age,
                _ => self.age.saturating_sub(1),
            },
            ending: self.ending,
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
            self.ending = Some(Ending::LastCityFell);
            return;
        }

        let elapsed = self.tick - self.age_start_tick;
        if elapsed >= DAYS_PER_AGE * TICKS_PER_DAY {
            if self.age >= MAX_AGE {
                // The MVP's run ends after age 3 whether or not anybody is
                // left standing.
                self.finished = Some(self.tick);
                self.ending = Some(Ending::AgesRanOut);
                return;
            }
            self.age += 1;
            self.age_start_tick = self.tick;
            // The high-water mark shows the *last* flood, so it is forgotten
            // when a new age begins. Floods escalate, so this is only ever a
            // difference during a flood — but during a flood is exactly when
            // somebody is watching it.
            self.water.forget_the_mark();
            // Drawn at age start, from the one Rng, and not shown.
            self.disaster = Disaster::draw(self.age, &self.map, &mut self.rng);
        }
    }
}
