//! People.
//!
//! A citizen is a small plain struct and a state machine, and the whole reason
//! the flood is worth building: a number going down is not a loss, and a named
//! stick figure who was walking to the granary and is now face down in a field
//! is. Everything here is integer and ordered, like the rest of `sim`.

use crate::balance::*;
use crate::fx::V2;
use serde::{Deserialize, Serialize};

/// Index into `World::citizens`. Citizens are never removed from that vector —
/// the dead stay in it as `Dead` — so an id is stable for the whole run and
/// nothing ever has to remap one. It costs a few bytes per corpse and buys the
/// absence of a whole category of desync.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct CitizenId(pub u16);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

/// Placeholder until phase 1 item 4 builds the real thing. Declared here so
/// `Citizen` has its final shape and the checksum does not change under it.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct BuildingId(pub u16);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Job {
    Hauler,
    Farmer,
    Builder,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum State {
    /// Nothing to do; stands near home.
    Idle,
    /// On the way somewhere.
    Walking,
    /// At the job building, producing.
    Working,
    /// At a granary, filling `food`.
    Eating,
    /// In a bed, filling `rest`.
    Sleeping,
    /// `food` is empty and the clock is running.
    Starving,
    Dead,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Citizen {
    pub id: CitizenId,
    pub owner: PlayerId,
    /// Index into `names::NAMES`. Two bytes rather than a `String`, because a
    /// `String` per citizen would put heap allocations in `World`, grow every
    /// snapshot, and give the determinism rules a new way to be broken. See
    /// DECISIONS.md.
    pub name: u16,
    pub pos: V2,
    pub vel: V2,
    pub home: Option<BuildingId>,
    pub job: Option<(Job, BuildingId)>,
    /// 0..=NEED_FULL.
    pub food: u16,
    pub rest: u16,
    pub state: State,
    /// Ticks spent with no food. Only counts while `Starving`.
    pub starved_for: u32,
}

impl Citizen {
    pub fn new(id: CitizenId, owner: PlayerId, name: u16, pos: V2) -> Citizen {
        Citizen {
            id,
            owner,
            name,
            pos,
            vel: V2::ZERO,
            home: None,
            job: None,
            food: NEED_FULL,
            rest: NEED_FULL,
            state: State::Idle,
            starved_for: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.state != State::Dead
    }

    pub fn hungry(&self) -> bool {
        self.food < HUNGRY
    }

    pub fn tired(&self) -> bool {
        self.rest < TIRED
    }

    /// One tick of getting hungrier and more tired.
    ///
    /// Needs fall while eating and sleeping too — the filling happens in the
    /// building's own rule and has to beat the decay to be worth walking to,
    /// which is what stops a citizen parking at a granary forever.
    pub fn tick_needs(&mut self) {
        if !self.alive() {
            return;
        }

        self.food = self.food.saturating_sub(FOOD_DECAY);
        self.rest = self.rest.saturating_sub(REST_DECAY);

        if self.food == 0 {
            if self.state != State::Starving {
                self.state = State::Starving;
                self.starved_for = 0;
            }
            self.starved_for += 1;
            if self.starved_for >= STARVE_TICKS {
                self.state = State::Dead;
                self.vel = V2::ZERO;
            }
        } else if self.state == State::Starving {
            // Fed again before the clock ran out.
            self.state = State::Idle;
            self.starved_for = 0;
        }
    }

    /// Eat, up to what is offered. Returns how much was actually taken, so the
    /// granary knows what to deduct.
    pub fn eat(&mut self, offered: u16) -> u16 {
        let room = NEED_FULL - self.food;
        let taken = room.min(offered);
        self.food += taken;
        taken
    }

    pub fn sleep(&mut self, amount: u16) {
        self.rest = (self.rest + amount).min(NEED_FULL);
    }
}
