//! People.
//!
//! A citizen is a small plain struct and a state machine, and the whole reason
//! the flood is worth building: a number going down is not a loss, and a named
//! stick figure who was walking to the granary and is now face down in a field
//! is. Everything here is integer and ordered, like the rest of `sim`.

use crate::balance::*;
use crate::building::{BuildingId, Good, Goods};
use crate::nav::Dest;
use crate::fx::{Fx, V2};
use serde::{Deserialize, Serialize};

/// Index into `World::citizens`. Citizens are never removed from that vector —
/// the dead stay in it as `Dead` — so an id is stable for the whole run and
/// nothing ever has to remap one. It costs a few bytes per corpse and buys the
/// absence of a whole category of desync.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct CitizenId(pub u16);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

/// What a citizen has been told to spend its day on.
///
/// Design §3.2 lists more; the plan's item 6 cuts the MVP to these three.
/// Hauler is what an unassigned citizen does, which is why `Citizen::job` is
/// an `Option` and `None` means hauling rather than idling: a city where
/// nobody moves anything is a city that starves next to a full granary.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Job {
    Hauler,
    Farmer,
    Builder,
}

/// The errand a citizen is part-way through.
///
/// Kept as one small enum rather than as flags, because the alternative is a
/// citizen that is somehow both eating and hauling and the tick order decides
/// which. An errand is abandoned wholesale when something more urgent comes
/// up — hunger interrupts hauling, and the load gets dropped where it is
/// standing, which is a real cost of leaving it too late.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Errand {
    /// On the way to `from` to pick up `good`, bound for `to`.
    Collect { from: BuildingId, good: Good, to: BuildingId },
    /// Loaded, on the way to `to`.
    Carry { to: BuildingId },
    /// On the way to the workplace.
    ToWork(BuildingId),
    /// On the way to a granary to eat.
    ToEat(BuildingId),
    /// On the way to a bed.
    ToSleep(BuildingId),
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
    /// What this citizen has been assigned to. `None` is a hauler: design
    /// §3.2 makes that the default for anyone unskilled.
    pub job: Option<Job>,
    /// The building the job is done at. A Farmer has one; a Hauler and a
    /// Builder pick their work as they go, so theirs is whatever they are
    /// currently at.
    pub workplace: Option<BuildingId>,
    /// What is in this citizen's arms.
    pub carrying: Goods,
    pub errand: Option<Errand>,
    /// 0..=NEED_FULL.
    pub food: u16,
    pub rest: u16,
    pub state: State,
    /// Where this citizen is walking, if anywhere.
    pub dest: Option<Dest>,
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
            workplace: None,
            carrying: Goods::NONE,
            errand: None,
            food: NEED_FULL,
            rest: NEED_FULL,
            state: State::Idle,
            dest: None,
            starved_for: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.state != State::Dead
    }

    pub fn hungry(&self) -> bool {
        self.food < HUNGRY
    }

    /// Out of food and on the clock.
    ///
    /// A condition, deliberately **not** a `State`. The first version of this
    /// made `Starving` a state, which overwrote `Walking` — so the moment a
    /// citizen ran out of food it stopped moving, including on its way to the
    /// granary that would have saved it. `State` is what somebody is doing;
    /// hunger and exhaustion are how they are, and the two are independent.
    pub fn starving(&self) -> bool {
        self.alive() && self.food == 0
    }

    pub fn tired(&self) -> bool {
        self.rest < TIRED
    }

    /// Send this citizen somewhere. Does nothing to the dead.
    pub fn walk_to(&mut self, dest: Dest) {
        if !self.alive() {
            return;
        }
        self.dest = Some(dest);
        self.state = State::Walking;
    }

    /// Stop being. Clears everything that only makes sense for the living, so
    /// a corpse is not still walking somewhere in the state it leaves behind.
    ///
    /// What it was carrying is lost with it. Design §6 says as much about
    /// trade — "a hauler that drowns loses the cargo" — and there is no reason
    /// for a famine to be gentler than a flood.
    pub fn die(&mut self) {
        self.state = State::Dead;
        self.vel = V2::ZERO;
        self.dest = None;
        self.job = None;
        self.workplace = None;
        self.errand = None;
        self.carrying = Goods::NONE;
    }

    /// Whether this citizen is doing something it should not be interrupted
    /// from without cause.
    pub fn busy(&self) -> bool {
        self.errand.is_some() || self.state == State::Walking
    }

    /// Give up whatever this was doing. Anything being carried is put down
    /// where the citizen stands, which is to say lost — there is nowhere else
    /// for it to go.
    pub fn abandon(&mut self) {
        self.errand = None;
        self.carrying = Goods::NONE;
        self.halt();
    }

    /// Stop walking, wherever this is.
    pub fn halt(&mut self) {
        self.dest = None;
        self.vel = V2::ZERO;
        if self.state == State::Walking {
            self.state = State::Idle;
        }
    }

    /// How far this citizen moves in a tick, in 256ths of a cell.
    ///
    /// Twice as fast on a road (design §6) and half speed when exhausted
    /// (design §3.2). Multiplication before division, so a tired citizen on a
    /// road walks at exactly the base rate rather than at whatever an integer
    /// division of an integer division would leave.
    pub fn speed(&self, on_road: bool) -> Fx {
        let mut v = WALK_SPEED;
        if on_road {
            v *= 2;
        }
        if self.tired() {
            v /= 2;
        }
        Fx(v)
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
            self.starved_for += 1;
            if self.starved_for >= STARVE_TICKS {
                self.die();
            }
        } else {
            // Fed before the clock ran out. The next famine starts from the
            // beginning rather than resuming where this one left off.
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
