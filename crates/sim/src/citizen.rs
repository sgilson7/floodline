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
/// A child, or somebody old enough to work.
///
/// A child does not haul, farm or build: it occupies a place in a nursery and
/// comes of age on a tick decided when it was born. It is a citizen in every
/// other way — it eats, it can be ordered uphill, and the flood does not care
/// how old anybody is.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Age {
    /// The tick this one becomes an adult.
    Child { of_age: u32 },
    Adult,
}

impl Age {
    pub fn is_child(self) -> bool {
        matches!(self, Age::Child { .. })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Job {
    Hauler,
    Farmer,
    Forester,
    Quarrier,
    Builder,
    /// Sends a mule out and takes what it brings back. The trader itself never
    /// leaves the post — the mule is a separate thing on the road, which is
    /// what design §6's "haulers you can watch" becomes when the load is going
    /// to another city.
    Trader,
    /// Stands at a cookery and turns raw food into meals.
    Cook,
}

impl Job {
    /// The job done at a building that makes something, or `None` if it makes
    /// nothing. Design §3.2 names these separately rather than calling them
    /// all "worker", and the panel says which one somebody is.
    pub fn at(kind: crate::building::Kind) -> Option<Job> {
        use crate::building::Kind;
        match kind {
            Kind::Farm => Some(Job::Farmer),
            Kind::Forester => Some(Job::Forester),
            Kind::Quarry => Some(Job::Quarrier),
            Kind::TradingPost => Some(Job::Trader),
            // The one job that had no building behind it until M12.B. A hut
            // is a roster and not a bench — `Job::Builder` is not `stationed`,
            // so nobody is ever added to its `workers` and nobody stands in
            // it. What assigning to it does is set the *job*, which is what
            // makes "these four are my builders" a thing a player can say.
            Kind::BuildersHut => Some(Job::Builder),
            Kind::Cookery => Some(Job::Cook),
            _ => None,
        }
    }

    /// Whether this job stands at one building and turns time into goods.
    pub fn produces(self) -> bool {
        matches!(self, Job::Farmer | Job::Forester | Job::Quarrier | Job::Cook)
    }

    /// Whether this job is done *at* a building, standing in one place and
    /// holding one of its slots.
    ///
    /// Not the same question as `produces`. A trader makes nothing — the gold
    /// is earned on the road by a mule — but it stands at its post and its
    /// slot is the trade rate, so every rule about being stationed applies to
    /// it. Answering the wrong one of these two questions is how a trader
    /// arriving at its post fell through to "there is no job here", had its
    /// workplace cleared, and left a mule on the road belonging to nobody.
    pub fn stationed(self) -> bool {
        self.produces() || self == Job::Trader
    }
}

/// The errand a citizen is part-way through.
///
/// Kept as one small enum rather than as flags, because the alternative is a
/// citizen that is somehow both eating and hauling and the tick order decides
/// which. An errand is given up wholesale when something more urgent comes up —
/// hunger interrupts hauling — but what is in the arms stays there and the
/// delivery resumes after the meal. See `Citizen::pause`.
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
    /// Ticks spent with no food.
    pub starved_for: u32,
    /// Ticks spent out of your depth. Resets the moment you find footing.
    pub drowning_for: u32,
    /// Told to go somewhere and stay there, and not yet told otherwise.
    ///
    /// Design §3.2 calls "get uphill" the one order that matters during a
    /// flood, and without this it does not work: a citizen that walked to
    /// where it was sent arrived with no errand, `find_work` gave it one, and
    /// it turned round and went back to the farm it had been told to leave —
    /// within a tick, and a day before the water came. Held citizens are not
    /// given work until the player says so, which is what `Unassign` (the
    /// panel's "back to hauling") and `Assign` are for. Hunger and exhaustion
    /// still overrule it, because a body overruling an order is the rule
    /// everywhere else in this file and a held citizen that starved standing
    /// on a hill would be a worse bug than the one this fixes.
    pub held: bool,
    /// Carried by the water rather than walking: too deep to stand up in.
    pub swept: bool,
    /// A child until the tick it names, then an adult. Everybody the run
    /// starts with is an adult.
    pub age: Age,
    /// The nursery a child is kept at, and nothing for an adult.
    pub nursery: Option<BuildingId>,
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
            drowning_for: 0,
            held: false,
            swept: false,
            age: Age::Adult,
            nursery: None,
        }
    }

    /// A child, born at `now` into `nursery`.
    pub fn born(
        id: CitizenId,
        owner: PlayerId,
        name: u16,
        pos: V2,
        now: u32,
        nursery: BuildingId,
    ) -> Citizen {
        let mut c = Citizen::new(id, owner, name, pos);
        c.age = Age::Child { of_age: now + COMING_OF_AGE };
        c.nursery = Some(nursery);
        c
    }

    pub fn is_child(&self) -> bool {
        self.age.is_child()
    }

    /// Grow up, if it is time. Returns true on the tick it happens.
    pub fn come_of_age(&mut self, now: u32) -> bool {
        match self.age {
            Age::Child { of_age } if now >= of_age => {
                self.age = Age::Adult;
                self.nursery = None;
                true
            }
            _ => false,
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
        self.swept = false;
    }

    /// Whether this citizen is doing something it should not be interrupted
    /// from without cause.
    pub fn busy(&self) -> bool {
        self.errand.is_some() || self.state == State::Walking
    }

    /// Give up whatever this was doing. Anything being carried is put down
    /// where the citizen stands, which is to say lost — there is nowhere else
    /// for it to go.
    ///
    /// For orders and for losses: `MoveTo`, `Unassign`, a dropped player, a
    /// workplace that has become rubble. **Not for hunger or exhaustion** —
    /// see `pause`, and the seven hundred and ten stone that went with it.
    pub fn abandon(&mut self) {
        self.errand = None;
        self.carrying = Goods::NONE;
        self.halt();
    }

    /// Stop what you are doing, but keep hold of it.
    ///
    /// What a body's interruption does. `assign_errands` used `abandon` here,
    /// so every time hunger or exhaustion overruled a delivery **the load was
    /// destroyed**, and the module's own note called that "the real cost of
    /// having left it too late".
    ///
    /// Measured, it is not a cost, it is a leak that empties a city:
    /// `what_a_walling_city_spends_its_days_on` watched a city with a wall
    /// ordered lose **seven hundred and ten stone of the seven hundred and
    /// twenty it started with, in one day**, on the day its granary first had
    /// food in it — because that is the day every starving hauler in the city
    /// had somewhere to eat, and each one dropped its load to go. Nothing
    /// anywhere said so; the panel read `stone 0` and the wall read
    /// `waiting for 210 stone`.
    ///
    /// The cost of leaving it too late is still there and is now the honest
    /// one: the walk, and a load that arrives later than it was wanted.
    /// `find_haul` looks at what is in a citizen's arms before anything else,
    /// so the delivery resumes after the meal.
    pub fn pause(&mut self) {
        self.errand = None;
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
    /// How fast this citizen moves, given what it is standing on.
    ///
    /// One function and not two, because "a road doubles it", "wading halves
    /// it" and "being tired halves it" are the same kind of rule and a second
    /// place that multiplied a speed is a second place to forget one. A road
    /// over a ford is a bridge, so the two footings cannot both apply.
    pub fn speed(&self, on_road: bool, wading: bool) -> Fx {
        let mut v = WALK_SPEED;
        if on_road {
            v *= 2;
        } else if wading {
            v /= 2;
        }
        if self.tired() {
            v /= 2;
        }
        Fx(v)
    }

    /// One tick of getting hungrier and more tired. `tick` is the world's, so
    /// that a need slower than one point a tick can be expressed at all.
    ///
    /// Needs fall while eating and sleeping too — the filling happens in the
    /// building's own rule and has to beat the decay to be worth walking to,
    /// which is what stops a citizen parking at a granary forever.
    pub fn tick_needs(&mut self, tick: u32) {
        if !self.alive() {
            return;
        }

        self.food = self.food.saturating_sub(FOOD_DECAY);
        // Rest is the slower need, and "slower than one point a tick" can only
        // be said by skipping ticks.
        if tick % REST_DECAY_INTERVAL == 0 {
            self.rest = self.rest.saturating_sub(REST_DECAY);
        }

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
