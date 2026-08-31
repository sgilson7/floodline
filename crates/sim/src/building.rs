//! Buildings, their placement rules, and how they get built.
//!
//! Roads and bridges are buildings here, not a flag on a cell. They are 1 x 1
//! and they cost builder-ticks like everything else, which means one
//! construction path, one damage path, and — the reason it matters — the
//! flood in phase 2 breaks a road cell by the same rule it breaks a cottage,
//! rather than by a second rule written later that has to be kept in step.

use crate::balance::*;
use crate::citizen::{CitizenId, PlayerId};
use crate::map::{Ground, Map};
use serde::{Deserialize, Serialize};

/// Index into `World::buildings`. Like `CitizenId`, never reused and never
/// removed: rubble stays in the vector so an id means one thing for a whole
/// run.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct BuildingId(pub u16);

/// The three things a city moves around. No currency: design §6 is explicit
/// that trade is barter along a road you can watch.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Good {
    Food,
    Wood,
    Stone,
}

impl Good {
    pub const ALL: [Good; 3] = [Good::Food, Good::Wood, Good::Stone];
}

/// A quantity of each good. Used for costs, for what has been delivered to a
/// site, and for what a store holds, because those are the same shape and
/// keeping them the same type is what lets `covers` be written once.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Goods {
    pub food: u16,
    pub wood: u16,
    pub stone: u16,
}

impl Goods {
    pub const NONE: Goods = Goods { food: 0, wood: 0, stone: 0 };

    pub const fn wood(n: u16) -> Goods {
        Goods { food: 0, wood: n, stone: 0 }
    }

    pub const fn stone(n: u16) -> Goods {
        Goods { food: 0, wood: 0, stone: n }
    }

    pub const fn of(food: u16, wood: u16, stone: u16) -> Goods {
        Goods { food, wood, stone }
    }

    pub fn get(&self, g: Good) -> u16 {
        match g {
            Good::Food => self.food,
            Good::Wood => self.wood,
            Good::Stone => self.stone,
        }
    }

    pub fn set(&mut self, g: Good, n: u16) {
        match g {
            Good::Food => self.food = n,
            Good::Wood => self.wood = n,
            Good::Stone => self.stone = n,
        }
    }

    /// Saturating everywhere: a store that would overflow is a store that is
    /// full, and a panic in a hauler is worse than a lost log.
    pub fn add(&mut self, g: Good, n: u16) {
        self.set(g, self.get(g).saturating_add(n));
    }

    /// Take up to `n`, returning what was actually there.
    pub fn take(&mut self, g: Good, n: u16) -> u16 {
        let taken = self.get(g).min(n);
        self.set(g, self.get(g) - taken);
        taken
    }

    pub fn is_empty(&self) -> bool {
        *self == Goods::NONE
    }

    /// Whether this holds at least `cost` of everything.
    pub fn covers(&self, cost: &Goods) -> bool {
        self.food >= cost.food && self.wood >= cost.wood && self.stone >= cost.stone
    }

    /// What is still missing to reach `cost`.
    pub fn shortfall(&self, cost: &Goods) -> Goods {
        Goods {
            food: cost.food.saturating_sub(self.food),
            wood: cost.wood.saturating_sub(self.wood),
            stone: cost.stone.saturating_sub(self.stone),
        }
    }

    pub fn total(&self) -> u32 {
        self.food as u32 + self.wood as u32 + self.stone as u32
    }

    /// A percentage of each, rounded down — what rubble gives back.
    pub fn percent(&self, pct: u16) -> Goods {
        let f = |n: u16| ((n as u32 * pct as u32) / 100) as u16;
        Goods { food: f(self.food), wood: f(self.wood), stone: f(self.stone) }
    }
}

/// What a building is made of, which is what decides how it fares in moving
/// water (design §3.4). Used in phase 2.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Material {
    Wood,
    Stone,
}

/// Which way a footprint runs.
///
/// The name is the axis the *long* side lies along, not the direction a wall
/// faces: `EastWest` is three cells across and one deep. Only a dike is
/// anything but square, so only a dike cares — but `Command::Place` carries a
/// facing for every kind, because a wire format with a field that is present
/// for one building and absent for the others is a wire format with two
/// shapes.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Facing {
    #[default]
    EastWest,
    NorthSouth,
}

impl Facing {
    pub const ALL: [Facing; 2] = [Facing::EastWest, Facing::NorthSouth];

    pub fn turned(self) -> Facing {
        match self {
            Facing::EastWest => Facing::NorthSouth,
            Facing::NorthSouth => Facing::EastWest,
        }
    }

    /// The axis a run from `from` to `to` lies along. Ties go to east-west,
    /// which is only reachable when the two ends are the same cell.
    pub fn of_run(from: (i32, i32), to: (i32, i32)) -> Facing {
        if (to.0 - from.0).abs() >= (to.1 - from.1).abs() {
            Facing::EastWest
        } else {
            Facing::NorthSouth
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Kind {
    Hearth,
    Cottage,
    Farm,
    /// Design §3.3's forester's hut: the only way to make wood.
    Forester,
    /// Design §3.3's quarry: the only way to make stone, and it has to be cut
    /// out of something, so it wants rock beside it.
    Quarry,
    Granary,
    Stockpile,
    Dike,
    Road,
    Bridge,
}

impl Kind {
    /// Everything the MVP builds. Design §3.3 also lists Fishery, Forester's
    /// hut, Quarry, Guildhall, Tavern and Watchtower; the plan defers all six.
    pub const ALL: [Kind; 10] = [
        Kind::Hearth,
        Kind::Cottage,
        Kind::Farm,
        Kind::Forester,
        Kind::Quarry,
        Kind::Granary,
        Kind::Stockpile,
        Kind::Dike,
        Kind::Road,
        Kind::Bridge,
    ];

    /// Footprint in cells, as (width, height). Only a dike answers this
    /// differently for the two facings; everything else is square.
    pub fn size(self, facing: Facing) -> (i32, i32) {
        match self {
            Kind::Hearth => (HEARTH_SIZE, HEARTH_SIZE),
            Kind::Farm => (3, 3),
            Kind::Forester | Kind::Quarry => (2, 2),
            Kind::Cottage | Kind::Granary | Kind::Stockpile => (2, 2),
            Kind::Dike => match facing {
                Facing::EastWest => (DIKE_LENGTH, 1),
                Facing::NorthSouth => (1, DIKE_LENGTH),
            },
            Kind::Road | Kind::Bridge => (1, 1),
        }
    }

    /// Whether facing means anything for this kind.
    ///
    /// Read off `size` rather than kept as a second table, so a kind that
    /// stops being square cannot forget to say so. Everything that stores a
    /// facing normalises through this: a cottage placed "north-south" has to
    /// be byte-identical to the same cottage placed "east-west", or two peers
    /// would checksum a distinction the game does not make.
    pub fn turns(self) -> bool {
        self.size(Facing::EastWest) != self.size(Facing::NorthSouth)
    }

    /// What one costs. A Dike costs this per level.
    pub fn cost(self) -> Goods {
        match self {
            // Free, and one per player: the run begins with it.
            Kind::Hearth => Goods::NONE,
            Kind::Cottage => Goods::wood(30),
            Kind::Farm => Goods::of(0, 40, 10),
            // Each one is bought with what the other makes, and that is the
            // whole point of the pair.
            //
            // Both cost wood to begin with, which meant the wood shortage
            // funded its own cure and the seven hundred stone a city starts
            // with had nowhere to go but dikes. A city starts with the stone
            // and needs the wood: so stone buys the hut that makes wood, and
            // wood buys the quarry that makes stone. Forty stone is a fifth of
            // what is in the Hearth on day one, so the hut is never the
            // building the shortage stops you building.
            Kind::Forester => Goods::stone(40),
            Kind::Quarry => Goods::wood(40),
            Kind::Granary => Goods::wood(50),
            // Free — it is a patch of ground somebody agreed to keep tidy.
            Kind::Stockpile => Goods::NONE,
            // Ten a level *per cell*, not forty. A wall that changes the
            // outcome of a run is about thirty-four cells long — measured, in
            // `tests/playtest.rs` — and at forty a cell that is 2 720 stone
            // against a purse of 120. See `balance::STARTING_STONE`.
            //
            // The price is per cell and the segment is the unit, so it scales
            // with `DIKE_LENGTH`. Making a segment three cells long without
            // this made a wall a third of its measured price overnight, and
            // the playtest noticed before anybody would have: the dike
            // strategy stopped needing to choose where to spend.
            Kind::Dike => Goods::stone(10 * DIKE_LENGTH as u16),
            // Design and plan both leave the cost of a road unstated. Time
            // only: a road is packed earth, and the thing it actually costs
            // is builders who could have been doing something else while the
            // water is coming. If phase 5 finds rebuilding a washed-out link
            // too cheap a decision, a material cost is the knob.
            Kind::Road => Goods::NONE,
            Kind::Bridge => Goods::wood(20),
        }
    }

    /// Builder-ticks to finish one, once its materials are on site.
    pub fn build_ticks(self) -> u32 {
        match self {
            Kind::Hearth => 0,
            Kind::Cottage => 200,
            Kind::Farm => 300,
            Kind::Forester => 180,
            Kind::Quarry => 220,
            Kind::Granary => 250,
            Kind::Stockpile => 60,
            // Per cell, for the same reason the stone is: a segment is three
            // cells of earth to move, not one.
            Kind::Dike => 150 * DIKE_LENGTH as u32,
            Kind::Road => 20,
            Kind::Bridge => 80,
        }
    }

    pub fn material(self) -> Material {
        match self {
            Kind::Cottage
            | Kind::Farm
            | Kind::Forester
            | Kind::Quarry
            | Kind::Granary
            | Kind::Stockpile
            | Kind::Bridge => Material::Wood,
            Kind::Hearth | Kind::Dike | Kind::Road => Material::Stone,
        }
    }

    /// How much damage one absorbs before it is rubble.
    pub fn integrity(self) -> u16 {
        match self {
            Kind::Hearth => 400,
            Kind::Cottage => 200,
            Kind::Farm => 200,
            Kind::Forester => 180,
            Kind::Quarry => 220,
            Kind::Granary => 250,
            Kind::Stockpile => 150,
            Kind::Dike => 400,
            // A road cell is a strip of packed stone and a bridge a few
            // planks: far less building than a cottage, and design §6 wants
            // the flood to take them. At a hundred a road was surviving an
            // age-two surge by a whisker, which made "rebuilding the link
            // after an age" a decision nobody ever had to make.
            Kind::Road => 40,
            Kind::Bridge => 60,
        }
    }

    /// How much water this keeps off somebody sheltering in or on it, in
    /// terrain-height units.
    ///
    /// Design §5: "Anyone who reaches high ground or a rooftop (buildings
    /// above `depth + 2` are climbable) survives." A roof is the second of the
    /// two things that save you, and without it a city on flat lowland is a
    /// death sentence rather than a gamble.
    pub fn shelter(self) -> u16 {
        match self {
            Kind::Hearth => 6,
            Kind::Granary => 5,
            Kind::Cottage => 4,
            Kind::Farm => 3,
            Kind::Forester | Kind::Quarry => 3,
            Kind::Stockpile => 2,
            Kind::Bridge => 1,
            // A dike shelters nobody by being a building; it does it by
            // raising the ground, which the automaton already knows about.
            Kind::Dike | Kind::Road => 0,
        }
    }

    /// How much flow this shrugs off before it starts taking damage.
    ///
    /// Per kind rather than per material, because a road is a stone thing that
    /// is nonetheless a hand's breadth thick: design §6 wants "the flood
    /// breaks road cells it flows over", and at a wall's resistance it never
    /// would.
    pub fn resist(self) -> u16 {
        match self {
            // A road and a bridge are the thinnest things on the map: a
            // hand's breadth of stone and a few planks. Design §6 wants "the
            // flood breaks road cells it flows over", and at a wall's
            // resistance a road came through a surge without a scratch.
            Kind::Road | Kind::Bridge => depth(1),
            _ => match self.material() {
                Material::Wood => RESIST_WOOD,
                Material::Stone => RESIST_STONE,
            },
        }
    }

    /// Beds. Only a Cottage has any (design §3.3).
    pub fn beds(self) -> usize {
        match self {
            Kind::Cottage => 4,
            _ => 0,
        }
    }

    /// Job slots, for the jobs the MVP has. Builders are not counted here —
    /// they are assigned to a construction site, which is a state rather than
    /// a kind, and `BUILDER_SLOTS` is their limit.
    pub fn job_slots(self) -> usize {
        match self {
            Kind::Farm => 3,
            // Two rather than three: wood and stone are wanted in bursts, and
            // a city that can put half its people on timber is a city that
            // forgets to eat.
            Kind::Forester | Kind::Quarry => 2,
            Kind::Granary | Kind::Stockpile | Kind::Hearth => 2,
            _ => 0,
        }
    }

    /// How much of each good this can hold.
    pub fn capacity(self) -> Goods {
        match self {
            // Wood and stone, and deliberately no food. Design §3.3 gives the
            // Hearth no larder and gives the Granary "citizens eat here"; a
            // Hearth that took food would sit nearer the farm than the granary
            // on most layouts, quietly become the city's pantry, and leave the
            // granary empty while everybody ate at the fire.
            //
            // Roomier than the starting stores, and that is a rule rather than
            // a comfortable margin — `the_hearth_can_hold_what_a_city_starts_with`
            // enforces it. At 500 against a starting 720 of stone, a hauler
            // that carried twenty to a site wanting ten had nowhere to put the
            // other ten: the Hearth was over its own capacity, so `has_room_for`
            // refused it, and the leftovers stayed in its arms for the rest of
            // the run. A hundred and forty stone went missing from the panel
            // that way, which is a fifth of the game's entire supply.
            Kind::Hearth => Goods::of(0, 900, 900),
            Kind::Granary => Goods::of(500, 0, 0),
            Kind::Stockpile => Goods::of(0, 500, 500),
            // Not stores — output buffers. See `is_store`.
            Kind::Farm => Goods::of(FARM_BUFFER, 0, 0),
            Kind::Forester => Goods::of(0, PRODUCER_BUFFER, 0),
            Kind::Quarry => Goods::of(0, 0, PRODUCER_BUFFER),
            _ => Goods::NONE,
        }
    }

    /// Whether this is somewhere goods are *kept*, as opposed to somewhere
    /// they happen to be.
    ///
    /// A Farm holds food, but it is a buffer waiting for a hauler, not a
    /// store: if farms counted, a hauler emptying one would find the nearest
    /// place to put food was the farm it just came from, and the food would
    /// never reach a granary.
    pub fn is_store(self) -> bool {
        matches!(self, Kind::Hearth | Kind::Granary | Kind::Stockpile)
    }

    /// What this building makes, if anything.
    pub fn produces(self) -> Option<Good> {
        match self {
            Kind::Farm => Some(Good::Food),
            Kind::Forester => Some(Good::Wood),
            Kind::Quarry => Some(Good::Stone),
            _ => None,
        }
    }

    /// Worker-ticks per unit made. See `balance` for where the numbers come
    /// from; a building that makes nothing never asks.
    pub fn ticks_per_unit(self) -> u32 {
        match self {
            Kind::Farm => FARM_TICKS_PER_UNIT,
            Kind::Forester => FOREST_TICKS_PER_UNIT,
            Kind::Quarry => QUARRY_TICKS_PER_UNIT,
            _ => u32::MAX,
        }
    }

    /// Whether goods of this kind may be *delivered* here for keeping.
    pub fn stores(self, g: Good) -> bool {
        self.is_store() && self.capacity().get(g) > 0
    }

    /// Whether this building can hold any more of `g`, given what it has.
    pub fn has_room_for(self, g: Good, held: &Goods) -> bool {
        held.get(g) < self.capacity().get(g)
    }

    /// How many citizens can work here at once. A construction site takes
    /// builders regardless of what it is going to become.
    pub fn slots_for(self, job: crate::citizen::Job) -> usize {
        use crate::citizen::Job;
        match job {
            job if job.produces() => {
                if Job::at(self) == Some(job) {
                    self.job_slots()
                } else {
                    0
                }
            }
            Job::Builder => BUILDER_SLOTS,
            Job::Hauler => usize::MAX,
            // Unreachable: every other job produces. Written out rather than
            // left to a wildcard so a new job has to come here and say what it
            // means.
            Job::Farmer | Job::Forester | Job::Quarrier => 0,
        }
    }

    /// Whether the ground under one cell of the footprint will take it.
    ///
    /// The two exceptions are the whole reason this is per-kind: a Bridge
    /// exists to cross shallows and must be *on* them, and everything else
    /// must be on ground that is not shallows and not rock.
    pub fn accepts(self, ground: Ground) -> bool {
        match self {
            // A bridge goes over water, and a ford is water. Bridging a ford
            // is the whole point of a ford: the crossing that was slow and
            // closed on the impact day becomes a road that is neither.
            Kind::Bridge => ground.watery(),
            _ => ground.buildable(),
        }
    }
}

/// Where a building is in its life.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BuildState {
    /// Materials are being hauled in and builder-ticks applied.
    Site,
    Standing,
    /// Broken. Keeps its footprint until somebody clears it.
    Rubble,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Building {
    pub id: BuildingId,
    pub owner: PlayerId,
    pub kind: Kind,
    /// Which way the footprint runs. Always `EastWest` for a kind that does
    /// not turn; `Building::site` normalises it.
    pub facing: Facing,
    /// Top-left cell of the footprint.
    pub x: u8,
    pub y: u8,
    pub state: BuildState,
    /// Materials hauled to the site so far.
    pub delivered: Goods,
    /// Builder-ticks applied so far.
    pub progress: u32,
    pub integrity: u16,
    /// Dikes only, 1..=DIKE_MAX_LEVEL. One for everything else.
    pub level: u8,
    /// How much the water has leaned on this, in scaled pressure-ticks.
    /// Dikes only; everything else takes damage the ordinary way.
    pub stress: u32,
    pub store: Goods,
    /// Work accumulated toward the next unit of output. Producers only.
    pub work: u32,
    /// Assigned citizens, in the order they were assigned. A `Vec` and not a
    /// set, because iteration order is a decision here like everywhere else.
    pub workers: Vec<CitizenId>,
}

impl Building {
    pub fn site(
        id: BuildingId,
        owner: PlayerId,
        kind: Kind,
        facing: Facing,
        x: i32,
        y: i32,
    ) -> Building {
        Building {
            id,
            owner,
            kind,
            facing: if kind.turns() { facing } else { Facing::EastWest },
            x: x as u8,
            y: y as u8,
            state: BuildState::Site,
            delivered: Goods::NONE,
            progress: 0,
            integrity: kind.integrity(),
            level: 1,
            stress: 0,
            store: Goods::NONE,
            work: 0,
            workers: Vec::new(),
        }
    }

    /// A building that is simply there, skipping construction. Only the
    /// founding Hearths use this.
    pub fn standing(
        id: BuildingId,
        owner: PlayerId,
        kind: Kind,
        facing: Facing,
        x: i32,
        y: i32,
    ) -> Building {
        let mut b = Building::site(id, owner, kind, facing, x, y);
        b.delivered = kind.cost();
        b.progress = kind.build_ticks();
        b.state = BuildState::Standing;
        b
    }

    pub fn standing_now(&self) -> bool {
        self.state == BuildState::Standing
    }

    /// The cells this occupies, in a fixed order.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (w, h) = self.size();
        let (x0, y0) = (self.x as i32, self.y as i32);
        (0..h).flat_map(move |dy| (0..w).map(move |dx| (x0 + dx, y0 + dy)))
    }

    /// This building's footprint in cells, as (width, height).
    pub fn size(&self) -> (i32, i32) {
        self.kind.size(self.facing)
    }

    /// The middle of the footprint, where citizens walk to.
    pub fn centre(&self) -> (i32, i32) {
        let (w, h) = self.size();
        (self.x as i32 + w / 2, self.y as i32 + h / 2)
    }

    /// What this site still needs before builders can do anything.
    pub fn outstanding(&self) -> Goods {
        self.delivered.shortfall(&self.total_cost())
    }

    /// A Dike pays its cost once per level.
    pub fn total_cost(&self) -> Goods {
        let c = self.kind.cost();
        let n = self.level as u32;
        Goods {
            food: (c.food as u32 * n).min(u16::MAX as u32) as u16,
            wood: (c.wood as u32 * n).min(u16::MAX as u32) as u16,
            stone: (c.stone as u32 * n).min(u16::MAX as u32) as u16,
        }
    }

    /// Take delivery of materials. Returns what was accepted, so a hauler
    /// carrying more than is wanted keeps the rest.
    ///
    /// Crate-private, like `build` and `damage`: finishing a road or a bridge
    /// changes what the map is passable through, and only `World` knows to
    /// tell the flow fields. Callers outside `sim` go through `World`.
    pub(crate) fn deliver(&mut self, g: Good, amount: u16) -> u16 {
        if self.state != BuildState::Site {
            return 0;
        }
        let wanted = self.outstanding().get(g);
        let taken = wanted.min(amount);
        self.delivered.add(g, taken);
        taken
    }

    /// Whether builders can make progress: everything is on site.
    pub fn ready_to_build(&self) -> bool {
        self.state == BuildState::Site && self.outstanding().is_empty()
    }

    /// Apply builder-ticks. Returns true on the tick it finishes.
    pub(crate) fn build(&mut self, effort: u32) -> bool {
        if !self.ready_to_build() {
            return false;
        }
        self.progress += effort;
        if self.progress >= self.kind.build_ticks() {
            self.progress = self.kind.build_ticks();
            self.state = BuildState::Standing;
            return true;
        }
        false
    }

    /// Take damage. Returns true on the tick it becomes rubble.
    pub(crate) fn damage(&mut self, amount: u16) -> bool {
        if self.state == BuildState::Rubble {
            return false;
        }
        self.integrity = self.integrity.saturating_sub(amount);
        if self.integrity == 0 {
            self.state = BuildState::Rubble;
            self.workers.clear();
            return true;
        }
        false
    }

    /// What is recovered when this is demolished or ruined: a fraction of the
    /// materials that went in, plus everything it was storing. Stored goods
    /// come back whole — they were not built into anything.
    pub fn salvage(&self) -> Goods {
        let mut out = self.delivered.percent(RUBBLE_REFUND_PERCENT);
        for g in Good::ALL {
            out.add(g, self.store.get(g));
        }
        out
    }

    /// The two rows or columns the water can push on: the cells beside the
    /// run, on each of its long sides.
    ///
    /// A wall has a wet side and a dry side, and which is which is a fact
    /// about the water rather than about the wall — so this answers with both
    /// and `flood` takes whichever is wetter. Cells off the map are included
    /// and read as dry, which is right: a wall on the edge is pushed on from
    /// one side only.
    pub fn sides(&self) -> [Vec<(i32, i32)>; 2] {
        let (w, h) = self.size();
        let (x0, y0) = (self.x as i32, self.y as i32);
        match self.facing {
            Facing::EastWest => [
                (0..w).map(|d| (x0 + d, y0 - 1)).collect(),
                (0..w).map(|d| (x0 + d, y0 + h)).collect(),
            ],
            Facing::NorthSouth => [
                (0..h).map(|d| (x0 - 1, y0 + d)).collect(),
                (0..h).map(|d| (x0 + w, y0 + d)).collect(),
            ],
        }
    }

    /// What this dike can take before it gives way.
    pub fn stress_limit(&self) -> u32 {
        dike_stress_limit(self.level)
    }

    /// How close this is to giving way, from 0 to 100.
    ///
    /// On screen rather than in the rules: a dike that breaks without warning
    /// is a dike that broke arbitrarily as far as the player is concerned, and
    /// the whole point of a pressure model is that you can watch it coming.
    pub fn strain(&self) -> u32 {
        if self.kind != Kind::Dike {
            return 0;
        }
        (self.stress.saturating_mul(100) / self.stress_limit().max(1)).min(100)
    }

    /// How much this raises the ground under it. Only a standing Dike does.
    pub fn ground_bonus(&self) -> u16 {
        if self.kind == Kind::Dike && self.standing_now() {
            DIKE_HEIGHT_PER_LEVEL * self.level as u16
        } else {
            0
        }
    }

    /// Whether a citizen walks faster over this. Roads and bridges carry
    /// traffic; a farm does not.
    pub fn carries_traffic(&self) -> bool {
        self.standing_now() && matches!(self.kind, Kind::Road | Kind::Bridge)
    }

    /// Whether this blocks movement.
    ///
    /// A construction site does not — builders have to stand in it. Roads and
    /// bridges do not, obviously. Neither does a **Dike**: it is a raised bank
    /// of earth and stone, and a player who rings their city with one to keep
    /// the water out must not thereby wall their own citizens in. That would
    /// turn the single most important defensive structure in the game into a
    /// trap, and a player would learn to stop building them, which is the
    /// opposite of what design §5 wants taught.
    pub fn blocks_movement(&self) -> bool {
        self.standing_now() && !matches!(self.kind, Kind::Road | Kind::Bridge | Kind::Dike)
    }

    /// Whether the footprint would fit on the map at all, ignoring what is
    /// already there. Split out so placement can report the two failures
    /// separately.
    pub fn fits_on_map(kind: Kind, facing: Facing, x: i32, y: i32) -> bool {
        let (w, h) = kind.size(facing);
        x >= 0 && y >= 0 && x + w <= crate::map::MAP_W && y + h <= crate::map::MAP_H
    }

    /// The cells a footprint of `kind` at `(x, y)` would cover.
    pub fn footprint(
        kind: Kind,
        facing: Facing,
        x: i32,
        y: i32,
    ) -> impl Iterator<Item = (i32, i32)> {
        let (w, h) = kind.size(facing);
        (0..h).flat_map(move |dy| (0..w).map(move |dx| (x + dx, y + dy)))
    }

    /// Whether every cell of the footprint is ground this kind will stand on.
    pub fn ground_suits(kind: Kind, facing: Facing, map: &Map, x: i32, y: i32) -> bool {
        Building::footprint(kind, facing, x, y)
            .all(|(cx, cy)| kind.accepts(map.ground_at(cx, cy)))
    }

    /// A quarry has to be cut out of something.
    ///
    /// The only rule in the game that asks what is *next to* a footprint
    /// rather than under it, and it is here because rock was decoration
    /// otherwise: every map has some (`ROCK_PERCENT`), none of it is
    /// buildable, none of it is passable, and until now nothing wanted it.
    /// Now the one building that ends the stone shortage has to be put
    /// somewhere particular, which is a decision about the map rather than
    /// another slot on the build menu.
    pub fn neighbours_suit(kind: Kind, facing: Facing, map: &Map, x: i32, y: i32) -> bool {
        if kind != Kind::Quarry {
            return true;
        }
        let (w, h) = kind.size(facing);
        for cy in y - 1..=y + h {
            for cx in x - 1..=x + w {
                if map.ground_at(cx, cy) == Ground::Rock {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goods_arithmetic_saturates_rather_than_wrapping() {
        let mut g = Goods::of(1, 2, 3);
        assert_eq!(g.get(Good::Food), 1);
        assert_eq!(g.get(Good::Wood), 2);
        assert_eq!(g.get(Good::Stone), 3);

        g.add(Good::Food, u16::MAX);
        assert_eq!(g.food, u16::MAX, "a full store is full, not empty again");

        assert_eq!(g.take(Good::Wood, 100), 2, "takes only what is there");
        assert_eq!(g.wood, 0);
        assert_eq!(g.take(Good::Wood, 100), 0, "and nothing from empty");
    }

    #[test]
    fn covers_and_shortfall_are_opposites() {
        let cost = Goods::of(0, 40, 10);
        assert!(!Goods::of(0, 39, 10).covers(&cost));
        assert!(Goods::of(0, 40, 10).covers(&cost));
        assert!(Goods::of(5, 100, 20).covers(&cost));

        assert_eq!(Goods::NONE.shortfall(&cost), cost);
        assert_eq!(Goods::of(0, 30, 0).shortfall(&cost), Goods::of(0, 10, 10));
        assert_eq!(Goods::of(0, 99, 99).shortfall(&cost), Goods::NONE);
        assert!(Goods::of(0, 99, 99).shortfall(&cost).is_empty());
    }

    #[test]
    fn percent_rounds_down() {
        assert_eq!(Goods::of(0, 30, 10).percent(50), Goods::of(0, 15, 5));
        assert_eq!(Goods::of(0, 1, 1).percent(50), Goods::NONE, "half of one is none");
        assert_eq!(Goods::of(9, 9, 9).percent(100), Goods::of(9, 9, 9));
        // No overflow at the top of the range.
        assert_eq!(Goods::of(u16::MAX, 0, 0).percent(100).food, u16::MAX);
    }

    #[test]
    fn every_kind_has_a_coherent_definition() {
        for k in Kind::ALL {
            for f in Facing::ALL {
                let (w, h) = k.size(f);
                assert!(w > 0 && h > 0, "{k:?} has no footprint facing {f:?}");
            }
            // Turning a footprint transposes it and nothing else, so a kind
            // cannot be one size east-west and a different area north-south.
            let (w, h) = k.size(Facing::EastWest);
            assert_eq!(k.size(Facing::NorthSouth), (h, w), "{k:?} changes area when turned");
            assert!(k.integrity() > 0, "{k:?} is rubble the moment it is built");
            // A Hearth is the only thing that appears already finished.
            if k != Kind::Hearth {
                assert!(k.build_ticks() > 0, "{k:?} builds itself instantly");
            }
            // Anything that takes delivery of something must have somewhere
            // to put it — but not the other way round. A Farm has room for
            // food and is not a store: that room is an output buffer waiting
            // for a hauler, and if it counted as a store then a hauler
            // emptying a farm would find the nearest place to put food was the
            // farm it had just come from.
            for g in Good::ALL {
                if k.stores(g) {
                    assert!(k.capacity().get(g) > 0, "{k:?} takes {g:?} with nowhere to put it");
                    assert!(k.is_store(), "{k:?} takes delivery but is not a store");
                }
            }
            if let Some(made) = k.produces() {
                assert!(k.capacity().get(made) > 0, "{k:?} makes {made:?} with nowhere to put it");
                assert!(!k.is_store(), "{k:?} is both a producer and a store");
                assert!(!k.stores(made), "a producer must not be a destination for its own output");
            }
        }
    }

    #[test]
    fn only_a_bridge_goes_in_the_water() {
        for k in Kind::ALL {
            assert!(!k.accepts(Ground::Rock), "{k:?} accepted rock");
            if k == Kind::Bridge {
                assert!(k.accepts(Ground::Shallows));
                assert!(!k.accepts(Ground::Grass), "a bridge over dry land is a folly");
                assert!(!k.accepts(Ground::Sand));
            } else {
                assert!(k.accepts(Ground::Grass), "{k:?} would not go on grass");
                assert!(k.accepts(Ground::Sand), "{k:?} would not go on sand");
                assert!(!k.accepts(Ground::Shallows), "{k:?} was placed in the water");
            }
        }
    }

    #[test]
    fn a_footprint_covers_exactly_its_cells() {
        let b = Building::site(BuildingId(0), PlayerId(0), Kind::Farm, Facing::EastWest, 10, 20);
        let cells: Vec<(i32, i32)> = b.cells().collect();
        assert_eq!(cells.len(), 9);
        assert_eq!(cells[0], (10, 20), "starts at the top-left");
        assert_eq!(cells[8], (12, 22), "and ends at the bottom-right");
        // In a fixed order — row by row — because a flood iterating these
        // must do it the same way on every peer.
        assert_eq!(cells[1], (11, 20));
        assert_eq!(cells[3], (10, 21));
        assert_eq!(b.centre(), (11, 21));

        // The free function and the method agree.
        let same: Vec<(i32, i32)> = Building::footprint(Kind::Farm, Facing::EastWest, 10, 20).collect();
        assert_eq!(same, cells);
    }

    #[test]
    fn a_one_by_one_centres_on_itself() {
        let b = Building::site(BuildingId(0), PlayerId(0), Kind::Road, Facing::EastWest, 7, 8);
        assert_eq!(b.centre(), (7, 8));
        assert_eq!(b.cells().collect::<Vec<_>>(), vec![(7, 8)]);
    }

    #[test]
    fn a_dike_is_three_cells_and_turning_it_turns_its_footprint() {
        // Was a 1 x 1 block placed cell by cell. A wall is a run, and which
        // way the run lies is the one thing about it a player chooses.
        let across = Building::site(BuildingId(0), PlayerId(0), Kind::Dike, Facing::EastWest, 7, 8);
        assert_eq!(across.cells().collect::<Vec<_>>(), vec![(7, 8), (8, 8), (9, 8)]);
        assert_eq!(across.centre(), (8, 8), "the middle of the run, not an end of it");

        let down =
            Building::site(BuildingId(1), PlayerId(0), Kind::Dike, Facing::NorthSouth, 7, 8);
        assert_eq!(down.cells().collect::<Vec<_>>(), vec![(7, 8), (7, 9), (7, 10)]);
        assert_eq!(down.centre(), (7, 9));
    }

    #[test]
    fn a_square_building_forgets_which_way_it_was_placed() {
        // Two peers must not checksum a distinction the game does not make.
        let ew = Building::site(BuildingId(0), PlayerId(0), Kind::Cottage, Facing::EastWest, 5, 5);
        let ns =
            Building::site(BuildingId(0), PlayerId(0), Kind::Cottage, Facing::NorthSouth, 5, 5);
        assert_eq!(ew, ns);
        assert_eq!(ns.facing, Facing::EastWest);
        assert!(!Kind::Cottage.turns());
        assert!(Kind::Dike.turns(), "the dike is the one kind that does turn");
    }

    #[test]
    fn a_site_needs_its_materials_before_anyone_can_build() {
        let mut b = Building::site(BuildingId(0), PlayerId(0), Kind::Farm, Facing::EastWest, 5, 5);
        assert_eq!(b.state, BuildState::Site);
        assert_eq!(b.outstanding(), Goods::of(0, 40, 10));
        assert!(!b.ready_to_build());

        // Builders standing at a site with nothing on it achieve nothing —
        // not "a bit", nothing. That is what makes haulers matter.
        assert!(!b.build(1000));
        assert_eq!(b.progress, 0);

        assert_eq!(b.deliver(Good::Wood, 100), 40, "takes only what it needs");
        assert_eq!(b.outstanding(), Goods::stone(10));
        assert!(!b.ready_to_build());

        assert_eq!(b.deliver(Good::Stone, 4), 4);
        assert_eq!(b.deliver(Good::Stone, 4), 4);
        assert_eq!(b.deliver(Good::Stone, 4), 2, "and only the remainder at the end");
        assert!(b.ready_to_build());
        assert_eq!(b.deliver(Good::Stone, 10), 0, "a finished delivery takes no more");
    }

    #[test]
    fn building_finishes_exactly_once() {
        let mut b = Building::site(BuildingId(0), PlayerId(0), Kind::Cottage, Facing::EastWest, 5, 5);
        b.deliver(Good::Wood, 30);
        let ticks = Kind::Cottage.build_ticks();

        for _ in 0..ticks - 1 {
            assert!(!b.build(1), "finished early");
        }
        assert!(b.build(1), "the tick it finishes says so");
        assert_eq!(b.state, BuildState::Standing);
        assert_eq!(b.progress, ticks, "progress does not run past the end");
        assert!(!b.build(1), "and it does not finish twice");
        assert_eq!(b.progress, ticks);
    }

    #[test]
    fn a_free_building_needs_no_delivery() {
        let mut b = Building::site(BuildingId(0), PlayerId(0), Kind::Stockpile, Facing::EastWest, 1, 1);
        assert!(b.outstanding().is_empty());
        assert!(b.ready_to_build(), "nothing to haul, so builders can start");
        assert!(b.build(Kind::Stockpile.build_ticks()));
    }

    #[test]
    fn a_dike_pays_per_level() {
        // Written against `Kind::Dike.cost()` rather than against the number
        // it happens to be: what is under test is that a dike pays per level,
        // and the price itself is a balance constant that has already moved
        // once (see `balance::STARTING_STONE`).
        let per = Kind::Dike.cost().stone;
        let mut b = Building::site(BuildingId(0), PlayerId(0), Kind::Dike, Facing::EastWest, 3, 3);
        assert_eq!(b.total_cost(), Goods::stone(per));
        b.level = 3;
        assert_eq!(b.total_cost(), Goods::stone(per * 3));
        assert_eq!(b.ground_bonus(), 0, "a site holds nothing back");

        b.deliver(Good::Stone, per * 3);
        b.build(Kind::Dike.build_ticks());
        assert_eq!(b.state, BuildState::Standing);
        assert_eq!(b.ground_bonus(), DIKE_HEIGHT_PER_LEVEL * 3);
    }

    #[test]
    fn damage_ends_at_rubble_and_stays_there() {
        let mut b = Building::standing(BuildingId(0), PlayerId(0), Kind::Cottage, Facing::EastWest, 5, 5);
        b.workers.push(CitizenId(3));
        let max = Kind::Cottage.integrity();

        assert!(!b.damage(max - 1));
        assert_eq!(b.integrity, 1);
        assert!(b.standing_now());

        assert!(b.damage(1), "the tick it breaks says so");
        assert_eq!(b.state, BuildState::Rubble);
        assert!(b.workers.is_empty(), "the people working there are no longer");
        assert!(!b.damage(500), "and it does not break twice");
        assert_eq!(b.integrity, 0, "damage saturates rather than wrapping");
    }

    #[test]
    fn salvage_returns_half_the_materials_and_all_the_stores() {
        let mut b = Building::standing(BuildingId(0), PlayerId(0), Kind::Granary, Facing::EastWest, 5, 5);
        b.store = Goods::of(120, 0, 0);
        // 50% of the 50 wood that went in, plus every scrap of the food.
        assert_eq!(b.salvage(), Goods::of(120, 25, 0));
    }

    #[test]
    fn roads_carry_traffic_and_only_walls_stop_it() {
        for k in Kind::ALL {
            let mut b = Building::standing(BuildingId(0), PlayerId(0), k, Facing::EastWest, 5, 5);

            let is_way = matches!(k, Kind::Road | Kind::Bridge);
            assert_eq!(b.carries_traffic(), is_way, "{k:?}");

            // A Dike is walkable but not fast: a raised bank of earth. If it
            // blocked, a player who ringed their city with one to keep the
            // water out would have walled their own citizens in, and would
            // learn to stop building the one structure design §5 exists to
            // teach.
            let walkable = matches!(k, Kind::Road | Kind::Bridge | Kind::Dike);
            assert_eq!(b.blocks_movement(), !walkable, "{k:?}");
            assert!(!Kind::Dike.accepts(Ground::Shallows), "and it is built on dry land");

            // A construction site is walked through, not around: builders have
            // to stand in it.
            b.state = BuildState::Site;
            assert!(!b.blocks_movement(), "{k:?} site blocked movement");
            assert!(!b.carries_traffic(), "{k:?} site carried traffic");

            // Rubble is neither. A ruined road is ground again and a ruined
            // bridge is open water.
            b.state = BuildState::Rubble;
            assert!(!b.blocks_movement(), "{k:?} rubble blocked movement");
            assert!(!b.carries_traffic(), "{k:?} rubble carried traffic");
        }
    }

    #[test]
    fn a_footprint_must_fit_on_the_map() {
        use crate::map::{MAP_H, MAP_W};
        assert!(Building::fits_on_map(Kind::Farm, Facing::EastWest, 0, 0));
        assert!(Building::fits_on_map(Kind::Farm, Facing::EastWest, MAP_W - 3, MAP_H - 3));
        assert!(!Building::fits_on_map(Kind::Farm, Facing::EastWest, MAP_W - 2, 0), "hangs off the right");
        assert!(!Building::fits_on_map(Kind::Farm, Facing::EastWest, 0, MAP_H - 2), "hangs off the bottom");
        assert!(!Building::fits_on_map(Kind::Road, Facing::EastWest, -1, 0));
        assert!(Building::fits_on_map(Kind::Road, Facing::EastWest, MAP_W - 1, MAP_H - 1));

        // A dike hangs off along whichever axis it runs, and not the other.
        assert!(Building::fits_on_map(Kind::Dike, Facing::EastWest, MAP_W - 3, MAP_H - 1));
        assert!(!Building::fits_on_map(Kind::Dike, Facing::EastWest, MAP_W - 2, MAP_H - 1));
        assert!(Building::fits_on_map(Kind::Dike, Facing::NorthSouth, MAP_W - 1, MAP_H - 3));
        assert!(!Building::fits_on_map(Kind::Dike, Facing::NorthSouth, MAP_W - 1, MAP_H - 2));
    }
}
