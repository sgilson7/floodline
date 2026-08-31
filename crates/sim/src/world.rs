//! The world, and the one number that says whether two of them agree.
//!
//! Everything a run consists of hangs off `World`, including the `Rng`, and
//! the only way any of it changes is `tick` and `apply`. Design §7's rule —
//! "`gui` never constructs a `World` change except by handing a `Command` to
//! the lockstep" — is why the mutating methods are the short list they are.

use crate::age::{Disaster, DisasterKind, Ending};
use crate::balance::*;
use crate::building::{BuildState, Building, BuildingId, Facing, Good, Goods, Kind};
use crate::citizen::{Citizen, CitizenId, Errand, Job, PlayerId, State};
use crate::command::Command;
use crate::fx::V2;
use crate::fx::Fx;
use crate::map::{Ground, Map};
use crate::household::{Household, HouseholdId};
use crate::mule::{Leg, Mule, MuleId};
use crate::names::NAMES;
use crate::nav::{self, Dest, FlowField, Nav};
use crate::road::{self, Road, RoadId, Trade, TradeId};
use crate::water::Water;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// Somebody pointing at the map.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Ping {
    pub by: PlayerId,
    pub x: u8,
    pub y: u8,
    /// The tick it was made, so it can fade.
    pub at: u32,
}

/// Why a rule said no.
///
/// Design §7 has `World::apply` return one of these, and every peer must
/// reject the same command for the same reason — a rule that is enforced on
/// one machine and not another is a desync wearing a disguise.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RuleError {
    /// Commanding somebody else's city.
    NotYours,
    /// The footprint would hang off the edge of the map.
    OffMap,
    /// Something is already standing there.
    Occupied,
    /// Rock, or shallows under something that is not a bridge, or dry land
    /// under a bridge.
    WrongGround,
    /// One Hearth per player, and the run starts with it.
    OneHearthOnly,
    NoSuchBuilding,
    NoSuchCitizen,
    /// The building is a site or rubble, and the command needed it standing.
    NotStanding,
    /// A dike cannot go above `DIKE_MAX_LEVEL`.
    TooHigh,
    /// There is no job of any kind at that building.
    NoJobThere,
    /// Every slot is taken.
    Full,
    /// `SetHome` at something that is not a cottage.
    NotACottage,
    /// A coordinate outside the map.
    NoSuchCell,
    /// No way to lay a road between those two cells.
    NoRoute,
    NoSuchRoad,
    NoSuchTrade,
    /// A road that ends nowhere near your city is not yours to accept.
    NotYourRoad,
    /// Trading with yourself, or with somebody who is not playing.
    NoSuchPartner,
    /// Already agreed.
    AlreadyAccepted,
    /// A quarry with no rock beside it.
    NoRockHere,
    /// Not enough gold.
    TooPoor,
    /// A hearth, a road or a bridge. See `Kind::movable`.
    CannotMove,
    /// A child. It does not haul, farm or build.
    TooYoung,
}

impl RuleError {
    /// Something a person can read, in the place they were looking.
    ///
    /// It lives here rather than in `gui` because the rule and the sentence
    /// that explains it are the same fact, and a rule added without a sentence
    /// would otherwise reach a player as silence — which is the failure mode
    /// `Lockstep::issue` exists to prevent, checking locally so a refusal
    /// arrives now instead of three ticks later on five machines at once.
    pub fn to_message(&self) -> &'static str {
        match self {
            RuleError::NotYours => "that is not yours",
            RuleError::OffMap => "it will not fit there",
            RuleError::Occupied => "something is already there",
            RuleError::WrongGround => "not on that ground",
            RuleError::OneHearthOnly => "one hearth to a city",
            RuleError::NoSuchBuilding => "there is nothing there",
            RuleError::NoSuchCitizen => "nobody there to ask",
            RuleError::NotStanding => "it is not built yet",
            RuleError::TooHigh => "that dike is as high as it goes",
            RuleError::NoJobThere => "there is no work there",
            RuleError::Full => "there is no room",
            RuleError::NotACottage => "only a cottage has beds",
            RuleError::NoSuchCell => "not on the map",
            RuleError::NoRoute => "no way through",
            RuleError::NoSuchRoad => "no such road",
            RuleError::NoSuchTrade => "no such offer",
            RuleError::NotYourRoad => "that road does not reach you",
            RuleError::NoSuchPartner => "there is no such city",
            RuleError::AlreadyAccepted => "already agreed",
            RuleError::NoRockHere => "a quarry needs rock beside it",
            RuleError::TooPoor => "not enough gold",
            RuleError::CannotMove => "that one stays where it is",
            RuleError::TooYoung => "too young to work",
        }
    }
}

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
    /// Indexed by `BuildingId`. Rubble stays in it, for the same reason.
    pub buildings: Vec<Building>,
    /// Which building is on each cell, or none. Design §3.1 puts "an optional
    /// building footprint" on the cell, and every walk, placement check and
    /// flood rule wants that answer in one step rather than by scanning the
    /// building list.
    ///
    /// Derived from `buildings`, and deliberately still part of the world and
    /// so of the checksum: if it ever disagrees with the list, that is a bug
    /// worth catching on the tick it happens rather than an invisible one.
    pub occupancy: Vec<Option<BuildingId>>,
    /// Players the game has given up on (design §8). Their city stands where
    /// it was; nothing of theirs moves and no command of theirs is accepted.
    pub dropped: Vec<PlayerId>,
    /// Players who have called for a pause. Design §6: it takes everyone's
    /// `Resume` to lift, so this is a list rather than a flag.
    pub paused_by: Vec<PlayerId>,
    /// Which age it is, counting from one.
    pub age: u32,
    /// The tick the current age began.
    pub age_start_tick: u32,
    /// This age's disaster, drawn at age start and deliberately not shown
    /// until the village has some reason to know it (design §4).
    pub disaster: Disaster,
    /// The tick the run ended, if it has.
    pub finished: Option<u32>,
    /// Why it ended. See `age::Ending` — it is what decides whether the last
    /// age counts on the score screen.
    pub ending: Option<Ending>,
    /// The most citizens each city has ever had, for the score screen.
    pub peak_population: Vec<u32>,
    /// Standing water. Design §3.1 puts a depth on every cell.
    pub water: Water,
    /// Roads that have been laid, in the order they were laid.
    /// Carts on the road, indexed by `MuleId`. Retired ones stay in it, for
    /// the reason every other id here does.
    pub mules: Vec<Mule>,
    /// Families, indexed by `HouseholdId`. Ended ones stay in it.
    pub households: Vec<Household>,
    pub roads: Vec<Road>,
    /// Standing trade agreements, proposed and accepted.
    pub trades: Vec<Trade>,
    /// Recent pings, for the GUI to draw. A command rather than a chat
    /// message so it lands on the same tick everywhere and replays with the
    /// game; pruned each tick so it cannot grow without bound.
    pub pings: Vec<Ping>,
    /// Bumped whenever a footprint appears or disappears.
    ///
    /// Flow fields are a cache outside `World` (see `nav`), and this is how
    /// they know they have gone stale. It lives in the world, and so in the
    /// checksum, because it counts something the world did: two peers whose
    /// generations differ have placed different numbers of buildings, and
    /// that is worth catching directly rather than inferring from the
    /// wreckage a day later.
    pub nav_generation: u32,
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
            // Around the hearth, and *outside* it. The Hearth's three-by-three
            // footprint blocks movement, so a party scattered within two cells
            // of its middle began the game standing in a wall — unorderable
            // until it wandered out, and drawn as one circle with eight people
            // inside it. A ring, walked in a fixed order, gives each of them a
            // cell of their own to stand on.
            let mut spots = spawn_ring(&map, hx, hy);
            for _ in 0..FOUNDING_CITIZENS {
                let (cx, cy) = spots.next().unwrap_or((hx, hy));
                // Jitter inside the cell, so a party is a crowd and not a
                // parade. Small enough that nobody starts on somebody else.
                let jx = Fx(rng.range(-48, 48));
                let jy = Fx(rng.range(-48, 48));
                let name = rng.below(NAMES.len() as u32) as u16;
                let id = CitizenId(citizens.len() as u16);
                citizens.push(Citizen::new(
                    id,
                    PlayerId(p as u8),
                    name,
                    V2::cell_centre(cx, cy) + V2::new(jx, jy),
                ));
            }
        }

        let mut world = World {
            seed,
            rng,
            tick: 0,
            map,
            citizens,
            buildings: Vec::new(),
            occupancy: vec![None; crate::map::CELLS],
            dropped: Vec::new(),
            paused_by: Vec::new(),
            age: 1,
            age_start_tick: 0,
            disaster: Disaster { kind: DisasterKind::Flood, sources: Vec::new(), height: 0 },
            finished: None,
            ending: None,
            peak_population: vec![0; players as usize],
            water: Water::dry(),
            mules: Vec::new(),
            households: Vec::new(),
            roads: Vec::new(),
            trades: Vec::new(),
            pings: Vec::new(),
            nav_generation: 0,
            players: (0..players).map(|p| PlayerId(p as u8)).collect(),
        };

        // Age one's disaster is drawn now, from the same seeded Rng that
        // built the map, so the whole run is a function of the seed.
        world.disaster = Disaster::draw(1, &world.map, &mut world.rng);
        world.peak_population = vec![FOUNDING_CITIZENS; players as usize];

        // The Hearths are already there when the run begins (design §4), each
        // holding what its city starts with. `level_pad` levelled a
        // HEARTH_SIZE square under every site during generation, so these
        // always fit.
        for p in 0..players {
            let (cx, cy) = world.map.hearth_sites[p as usize];
            let (w, h) = Kind::Hearth.size(Facing::EastWest);
            let id = BuildingId(world.buildings.len() as u16);
            let mut hearth = Building::standing(
                id,
                PlayerId(p as u8),
                Kind::Hearth,
                Facing::EastWest,
                cx - w / 2,
                cy - h / 2,
            );
            hearth.store = Goods::of(0, STARTING_WOOD, STARTING_STONE, 0);
            world.occupy(&hearth);
            world.buildings.push(hearth);
        }

        world
    }

    // ---- buildings ---------------------------------------------------------

    /// Which building covers a cell, if any.
    pub fn building_at(&self, x: i32, y: i32) -> Option<&Building> {
        if !Map::contains(x, y) {
            return None;
        }
        self.occupancy[Map::idx(x, y)].map(|id| &self.buildings[id.0 as usize])
    }

    /// Whether `kind` may be placed with its top-left at `(x, y)`.
    ///
    /// Split from `place` so the GUI can grey out an illegal site without
    /// issuing a command that will be rejected — and so the reason can be
    /// tested one at a time.
    pub fn can_place(
        &self,
        owner: PlayerId,
        kind: Kind,
        facing: Facing,
        x: i32,
        y: i32,
    ) -> Result<(), RuleError> {
        if !self.players.contains(&owner) {
            return Err(RuleError::NotYours);
        }
        if kind == Kind::Hearth {
            // The run starts with the only one a player gets.
            return Err(RuleError::OneHearthOnly);
        }
        if !Building::fits_on_map(kind, facing, x, y) {
            return Err(RuleError::OffMap);
        }
        if !Building::ground_suits(kind, facing, &self.map, x, y) {
            return Err(RuleError::WrongGround);
        }
        if !Building::neighbours_suit(kind, facing, &self.map, x, y) {
            return Err(RuleError::NoRockHere);
        }
        for (cx, cy) in Building::footprint(kind, facing, x, y) {
            if self.occupancy[Map::idx(cx, cy)].is_some() {
                return Err(RuleError::Occupied);
            }
        }
        Ok(())
    }

    /// How many of `citizens` this building would actually take.
    ///
    /// Split out of `assign` for the same reason `can_place` is split out of
    /// `place`: so the GUI can offer what will work instead of a command the
    /// rules will refuse. It matters more here, because a refusal is
    /// all-or-nothing (see DECISIONS.md). Selecting a whole city of eight and
    /// right-clicking a farm asks to put eight people in three slots, and the
    /// answer is not "three of them start farming" but `Full` — *nobody* is
    /// assigned, the only sign is a line under the map that fades, and the
    /// city starves on day four with a farm standing empty in the middle of
    /// it. That is the single most natural gesture in the game and it did
    /// nothing at all.
    ///
    /// Counts the way `assign` counts: whoever is already there and is *not*
    /// named in the list still holds their slot.
    pub fn will_take(&self, owner: PlayerId, building: BuildingId, citizens: &[CitizenId]) -> usize {
        let Some(b) = self.buildings.get(building.0 as usize) else {
            return 0;
        };
        if b.owner != owner {
            return 0;
        }
        let job = match b.state {
            BuildState::Site => Job::Builder,
            BuildState::Standing => match Job::at(b.kind) {
                Some(j) => j,
                None if b.kind.is_store() => Job::Hauler,
                None => return 0,
            },
            _ => return 0,
        };
        let slots = b.slots_for(job);
        if slots == usize::MAX {
            return citizens.len();
        }
        let held = self
            .citizens
            .iter()
            .filter(|c| c.alive() && c.workplace == Some(building) && !citizens.contains(&c.id))
            .count();
        citizens.len().min(slots.saturating_sub(held))
    }

    /// The same question for a cottage's beds, which `SetHome` limits the same
    /// way and would refuse the same way.
    pub fn will_house(&self, owner: PlayerId, cottage: BuildingId, citizens: &[CitizenId]) -> usize {
        let Some(b) = self.buildings.get(cottage.0 as usize) else {
            return 0;
        };
        if b.owner != owner || b.kind != Kind::Cottage || !b.standing_now() {
            return 0;
        }
        let held = self
            .citizens
            .iter()
            .filter(|c| c.alive() && c.home == Some(cottage) && !citizens.contains(&c.id))
            .count();
        citizens.len().min(b.beds().saturating_sub(held))
    }

    /// Start a construction site. Materials still have to be hauled to it and
    /// builder-ticks spent on it before it is anything.
    pub fn place(
        &mut self,
        owner: PlayerId,
        kind: Kind,
        facing: Facing,
        x: i32,
        y: i32,
    ) -> Result<BuildingId, RuleError> {
        self.can_place(owner, kind, facing, x, y)?;
        let id = BuildingId(self.buildings.len() as u16);
        let mut b = Building::site(id, owner, kind, facing, x, y);
        if kind == Kind::Dike {
            // One draw, here, so the wall a player is looking at has the
            // toughness it will still have in an hour. See
            // `balance::FOOTING_SPREAD`.
            let spread = FOOTING_SPREAD as i32;
            b.footing = (100 + self.rng.range(-spread, spread)) as u8;
        }
        self.occupy(&b);
        self.buildings.push(b);
        Ok(id)
    }

    /// Raise an existing dike by one level. Costs another dike's worth of
    /// stone, hauled and built like the first.
    pub fn raise_dike(&mut self, owner: PlayerId, id: BuildingId) -> Result<(), RuleError> {
        let b = self.buildings.get_mut(id.0 as usize).ok_or(RuleError::NoSuchBuilding)?;
        if b.owner != owner {
            return Err(RuleError::NotYours);
        }
        if b.kind != Kind::Dike || b.state != BuildState::Standing {
            return Err(RuleError::NotStanding);
        }
        if b.level >= DIKE_MAX_LEVEL {
            return Err(RuleError::TooHigh);
        }
        b.level += 1;
        b.state = BuildState::Site;
        // Part-built, because the bank below the new course is already there.
        // See `balance::DIKE_RAISE_PERCENT`.
        b.progress = Kind::Dike.build_ticks() * (100 - DIKE_RAISE_PERCENT) / 100;
        Ok(())
    }

    /// Buy a building one more level, in gold.
    ///
    /// Paid at once out of the city's stores rather than hauled: gold is not
    /// hauled (`Good::hauled`), and a level is a wage rather than a wall.
    /// Unlike raising a dike it does not put the building back to a site — the
    /// building is not being made bigger, it is being given another pair of
    /// hands, and a farm that stopped feeding anybody while its fourth farmer
    /// was hired would be a strange thing to sell.
    pub fn upgrade(&mut self, owner: PlayerId, id: BuildingId) -> Result<(), RuleError> {
        let b = self.buildings.get(id.0 as usize).ok_or(RuleError::NoSuchBuilding)?;
        if b.owner != owner {
            return Err(RuleError::NotYours);
        }
        if !b.standing_now() {
            return Err(RuleError::NotStanding);
        }
        if !b.kind.upgradable() {
            return Err(RuleError::NoJobThere);
        }
        if b.level >= MAX_LEVEL {
            return Err(RuleError::TooHigh);
        }
        let cost = b.kind.upgrade_cost(b.level);
        if !self.treasury(owner).covers(&cost) {
            return Err(RuleError::TooPoor);
        }
        for g in Good::ALL {
            let n = cost.get(g);
            if n > 0 {
                self.take_from_stores(owner, g, n);
            }
        }
        self.buildings[id.0 as usize].level += 1;
        Ok(())
    }

    /// Whether a building may be picked up and put down at `(x, y)`.
    ///
    /// Split from `move_building` the way `can_place` is split from `place`,
    /// and for the same reason: the ghost under the cursor has to be able to
    /// ask without issuing a command the rules will refuse.
    ///
    /// Validated exactly the way placement is, but **ignoring the building's
    /// own cells** — otherwise a building would refuse to shuffle one step
    /// because it is already standing in the way.
    pub fn can_move(
        &self,
        owner: PlayerId,
        id: BuildingId,
        x: i32,
        y: i32,
    ) -> Result<(), RuleError> {
        let b = self.buildings.get(id.0 as usize).ok_or(RuleError::NoSuchBuilding)?;
        if b.owner != owner {
            return Err(RuleError::NotYours);
        }
        if b.state == BuildState::Rubble {
            return Err(RuleError::NotStanding);
        }
        if !b.kind.movable() {
            return Err(RuleError::CannotMove);
        }
        let (kind, facing) = (b.kind, b.facing);
        if !Building::fits_on_map(kind, facing, x, y) {
            return Err(RuleError::OffMap);
        }
        if !Building::ground_suits(kind, facing, &self.map, x, y) {
            return Err(RuleError::WrongGround);
        }
        if !Building::neighbours_suit(kind, facing, &self.map, x, y) {
            return Err(RuleError::NoRockHere);
        }
        for (cx, cy) in Building::footprint(kind, facing, x, y) {
            match self.occupancy[Map::idx(cx, cy)] {
                Some(other) if other != id => return Err(RuleError::Occupied),
                _ => {}
            }
        }
        Ok(())
    }

    /// Pick a building up and put it down somewhere else.
    ///
    /// It keeps its id, so everybody who worked or lived there keeps pointing
    /// at it and simply re-paths to the new address — no new machinery at all,
    /// which is the elegant half. It keeps its store and its level too, and it
    /// arrives as a construction site with its materials already delivered: so
    /// a move costs the builder-ticks again and none of the materials.
    ///
    /// Being a site while it moves is the price. It shelters nobody and
    /// produces nothing until it is finished, which makes moving the granary
    /// the day before the water comes a real decision rather than a free tidy
    /// up.
    pub fn move_building(
        &mut self,
        owner: PlayerId,
        id: BuildingId,
        x: i32,
        y: i32,
    ) -> Result<(), RuleError> {
        self.can_move(owner, id, x, y)?;
        let was: Vec<(i32, i32)> = self.buildings[id.0 as usize].cells().collect();

        for (cx, cy) in was {
            self.occupancy[Map::idx(cx, cy)] = None;
        }
        let b = &mut self.buildings[id.0 as usize];
        b.x = x as u8;
        b.y = y as u8;
        // Back to a site with its materials in it: the earth has to be moved
        // again, the timber does not.
        b.state = BuildState::Site;
        b.progress = 0;
        b.delivered = b.total_cost();
        b.workers.clear();
        let moved = self.buildings[id.0 as usize].clone();
        self.occupy(&moved);
        self.nav_generation += 1;
        Ok(())
    }

    /// Pull a building down. Its salvage goes to the owner's nearest store.
    pub fn demolish(&mut self, owner: PlayerId, id: BuildingId) -> Result<Goods, RuleError> {
        let b = self.buildings.get(id.0 as usize).ok_or(RuleError::NoSuchBuilding)?;
        if b.owner != owner {
            return Err(RuleError::NotYours);
        }
        let salvage = b.salvage();
        let cells: Vec<(i32, i32)> = b.cells().collect();
        for (cx, cy) in cells {
            if Map::contains(cx, cy) {
                self.occupancy[Map::idx(cx, cy)] = None;
            }
        }
        self.nav_generation += 1;
        let b = &mut self.buildings[id.0 as usize];
        b.state = BuildState::Rubble;
        b.integrity = 0;
        b.store = Goods::NONE;
        b.workers.clear();
        self.release_from(id);
        Ok(salvage)
    }

    /// The ground a flood sees: terrain, plus whatever a standing dike adds.
    pub fn effective_height(&self, x: i32, y: i32) -> u16 {
        let base = self.map.height_at(x, y) as u16;
        match self.building_at(x, y) {
            Some(b) => base.saturating_add(b.ground_bonus()),
            None => base,
        }
    }

    /// Every standing store of `owner` that holds `good`, nearest first from
    /// `(x, y)`. Ordered by distance and then by id, so ties break the same
    /// way on every peer.
    pub fn stores_for(&self, owner: PlayerId, good: Good, x: i32, y: i32) -> Vec<BuildingId> {
        let mut found: Vec<(i32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|b| b.owner == owner && b.standing_now() && b.kind.stores(good))
            .map(|b| {
                let (bx, by) = b.centre();
                ((bx - x).abs() + (by - y).abs(), b.id)
            })
            .collect();
        found.sort_unstable();
        found.into_iter().map(|(_, id)| id).collect()
    }

    /// Haul materials to a site. Returns what it accepted.
    pub fn deliver_to(&mut self, id: BuildingId, g: Good, amount: u16) -> u16 {
        match self.buildings.get_mut(id.0 as usize) {
            Some(b) => b.deliver(g, amount),
            None => 0,
        }
    }

    /// Apply builder-ticks to a site. Returns true on the tick it finishes.
    ///
    /// This exists rather than letting callers reach into `buildings` because
    /// of one case: the tick a Road or a Bridge is finished is the tick the
    /// map becomes passable somewhere it was not, and every cached flow field
    /// is then wrong. Routing construction through here is what makes that
    /// impossible to forget.
    pub fn build_at(&mut self, id: BuildingId, effort: u32) -> bool {
        let finished = match self.buildings.get_mut(id.0 as usize) {
            Some(b) => b.build(effort),
            None => false,
        };
        if finished {
            self.nav_generation += 1;
        }
        finished
    }

    /// Damage a building. Returns true on the tick it becomes rubble.
    ///
    /// A ruined road is ordinary ground again and a ruined bridge is open
    /// water, so this bumps the generation for the same reason `build_at`
    /// does. Rubble keeps its footprint until somebody clears it with
    /// `demolish`.
    pub fn damage_building(&mut self, id: BuildingId, amount: u16) -> bool {
        let ruined = match self.buildings.get_mut(id.0 as usize) {
            Some(b) => b.damage(amount),
            None => false,
        };
        if ruined {
            self.nav_generation += 1;
            self.release_from(id);
        }
        ruined
    }

    /// Let go of everybody who depended on a building that is no longer there.
    ///
    /// Called wherever a building becomes rubble, rather than only where the
    /// flood does it. The first version lived inside the flood, so a cottage
    /// pulled down by its owner left its residents homed to a hole in the
    /// ground until something else happened to notice.
    pub(crate) fn release_from(&mut self, id: BuildingId) {
        for i in 0..self.citizens.len() {
            if self.citizens[i].workplace == Some(id) {
                self.citizens[i].workplace = None;
                self.citizens[i].job = None;
                self.citizens[i].abandon();
            }
            if self.citizens[i].dest == Some(Dest::Building(id)) {
                self.citizens[i].halt();
            }
            if self.citizens[i].home == Some(id) {
                self.citizens[i].home = None;
            }
        }
    }

    /// Mark a building's cells as taken. Private, because the occupancy grid
    /// and the building list must only ever change together.
    fn occupy(&mut self, b: &Building) {
        for (cx, cy) in b.cells() {
            if Map::contains(cx, cy) {
                self.occupancy[Map::idx(cx, cy)] = Some(b.id);
            }
        }
        self.nav_generation += 1;
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

    /// What this player's people eat in a day, in units of `Good::Food`.
    ///
    /// Children included: M9 made a child a citizen in every way but work, and
    /// the way that matters here is that it eats.
    pub fn eaten_a_day(&self, owner: PlayerId) -> u32 {
        self.population(owner) * crate::balance::FOOD_A_DAY
    }

    /// Whole days the granary will feed this city for, if nothing is grown.
    ///
    /// The number both players in the M10.5 rehearsal said would have saved
    /// them. The panel could say "the granary is empty" and did; what it could
    /// not say was how long that had left to run, and a city with a staffed
    /// farm and an empty granary cannot tell "too slow" from "not working".
    ///
    /// `None` when there is nobody left to feed, which is not the same answer
    /// as nought days and must not read like it.
    /// Meals count for what they are worth, not for what they are: one meal
    /// feeds a citizen as far as `MEAL_WORTH` raw units do, and a city that
    /// had cooked its whole larder would otherwise be told it had half the
    /// days it has. This number is the amber line's, and a wrong one here is
    /// worse than none.
    pub fn days_of_food(&self, owner: PlayerId) -> Option<u32> {
        let eaten = self.eaten_a_day(owner);
        let t = self.treasury(owner);
        let stored = t.food as u32 + t.meal as u32 * crate::balance::MEAL_WORTH as u32;
        (eaten > 0).then(|| stored / eaten)
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
    ///
    /// `nav` is passed in rather than owned because flow fields are a cache
    /// and `World` is the authoritative state; see the `nav` module. It is
    /// `&mut` because a tick may need a field that has not been built yet.
    /// The order of the stages is itself a decision:
    ///
    /// 1. **needs** — everybody gets hungrier and more tired, and anyone whose
    ///    three days are up dies. First, so the rest of the tick is decided by
    ///    the state a citizen is actually in.
    /// 2. **the dead are taken off the rosters** — before production, so a
    ///    farm does not harvest one more tick from someone who starved on it.
    /// 3. **errands** — the living decide, or are overruled by their bodies.
    /// 4. **walking** — one step along a shared field.
    /// 5. **arrivals** — whoever got there does the thing they went for.
    /// 6. **production** — producers turn worker-ticks into goods, after
    ///    arrivals so that a farmer who reached the field this tick counts on
    ///    it.
    /// 7. **the crowd settles** — last, so that whatever moved somebody, they
    ///    end the tick out of the walls and out of each other.
    pub fn tick(&mut self, nav: &mut Nav, commands: &[(PlayerId, Command)]) {
        // Commands first, and in the order given. The lockstep is what
        // guarantees every peer sees that order identically (design §8); `sim`
        // just has to be a function of it.
        //
        // A rejected command is not an error here. Every peer rejects it for
        // the same reason on the same tick, which is the property that
        // matters; the sender's own GUI can call `apply` itself to find out
        // why before it ever issues one.
        if self.finished.is_some() {
            // The run is over. Nothing more happens, on any peer.
            return;
        }

        for (player, cmd) in commands {
            let _ = self.apply(*player, cmd);
        }

        if self.paused() {
            // Nothing moves, and the clock does not advance. Every peer agrees
            // about that, because `paused_by` is world state like any other.
            return;
        }

        for i in 0..self.citizens.len() {
            let was_alive = self.citizens[i].alive();
            self.citizens[i].tick_needs(self.tick);
            if was_alive && !self.citizens[i].alive() {
                self.clear_from_rosters(CitizenId(i as u16));
            }
        }
        self.families();
        self.assign_errands();
        self.walk(nav);
        self.resolve_arrivals();
        self.produce();
        self.step_water();
        self.flood_bodies();
        // Last, after everything that moves anybody: walking, the flood
        // carrying bodies about, a citizen stepping out of the building it
        // started inside. Doing it once at the end is what makes "nobody is
        // standing in a wall, and nobody is standing in anybody else" true
        // whatever put them there.
        self.settle_crowd();
        self.tick += 1;
        if self.tick % TICKS_PER_DAY == 0 {
            self.trade_day();
        }
        self.tick_clock();
        self.pings.retain(|p| self.tick.saturating_sub(p.at) < PING_LIFETIME);
    }

    /// Whether the world is stopped.
    pub fn paused(&self) -> bool {
        !self.paused_by.is_empty()
    }

    // ---- the one door ------------------------------------------------------

    /// Apply one command on behalf of one player.
    ///
    /// Design §7: this is the only way `World` changes, and every rule is
    /// inside it — including ownership, so a peer commanding another city is
    /// rejected identically everywhere.
    ///
    /// Rejection is all-or-nothing for commands that name several citizens: a
    /// `MoveTo` listing eight of your citizens and one of mine does nothing at
    /// all, rather than moving eight. Half-applied commands are the shape of
    /// bug that makes two peers disagree about what a rejection meant.
    pub fn apply(&mut self, player: PlayerId, cmd: &Command) -> Result<(), RuleError> {
        if !self.players.contains(&player) {
            return Err(RuleError::NotYours);
        }
        // A dropped player is gone: nothing they say is heard, including a
        // command that arrived on the wire before they went quiet.
        if self.dropped.contains(&player) && !matches!(cmd, Command::Drop { .. }) {
            return Err(RuleError::NotYours);
        }
        // Every citizen a command speaks for must belong to the player, and
        // must be alive. Checked up front, for all variants at once.
        for id in cmd.citizens() {
            let c = self.citizens.get(id.0 as usize).ok_or(RuleError::NoSuchCitizen)?;
            if c.owner != player {
                return Err(RuleError::NotYours);
            }
            if !c.alive() {
                return Err(RuleError::NoSuchCitizen);
            }
        }

        match cmd {
            Command::Place { kind, facing, x, y } => {
                self.place(player, *kind, *facing, *x as i32, *y as i32).map(|_| ())
            }
            Command::Demolish { building } => self.demolish(player, *building).map(|_| ()),
            Command::RaiseDike { dike } => self.raise_dike(player, *dike),
            Command::Upgrade { building } => self.upgrade(player, *building),
            Command::Move { building, x, y } => {
                self.move_building(player, *building, *x as i32, *y as i32)
            }
            Command::Assign { citizens, building } => self.assign(player, citizens, *building),
            Command::Unassign { citizens } => {
                for id in citizens {
                    self.unassign_one(*id);
                }
                Ok(())
            }
            Command::MoveTo { citizens, x, y } => {
                if !Map::contains(*x as i32, *y as i32) {
                    return Err(RuleError::NoSuchCell);
                }
                for id in citizens {
                    // The job goes too. "Get uphill" means drop what you are
                    // doing, not "visit that hill and come back to the farm".
                    self.unassign_one(*id);
                    let c = &mut self.citizens[id.0 as usize];
                    c.abandon();
                    c.held = true;
                    c.walk_to(Dest::Cell(*x, *y));
                }
                Ok(())
            }
            Command::SetHome { citizens, cottage } => {
                let b = self
                    .buildings
                    .get(cottage.0 as usize)
                    .ok_or(RuleError::NoSuchBuilding)?;
                if b.owner != player {
                    return Err(RuleError::NotYours);
                }
                if b.kind != Kind::Cottage {
                    return Err(RuleError::NotACottage);
                }
                if !b.standing_now() {
                    return Err(RuleError::NotStanding);
                }
                // Beds are finite, and the ones already spoken for by citizens
                // not named in this command still count.
                let outsiders = self
                    .citizens
                    .iter()
                    .filter(|c| c.home == Some(*cottage) && !citizens.contains(&c.id))
                    .count();
                if outsiders + citizens.len() > self.buildings[cottage.0 as usize].beds() {
                    return Err(RuleError::Full);
                }
                for id in citizens {
                    self.citizens[id.0 as usize].home = Some(*cottage);
                }
                Ok(())
            }
            Command::Ping { x, y } => {
                if !Map::contains(*x as i32, *y as i32) {
                    return Err(RuleError::NoSuchCell);
                }
                self.pings.push(Ping { by: player, x: *x, y: *y, at: self.tick });
                Ok(())
            }
            Command::DikeLine { from, to } => self.lay_dike_line(player, *from, *to).map(|_| ()),
            Command::Road { from, to } => self.lay_road(player, *from, *to).map(|_| ()),
            Command::AcceptRoad { road } => {
                let r = self.roads.get_mut(road.0 as usize).ok_or(RuleError::NoSuchRoad)?;
                if r.reaches != Some(player) {
                    return Err(RuleError::NotYourRoad);
                }
                if r.joined {
                    return Err(RuleError::AlreadyAccepted);
                }
                r.joined = true;
                Ok(())
            }
            Command::Trade { with, give, take } => {
                if *with == player || !self.players.contains(with) {
                    return Err(RuleError::NoSuchPartner);
                }
                let id = TradeId(self.trades.len() as u16);
                self.trades.push(Trade {
                    id,
                    from: player,
                    with: *with,
                    give: *give,
                    take: *take,
                    accepted: false,
                });
                Ok(())
            }
            Command::AcceptTrade { trade } => {
                let t = self.trades.get_mut(trade.0 as usize).ok_or(RuleError::NoSuchTrade)?;
                // Only the other party, and only once.
                if t.with != player {
                    return Err(RuleError::NotYours);
                }
                if t.accepted {
                    return Err(RuleError::AlreadyAccepted);
                }
                t.accepted = true;
                Ok(())
            }
            Command::Drop { player: who } => {
                if !self.players.contains(who) {
                    return Err(RuleError::NoSuchPartner);
                }
                if !self.dropped.contains(who) {
                    self.dropped.push(*who);
                    // Whatever they had going, they are no longer doing.
                    for i in 0..self.citizens.len() {
                        if self.citizens[i].owner == *who {
                            self.citizens[i].abandon();
                        }
                    }
                    // And they are not holding the game paused any more.
                    self.paused_by.retain(|p| p != who);
                }
                Ok(())
            }
            Command::Pause => {
                if !self.paused_by.contains(&player) {
                    self.paused_by.push(player);
                }
                Ok(())
            }
            Command::Resume => {
                self.paused_by.retain(|&p| p != player);
                Ok(())
            }
        }
    }

    // ---- roads and trade ---------------------------------------------------

    /// Lay a road along the cheapest path between two cells.
    ///
    /// Every cell of the path that is not already a way becomes a *site* —
    /// design §6 says "builders from the ordering city construct it", so this
    /// marks out the route and the city's builders and haulers do the rest.
    /// Cells that already carry traffic are reused and cost nothing.
    fn lay_road(
        &mut self,
        player: PlayerId,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Result<RoadId, RuleError> {
        let a = (from.0 as i32, from.1 as i32);
        let b = (to.0 as i32, to.1 as i32);
        if !Map::contains(a.0, a.1) || !Map::contains(b.0, b.1) {
            return Err(RuleError::NoSuchCell);
        }
        let path = road::plan(self, a, b).ok_or(RuleError::NoRoute)?;

        let mut cells = Vec::with_capacity(path.len());
        for (x, y) in path {
            cells.push((x as u8, y as u8));
            if self.building_at(x, y).is_some() {
                continue; // an existing way, reused
            }
            // A bridge where it must cross water, a road everywhere else —
            // and **a ford is water**. This asked for `Shallows` exactly, so a
            // road routed over the ford laid road cells on it, every one of
            // them was refused for standing on water, and the road came out
            // with a hole in it that nothing reported. `Road::intact` then said
            // the two cities were not linked and the trade moved nothing.
            let kind = if self.map.ground_at(x, y).watery() {
                Kind::Bridge
            } else {
                Kind::Road
            };
            // `plan` already established every cell is layable, so this cannot
            // fail for a reason the player could act on.
            let _ = self.place(player, kind, Facing::EastWest, x, y);
        }

        let end = (b.0, b.1);
        let reaches = road::city_at(self, player, end.0, end.1);
        let id = RoadId(self.roads.len() as u16);
        self.roads.push(Road { id, by: player, reaches, joined: false, cells });
        Ok(id)
    }

    /// Where a dike line drawn from `from` to `to` would put its segments.
    ///
    /// Answered without changing anything so the drag tool can draw the ghost
    /// and total the cost from the same arithmetic that will lay the wall —
    /// the mistake the letterbox made twice was two places doing one sum.
    ///
    /// A wall is straight. The run snaps to whichever axis it is longer along
    /// and keeps the cell it started from, because a road takes the cheapest
    /// path and a wall stays where you drew it. It also snaps to a whole
    /// number of segments, so the far end can overshoot the cursor by up to
    /// `DIKE_LENGTH - 1`: a wall with a one-cell hole in it is not a wall, and
    /// the ghost shows the run that will actually be built.
    ///
    /// Segments the ground or another building refuses are skipped rather than
    /// failing the whole line, so drawing across a farm walls both sides of it.
    pub fn plan_dike_line(
        &self,
        owner: PlayerId,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Vec<(i32, i32)> {
        let a = (from.0 as i32, from.1 as i32);
        let b = (to.0 as i32, to.1 as i32);
        if !Map::contains(a.0, a.1) || !Map::contains(b.0, b.1) {
            return Vec::new();
        }
        let facing = Facing::of_run(a, b);
        // Along the run, and across it — the axis the wall does not move on.
        let (start, end, fixed) = match facing {
            Facing::EastWest => (a.0, b.0, a.1),
            Facing::NorthSouth => (a.1, b.1, a.0),
        };
        let (lo, hi) = (start.min(end), start.max(end));

        let mut segments = Vec::new();
        let mut at = lo;
        while at <= hi {
            let (x, y) = match facing {
                Facing::EastWest => (at, fixed),
                Facing::NorthSouth => (fixed, at),
            };
            if self.can_place(owner, Kind::Dike, facing, x, y).is_ok() {
                segments.push((x, y));
            }
            at += DIKE_LENGTH;
        }
        segments
    }

    /// Lay a wall along a straight run. Returns what was actually built, which
    /// is empty when every segment was refused.
    pub fn lay_dike_line(
        &mut self,
        owner: PlayerId,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Result<Vec<BuildingId>, RuleError> {
        if !Map::contains(from.0 as i32, from.1 as i32)
            || !Map::contains(to.0 as i32, to.1 as i32)
        {
            return Err(RuleError::NoSuchCell);
        }
        let facing = Facing::of_run(
            (from.0 as i32, from.1 as i32),
            (to.0 as i32, to.1 as i32),
        );
        let mut placed = Vec::new();
        for (x, y) in self.plan_dike_line(owner, from, to) {
            // `plan_dike_line` asked `can_place` for every one of these and
            // nothing has moved since, so a failure here would be a bug rather
            // than something the player could act on.
            if let Ok(id) = self.place(owner, Kind::Dike, facing, x, y) {
                placed.push(id);
            }
        }
        if placed.is_empty() {
            // The whole run was refused, so the player gets the reason the
            // cell they started from was refused rather than silence.
            return Err(self
                .can_place(owner, Kind::Dike, facing, from.0 as i32, from.1 as i32)
                .err()
                .unwrap_or(RuleError::Occupied));
        }
        Ok(placed)
    }

    /// The height of every cell as the water sees it: terrain, plus whatever
    /// a standing dike adds.
    ///
    /// Built fresh each tick rather than cached. Sixteen thousand lookups is
    /// nothing next to the automaton that follows, and a cache would be one
    /// more thing to invalidate when a dike finishes or a flood takes one.
    pub fn ground_heights(&self) -> Vec<i32> {
        let mut g: Vec<i32> = self.map.height.iter().map(|&h| h as i32).collect();
        for b in &self.buildings {
            let bonus = b.ground_bonus();
            if bonus == 0 {
                continue;
            }
            for (x, y) in b.cells() {
                if Map::contains(x, y) {
                    g[Map::idx(x, y)] += bonus as i32;
                }
            }
        }
        g
    }

    /// One tick of water: pour in whatever the surge is pouring, then let it
    /// find its level.
    pub fn step_water(&mut self) {
        let sea = self.sea_surface();
        self.inject_surge();
        if self.water.volume() > 0 {
            let ground = self.ground_heights();
            self.water.step(&ground, sea);
        }
    }

    /// The level of the sea beyond the edges of the map.
    ///
    /// Zero except during a surge, when it is the surge's own height above the
    /// ground it is pouring onto — because a storm surge is the sea being
    /// high, and a sea that stays at zero while the river mouth is held at
    /// eighteen simply drains the flood back out beside where it came in.
    fn sea_surface(&self) -> i32 {
        if self.surging() == 0 {
            return 0;
        }
        // Measured at the river's *outfall*, not at its source.
        //
        // The sea is at the low end of the map, and this is the level water
        // has to climb to before it can leave — so taking it from the high end
        // was a flood that could not drain anywhere at all. Measured: the
        // whole map went wet (fifteen thousand cells of sixteen) at eleven
        // sixteenths a cell, which is a damp map rather than a flood. A storm
        // surge is the sea being high while it happens, and the sea is down
        // there.
        let rise = depth(self.disaster.height) as i32;
        let (ex, ey) = self.river_outfall();
        self.map.height_at(ex, ey) as i32 * DEPTH_SCALE as i32 + rise
    }

    /// The downstream end of the channel: where the water leaves.
    pub fn river_outfall(&self) -> (i32, i32) {
        match self.map.river.last() {
            Some(&(x, y)) => (x as i32, y as i32),
            None => self.map.low_corner.cell(),
        }
    }

    /// The upstream end of the channel: where the water comes from.
    ///
    /// The centreline is stored from source to mouth, so this is its first
    /// cell. A map with no river at all — which the generator does not make —
    /// falls back to the low corner, so nothing downstream of here has to
    /// carry an `Option` for a case that cannot happen.
    pub fn river_mouth(&self) -> (i32, i32) {
        match self.map.river.first() {
            Some(&(x, y)) => (x as i32, y as i32),
            None => self.map.low_corner.cell(),
        }
    }

    /// The surge: for `SURGE_TICKS`, hold the top of the channel at the age's
    /// height and the reach below it at half, and let the river carry it
    /// (design §5).
    ///
    /// Not a scripted wave — a source strong enough that the automaton makes a
    /// front out of it, which is the difference between water that behaves and
    /// water that has been animated.
    ///
    /// **This used to fill an 8 x 8 block at a corner and a second block one
    /// step inland.** The shape is the same and so is the reason for it: the
    /// first reach is the volume, the second is the shove, and without the
    /// second an injected source stops the moment its neighbours are as deep
    /// as it is. What changed is that the shove now has a channel to go down,
    /// so it spills over the banks where the banks are low instead of spreading
    /// evenly in every direction.
    ///
    /// Held *to* a depth, not topped up by one. Adding was the first version
    /// and it accumulated without limit: three hundred ticks of pumping piled
    /// water three hundred and seventy units deep beside a surge whose stated
    /// height was twelve. §5 says the source "sets depth = H", and a set is a
    /// cap.
    fn inject_surge(&mut self) {
        let pulses = self.surging();
        if pulses == 0 {
            return;
        }
        // `Disaster::height` is design §5's surge height, in terrain units —
        // "height 12" means twelve of the same units the map is drawn in.
        // Water is kept in sixteenths of one, so the two have to be converted
        // rather than compared. Left unscaled, an age-one flood poured water
        // three quarters of a unit deep and the map barely got wet.
        //
        // Two pulses at once is more water, not two floods: the deeper of the
        // two wins, which is what "sets depth = H" means when they overlap.
        let height = depth(self.disaster.height) * pulses.min(2) as u16;
        let push = surge_push(self.disaster.height) * pulses.min(2) as u16;


        let river: Vec<(i32, i32)> =
            self.map.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
        // The whole cut, banks included, not just the floor: a source that
        // fills only the five cells of the channel is a source the banks
        // contain, and design §5 wants a wave that tops them.
        let wide = RIVER_HALF_WIDTH + RIVER_BANK;

        for (i, &(cx, cy)) in river.iter().enumerate().take(2 * SURGE_REACH as usize) {
            let over = if (i as i32) < SURGE_REACH { height } else { push };
            // The surface the source holds, not the depth it holds.
            //
            // A river in flood rises to its banks and over them; it does not
            // rise to a fixed number of sixteenths wherever it happens to be.
            // Holding a depth made the flood a different flood on every map —
            // `which_dikes_break` found level-one walls losing anywhere from
            // 67% to 93% of their segments across five seeds — because the
            // same depth in a deep confined channel is a river and in a
            // shallow one is a lake. The top of the bank is the channel floor
            // plus `RIVER_DEPTH`, and the surge stands `over` above that.
            let bed = self.map.height_at(cx, cy) as i32;
            let surface = (bed + RIVER_DEPTH) * DEPTH_SCALE as i32 + over as i32;

            for dy in -wide..=wide {
                for dx in -wide..=wide {
                    let (x, y) = (cx + dx, cy + dy);
                    if !Map::contains(x, y) {
                        continue;
                    }
                    let ground = self.map.height_at(x, y) as i32 * DEPTH_SCALE as i32;
                    let want = (surface - ground).clamp(0, u16::MAX as i32) as u16;
                    self.water.raise_to(x, y, want);
                }
            }
        }
    }

    /// Whether two cities have a road between them that is joined and whole.
    pub fn linked(&self, a: PlayerId, b: PlayerId) -> bool {
        self.roads.iter().any(|r| r.links(a, b) && r.intact(self))
    }

    /// A day's trade: send the caravans out.
    ///
    /// Once a day, not every tick, because a standing agreement is a daily
    /// exchange (design §6). Nothing is teleported: haulers are given the
    /// errand and they walk it, so a hauler that drowns on the way loses the
    /// cargo exactly as the design says.
    fn trade_day(&mut self) {
        for i in 0..self.trades.len() {
            let t = self.trades[i];
            if !t.accepted || !self.linked(t.from, t.with) {
                continue;
            }
            self.send_caravan(t.from, t.with, t.give.0, t.give.1);
            self.send_caravan(t.with, t.from, t.take.0, t.take.1);
        }
    }

    /// Load up to `amount` of `good` onto idle haulers of `from`, bound for a
    /// store of `to`.
    fn send_caravan(&mut self, from: PlayerId, to: PlayerId, good: Good, amount: u16) {
        if amount == 0 {
            return;
        }
        let source = self
            .buildings
            .iter()
            .find(|b| b.owner == from && b.standing_now() && b.kind.stores(good) && b.store.get(good) > 0)
            .map(|b| b.id);
        let Some(source) = source else {
            return;
        };
        let (sx, sy) = self.buildings[source.0 as usize].centre();
        let Some(&target) = self
            .stores_for(to, good, sx, sy)
            .iter()
            .find(|id| {
                let b = &self.buildings[id.0 as usize];
                b.kind.has_room_for(good, &b.store)
            })
        else {
            return;
        };

        // Split the load between a few carriers, so a bigger trade is a longer
        // line of people rather than one citizen with a mountain on their back.
        let per = (amount as usize).div_ceil(CARAVAN_SIZE).max(1) as u16;
        let mut left = amount;
        for i in 0..self.citizens.len() {
            if left == 0 {
                break;
            }
            let c = &self.citizens[i];
            // Unassigned citizens only — a farmer is not pulled off the field
            // to walk to another city. A hauler already carrying something is
            // left alone, because taking it off that errand would drop the
            // load; one merely on its way to pick something up has nothing to
            // lose and is fair game.
            if c.owner != from || !c.alive() || c.job.is_some() {
                continue;
            }
            if c.busy() && !c.carrying.is_empty() {
                continue;
            }
            self.citizens[i].abandon();
            self.citizens[i].errand =
                Some(Errand::Collect { from: source, good, to: target });
            self.citizens[i].walk_to(Dest::Building(source));
            left = left.saturating_sub(per);
        }
    }

    /// Put citizens to work at a building. Which job it is follows from what
    /// the building is, exactly as right-clicking it would.
    fn assign(
        &mut self,
        player: PlayerId,
        citizens: &[CitizenId],
        building: BuildingId,
    ) -> Result<(), RuleError> {
        let b = self.buildings.get(building.0 as usize).ok_or(RuleError::NoSuchBuilding)?;
        if b.owner != player {
            return Err(RuleError::NotYours);
        }
        if citizens
            .iter()
            .any(|id| self.citizens.get(id.0 as usize).is_some_and(|c| c.is_child()))
        {
            return Err(RuleError::TooYoung);
        }
        let job = match b.state {
            // Anything half-built wants builders, whatever it is going to be.
            BuildState::Site => Job::Builder,
            BuildState::Standing => match Job::at(b.kind) {
                Some(j) => j,
                None if b.kind.is_store() => Job::Hauler,
                None => return Err(RuleError::NoJobThere),
            },
            _ => return Err(RuleError::NoJobThere),
        };

        // Slots are finite, and citizens already there who are not named in
        // this command still hold theirs.
        if job != Job::Hauler {
            let held = self
                .citizens
                .iter()
                .filter(|c| {
                    c.alive() && c.workplace == Some(building) && !citizens.contains(&c.id)
                })
                .count();
            if held + citizens.len() > b.slots_for(job) {
                return Err(RuleError::Full);
            }
        }

        for id in citizens {
            self.unassign_one(*id);
            let c = &mut self.citizens[id.0 as usize];
            c.job = Some(job);
            // A hauler is based nowhere: it goes where the work is.
            c.workplace = if job == Job::Hauler { None } else { Some(building) };
        }
        // One trader is one mule. Spawned here rather than in the tick,
        // because "assigning one spawns a mule" is a rule about the command
        // and a rule about a command belongs inside `apply`.
        if job == Job::Trader {
            for _ in 0..citizens.len() {
                self.spawn_mule(player, building);
            }
        }
        Ok(())
    }

    /// Send a cart out from a post.
    fn spawn_mule(&mut self, owner: PlayerId, post: BuildingId) {
        let (cx, cy) = self.buildings[post.0 as usize].centre();
        // In the yard, not in the building: a post blocks movement like any
        // other, and a cart standing inside one has no flow field to follow.
        let at = (0..=3i32)
            .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))))
            .map(|(dx, dy)| (cx + dx, cy + dy))
            .find(|&(x, y)| nav::passable(self, x, y))
            .unwrap_or((cx, cy));
        let id = MuleId(self.mules.len() as u16);
        self.mules.push(Mule::new(id, owner, post, V2::cell_centre(at.0, at.1)));
    }

    /// Retire every mule belonging to a post that has more of them than it has
    /// traders — because a trader was unassigned, or because the post is gone.
    fn retire_spare_mules(&mut self, post: BuildingId) {
        let traders = self
            .citizens
            .iter()
            .filter(|c| c.alive() && c.workplace == Some(post) && c.job == Some(Job::Trader))
            .count();
        let standing = self
            .buildings
            .get(post.0 as usize)
            .is_some_and(|b| b.standing_now() && b.kind == Kind::TradingPost);
        let want = if standing { traders } else { 0 };

        let mine: Vec<usize> = self
            .mules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive() && m.post == post)
            .map(|(i, _)| i)
            .collect();
        // The newest go first, so a post that loses one trader keeps the mule
        // that has been on the road longest and its load with it.
        for &i in mine.iter().skip(want).rev() {
            self.mules[i].retired = true;
        }
    }

    /// Back to hauling, and off whatever roster it was on.
    fn unassign_one(&mut self, id: CitizenId) {
        let was = self.citizens[id.0 as usize]
            .workplace
            .filter(|_| self.citizens[id.0 as usize].job == Some(Job::Trader));
        self.clear_from_rosters(id);
        if let Some(post) = was {
            // Done after `clear_from_rosters` and before the job is cleared
            // would be a mule counted twice; done here it is counted once.
            self.citizens[id.0 as usize].job = None;
            self.citizens[id.0 as usize].workplace = None;
            self.retire_spare_mules(post);
        }
        let c = &mut self.citizens[id.0 as usize];
        c.job = None;
        c.workplace = None;
        // Back to hauling means back to hauling: a citizen that was told to
        // stand on a hill starts finding its own work again. `MoveTo` calls
        // this and then sets the flag, so the order of those two matters.
        c.held = false;
        c.abandon();
    }

    /// A tick with no commands and a throwaway cache. For tests, and for
    /// anywhere that only wants the world to advance.
    pub fn tick_alone(&mut self) {
        let mut nav = Nav::new();
        self.tick(&mut nav, &[]);
    }

    // ---- families ----------------------------------------------------------

    /// One tick of who lives with whom, and what comes of it.
    ///
    /// Three small passes in a fixed order: children grow up, households form,
    /// households have children. Order matters only in that a child coming of
    /// age this tick is an adult for the rest of it, which is the answer a
    /// player would expect.
    fn families(&mut self) {
        self.grow_up();
        self.form_households();
        self.have_children();
    }

    /// Children become ordinary citizens on the tick they were born knowing.
    fn grow_up(&mut self) {
        let now = self.tick;
        for i in 0..self.citizens.len() {
            if self.citizens[i].alive() {
                self.citizens[i].come_of_age(now);
            }
        }
    }

    /// Two adults who share a cottage become a household, after a day of it.
    ///
    /// **In id order, and only the first two.** A cottage with five beds and
    /// five people in it is not two and a half families; it is one household
    /// and three lodgers, and which two is decided by the same rule on every
    /// machine. Design §3.2 says "two adult citizens sharing a cottage", and
    /// the pairing has to be a function of the world or two peers would
    /// disagree about who married whom.
    fn form_households(&mut self) {
        let cottages: Vec<BuildingId> = self
            .buildings
            .iter()
            .filter(|b| b.kind == Kind::Cottage && b.standing_now())
            .map(|b| b.id)
            .collect();

        for cottage in cottages {
            let mut residents: Vec<CitizenId> = self
                .citizens
                .iter()
                .filter(|c| c.alive() && !c.is_child() && c.home == Some(cottage))
                .map(|c| c.id)
                .collect();
            residents.sort_unstable();
            let Some(&[a, b]) = residents.get(..2).map(|s| <&[CitizenId; 2]>::try_from(s).ok()).flatten()
            else {
                continue;
            };

            let owner = self.citizens[a.0 as usize].owner;
            let fed = self.citizens[a.0 as usize].food >= CHILD_FOOD
                && self.citizens[b.0 as usize].food >= CHILD_FOOD;

            match self.households.iter_mut().position(|h| {
                h.alive() && h.cottage == cottage && h.members == [a, b]
            }) {
                Some(i) => {
                    // A hungry day loses the progress rather than pausing it:
                    // being fed is the gate, and a gate that only slows you
                    // down is not a gate.
                    if fed {
                        self.households[i].together += 1;
                    } else {
                        self.households[i].together = 0;
                    }
                }
                None => {
                    let id = HouseholdId(self.households.len() as u16);
                    self.households.push(Household::new(id, owner, [a, b], cottage));
                }
            }
        }

        // A household whose cottage is gone, or one of whose members has died,
        // is over. Its children keep growing up; the flood does that.
        for i in 0..self.households.len() {
            if !self.households[i].alive() {
                continue;
            }
            let h = &self.households[i];
            let standing = self
                .buildings
                .get(h.cottage.0 as usize)
                .is_some_and(|b| b.standing_now());
            let both = h.members.iter().all(|m| {
                self.citizens
                    .get(m.0 as usize)
                    .is_some_and(|c| c.alive() && c.home == Some(h.cottage))
            });
            if !standing || !both {
                self.households[i].ended = true;
            }
        }
    }

    /// A fed household with somewhere to put a child has one, on a timer.
    fn have_children(&mut self) {
        for i in 0..self.households.len() {
            if !self.households[i].alive() || !self.households[i].settled() {
                continue;
            }
            let owner = self.households[i].owner;
            let [a, b] = self.households[i].members;
            let fed = self.citizens[a.0 as usize].food >= CHILD_FOOD
                && self.citizens[b.0 as usize].food >= CHILD_FOOD
                && (self.treasury(owner).food > 0 || self.treasury(owner).meal > 0);
            let spare_bed = {
                let cottage = self.households[i].cottage;
                let taken = self
                    .citizens
                    .iter()
                    .filter(|c| c.alive() && c.home == Some(cottage))
                    .count();
                taken < self.buildings[cottage.0 as usize].beds()
            };
            if !fed || !spare_bed {
                self.households[i].toward_child = 0;
                continue;
            }
            self.households[i].toward_child += 1;
            if self.households[i].toward_child < CHILD_TICKS {
                continue;
            }
            self.households[i].toward_child = 0;
            if let Some(child) = self.bear_a_child(i) {
                self.households[i].children.push(child);
            }
        }
    }

    /// Somewhere for a child to be kept: a nursery of this player's with a
    /// place free. **No nursery, no children.**
    fn free_nursery(&self, owner: PlayerId) -> Option<BuildingId> {
        self.buildings
            .iter()
            .filter(|b| b.owner == owner && b.kind == Kind::Nursery && b.standing_now())
            .find(|b| {
                let held = self
                    .citizens
                    .iter()
                    .filter(|c| c.alive() && c.nursery == Some(b.id))
                    .count();
                held < b.places()
            })
            .map(|b| b.id)
    }

    /// Add a citizen to the world. The only place that ever does.
    ///
    /// **Appending only.** Ids are indices into `citizens` in half a dozen
    /// places, and the crowd, the flood and every roster iterate it: appending
    /// is safe and anything that reorders or reuses an id is not. There is no
    /// removal anywhere — the dead stay in the vector.
    fn bear_a_child(&mut self, household: usize) -> Option<CitizenId> {
        let owner = self.households[household].owner;
        let nursery = self.free_nursery(owner)?;
        let cottage = self.households[household].cottage;
        let (nx, ny) = self.buildings[nursery.0 as usize].centre();

        let name = self.rng.below(NAMES.len() as u32) as u16;
        let jitter = V2::new(Fx(self.rng.range(-48, 48)), Fx(self.rng.range(-48, 48)));
        let id = CitizenId(self.citizens.len() as u16);
        let mut child = Citizen::born(
            id,
            owner,
            name,
            V2::cell_centre(nx, ny) + jitter,
            self.tick,
            nursery,
        );
        // It sleeps where its parents do. The spare bed was checked before
        // this was called.
        child.home = Some(cottage);
        self.citizens.push(child);
        Some(id)
    }

    // ---- mules -------------------------------------------------------------

    /// One tick of the carts on the road.
    ///
    /// Deliberately small, and deliberately not `walk`: a mule has no hunger,
    /// no rest, no home, no crowd and no errand it can abandon, so sharing the
    /// citizen path would have meant six rules that do not apply to it. What it
    /// does share is the flow fields, which is the half that matters.
    pub fn move_mules(&mut self, nav: &mut Nav) {
        if self.mules.is_empty() {
            return;
        }
        // A post taken by the flood retires its carts.
        let posts: Vec<BuildingId> = {
            let mut p: Vec<BuildingId> =
                self.mules.iter().filter(|m| m.alive()).map(|m| m.post).collect();
            p.sort_unstable();
            p.dedup();
            p
        };
        for post in posts {
            self.retire_spare_mules(post);
        }

        for i in 0..self.mules.len() {
            if self.mules[i].alive() {
                self.set_mule_errand(i);
            }
        }

        let mut wanted: Vec<Dest> = self
            .mules
            .iter()
            .filter(|m| m.alive())
            .filter_map(|m| m.dest)
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        for dest in wanted {
            let field = nav.field(self, dest).clone();
            for i in 0..self.mules.len() {
                if self.mules[i].dest == Some(dest) && self.mules[i].alive() {
                    self.step_mule(i, &field);
                }
            }
        }
    }

    /// Where a mule is going, and what it is carrying while it goes there.
    fn set_mule_errand(&mut self, i: usize) {
        if self.mules[i].dest.is_some() {
            return;
        }
        let (owner, post) = (self.mules[i].owner, self.mules[i].post);
        match self.mules[i].leg {
            Leg::Home => {
                self.mules[i].dest = Some(Dest::Building(post));
            }
            Leg::Out | Leg::Stuck => {
                if !self.mules[i].carrying_any() {
                    // Load, out of the city's own stores. A post with nothing
                    // to sell waits, which is a thing a player can see and act
                    // on: build a forester's hut.
                    let want = Mule::load();
                    for g in Good::ALL {
                        let n = want.get(g);
                        if n == 0 {
                            continue;
                        }
                        if self.take_from_stores(owner, g, n) < n {
                            return;
                        }
                        self.mules[i].carrying.add(g, n);
                    }
                }
                match self.trade_partner(owner) {
                    Some(hearth) => {
                        self.mules[i].leg = Leg::Out;
                        self.mules[i].dest = Some(Dest::Building(hearth));
                    }
                    // Loaded and nowhere to take it. The panel says so; see
                    // `Leg::Stuck`.
                    None => self.mules[i].leg = Leg::Stuck,
                }
            }
        }
    }

    /// The hearth a mule sells at: another city's, and the nearest of them.
    pub fn trade_partner(&self, mine: PlayerId) -> Option<BuildingId> {
        let home = self
            .buildings
            .iter()
            .find(|b| b.owner == mine && b.kind == Kind::Hearth)
            .map(|b| b.centre())?;
        self.buildings
            .iter()
            .filter(|b| {
                b.owner != mine
                    && b.kind == Kind::Hearth
                    && b.standing_now()
                    && self.players.contains(&b.owner)
                    && !self.dropped.contains(&b.owner)
            })
            .min_by_key(|b| {
                let (x, y) = b.centre();
                (x - home.0).abs() + (y - home.1).abs()
            })
            .map(|b| b.id)
    }

    /// Take up to `amount` of `good` out of a player's stores, nearest first.
    fn take_from_stores(&mut self, owner: PlayerId, good: Good, amount: u16) -> u16 {
        let mut left = amount;
        for i in 0..self.buildings.len() {
            if left == 0 {
                break;
            }
            let b = &self.buildings[i];
            if b.owner != owner || !b.standing_now() || !b.kind.is_store() {
                continue;
            }
            left -= self.buildings[i].store.take(good, left);
        }
        amount - left
    }

    /// One mule, one step.
    fn step_mule(&mut self, i: usize, field: &FlowField) {
        let (cx, cy) = self.mules[i].pos.cell();
        let Some(dest) = self.mules[i].dest else { return };

        // Near enough. A cart does not have to stand on the doorstep.
        let there = match dest {
            Dest::Building(id) => {
                let b = &self.buildings[id.0 as usize];
                let (bx, by) = b.centre();
                (cx - bx).abs().max((cy - by).abs()) <= MULE_ARRIVED
            }
            Dest::Cell(x, y) => {
                (cx - x as i32).abs().max((cy - y as i32).abs()) <= MULE_ARRIVED
            }
        };
        if there {
            self.mule_arrived(i);
            return;
        }

        let step = field.step_at(cx, cy).or_else(|| self.step_off_a_building(cx, cy));
        let Some((dx, dy)) = step else {
            // No way through from where it is standing. It waits rather than
            // wanders; `Leg::Stuck` is for having nowhere to go at all, and
            // this is a road that is not there yet.
            return;
        };
        // A mule is spawned at its post and a post blocks movement, so its
        // first step out is the same one a citizen born inside a hearth takes.
        // Without it the cart stood in the yard for ever and the whole feature
        // did nothing, silently.
        let on_road =
            self.building_at(cx, cy).map(|b| b.carries_traffic()).unwrap_or(false);
        let speed = if on_road { Fx(MULE_SPEED * 2) } else { Fx(MULE_SPEED) };
        let step = V2::new(Fx::cells(dx), Fx::cells(dy)).with_len(speed);
        self.mules[i].pos += step;
    }

    /// A mule at the end of a leg: hand over, get paid, turn round.
    fn mule_arrived(&mut self, i: usize) {
        let owner = self.mules[i].owner;
        match self.mules[i].leg {
            Leg::Out | Leg::Stuck => {
                let Some(Dest::Building(hearth)) = self.mules[i].dest else { return };
                let them = self.buildings[hearth.0 as usize].owner;
                for g in Good::ALL {
                    let n = self.mules[i].carrying.take(g, u16::MAX);
                    if n == 0 {
                        continue;
                    }
                    let put = self.buildings[hearth.0 as usize].stow(g, n);
                    // What will not fit stays on the cart and goes home again.
                    self.mules[i].carrying.add(g, n - put);
                }
                // **Gold is minted by the exchange, not moved.** The other
                // city does not pay out of a purse it has not got: nothing
                // else in the game makes gold, so if trade only moved it the
                // first trade could never happen. See DECISIONS.md.
                let _ = them;
                self.mules[i].carrying = Mule::pay();
                self.mules[i].leg = Leg::Home;
                self.mules[i].dest = None;
            }
            Leg::Home => {
                for g in Good::ALL {
                    let n = self.mules[i].carrying.take(g, u16::MAX);
                    if n == 0 {
                        continue;
                    }
                    let put = self.put_in_stores(owner, g, n);
                    // Nowhere to put it is not a reason to destroy it.
                    self.mules[i].carrying.add(g, n - put);
                }
                if !self.mules[i].carrying_any() {
                    self.mules[i].leg = Leg::Out;
                }
                self.mules[i].dest = None;
            }
        }
    }

    /// Put up to `amount` of `good` into a player's stores. Returns what fitted.
    fn put_in_stores(&mut self, owner: PlayerId, good: Good, amount: u16) -> u16 {
        let mut left = amount;
        for i in 0..self.buildings.len() {
            if left == 0 {
                break;
            }
            let b = &self.buildings[i];
            if b.owner != owner || !b.standing_now() || !b.kind.stores(good) {
                continue;
            }
            left -= self.buildings[i].stow(good, left);
        }
        amount - left
    }

    /// Move everyone who is going somewhere, one step along their field.
    fn walk(&mut self, nav: &mut Nav) {
        // Gather the destinations in use first, so the borrow of `self` for
        // the field lookup does not overlap the mutation of the citizens. The
        // list is deduplicated, which is the entire point of a shared field:
        // eight people walking to the granary consult one.
        let mut wanted: Vec<Dest> = self
            .citizens
            .iter()
            .filter(|c| c.alive() && c.state == State::Walking)
            .filter_map(|c| c.dest)
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        if wanted.is_empty() {
            return;
        }

        for dest in wanted {
            // One field, then everyone using it, in citizen-id order.
            let field = nav.field(self, dest).clone();
            for i in 0..self.citizens.len() {
                if self.citizens[i].dest != Some(dest)
                    || !self.citizens[i].alive()
                    || self.citizens[i].state != State::Walking
                {
                    continue;
                }
                self.step_citizen(i, &field);
            }
        }
    }

    /// One citizen, one step.
    fn step_citizen(&mut self, i: usize, field: &FlowField) {
        let (cx, cy) = self.citizens[i].pos.cell();

        if self.arrived(i, cx, cy) {
            self.citizens[i].halt();
            return;
        }

        let Some((dx, dy)) = field.step_at(cx, cy) else {
            if let Some(out) = self.step_off_a_building(cx, cy) {
                // Walking out of a footprint it is standing inside. Not
                // through the check below, which would refuse the first step:
                // the cell next to the middle of a three-by-three Hearth is
                // also the Hearth.
                self.nudge(i, out);
                return;
            }
            // Nowhere to go from here: the granary washed away, or a building
            // went up across the only path, or this citizen is standing
            // somewhere the field never reached. Stopping is the honest
            // answer — the citizen is not stuck in a wall, it is standing
            // still, and whatever wanted it to move can ask again.
            self.citizens[i].halt();
            return;
        };

        // A goal cell can be somewhere nobody can stand: `MoveTo` a boulder is
        // a legal order, and the field is seeded there whether it is passable
        // or not. Getting as close as possible and stopping is what the player
        // meant; walking onto the rock is not.
        //
        // The exception is the building you are walking to, which you may walk
        // into — the same rule the field is built with.
        let into = match self.citizens[i].dest {
            Some(Dest::Building(id)) => Some(id),
            _ => None,
        };
        if !nav::passable_into(self, cx + dx, cy + dy, into) {
            self.citizens[i].halt();
            return;
        }

        self.nudge(i, (dx, dy));
    }

    /// One citizen, one step in a direction already decided.
    fn nudge(&mut self, i: usize, (dx, dy): (i32, i32)) {
        let (cx, cy) = self.citizens[i].pos.cell();
        let on_road = self.building_at(cx, cy).map(|b| b.carries_traffic()).unwrap_or(false);
        let wading = self.map.ground_at(cx, cy) == Ground::Ford;
        let speed = self.citizens[i].speed(on_road, wading);

        // The step is one of the eight neighbours, so its length is 1 or
        // sqrt(2); `with_len` scales it to the distance walked this tick and
        // keeps a diagonal from being 41% faster than a straight line.
        let dir = V2::new(Fx::cells(dx), Fx::cells(dy)).with_len(speed);
        self.citizens[i].vel = dir;
        self.citizens[i].pos += dir;
    }

    /// A way out of an impassable cell, for somebody standing in one.
    ///
    /// A flow field is built over passable ground, so it has no step for a
    /// citizen standing inside a building — and every citizen starts inside
    /// one, because a Hearth blocks movement and the founding party spawns on
    /// its site. Ordering everybody uphill on the first day therefore left
    /// whoever had not wandered off yet standing in the fire, unable to be
    /// sent anywhere for the rest of the game. Anyone who has stepped into a
    /// granary to eat is in the same position.
    ///
    /// Rings outward in a fixed order, which is the only order there is. Not
    /// just the four neighbours: a citizen in the middle of a three-by-three
    /// Hearth has four neighbours that are also the Hearth.
    fn step_off_a_building(&self, cx: i32, cy: i32) -> Option<(i32, i32)> {
        if nav::passable(self, cx, cy) {
            return None;
        }
        for r in 1..=3i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if nav::passable(self, cx + dx, cy + dy) {
                        // One step of the eight, toward it.
                        return Some((dx.signum(), dy.signum()));
                    }
                }
            }
        }
        None
    }

    /// Whether citizen `i` has reached what it was walking to.
    ///
    /// Arriving is still *at* the building and not *in* it, and that is
    /// deliberate. M8 was written the other way first — a worker arrives only
    /// once it is on the footprint — and it starved two cities: a farmer that
    /// has not "arrived" is still `Walking`, and a city of people permanently
    /// on their way somewhere eats and sleeps at the wrong times. What puts a
    /// worker inside its farm is the crowd, which may now push somebody into
    /// the building they belong in and cannot push anybody else there.
    fn arrived(&self, i: usize, cx: i32, cy: i32) -> bool {
        match self.citizens[i].dest {
            None => true,
            Some(Dest::Cell(gx, gy)) => cx == gx as i32 && cy == gy as i32,
            Some(Dest::Building(id)) => match self.buildings.get(id.0 as usize) {
                // Near enough to hand something over or to work in it.
                Some(b) if b.state != BuildState::Rubble => nav::at_building(b, cx, cy),
                // It is not there any more. Arriving is the kindest reading:
                // the citizen stops where it is instead of walking to a hole.
                _ => true,
            },
        }
    }
}

/// Cells around a hearth that somebody can stand on, nearest first.
///
/// Rings outward in a fixed order, skipping the hearth's own footprint and any
/// ground nobody can walk on. Deterministic by construction: it depends on the
/// map and nothing else.
fn spawn_ring(map: &Map, hx: i32, hy: i32) -> impl Iterator<Item = (i32, i32)> + '_ {
    let half = HEARTH_SIZE / 2;
    (2..8i32).flat_map(move |r| {
        (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))).filter_map(
            move |(dx, dy)| {
                if dx.abs() != r && dy.abs() != r {
                    return None;
                }
                if dx.abs() <= half && dy.abs() <= half {
                    return None;
                }
                let (x, y) = (hx + dx, hy + dy);
                if !Map::contains(x, y) || !map.buildable(x, y) {
                    return None;
                }
                Some((x, y))
            },
        )
    })
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
    use crate::map::{Ground, MAP_H, MAP_W};

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
        // Design §8 budgets 50–150 KB for a late joiner's `Welcome` at 500
        // citizens. A fresh world is the empty end of that and is almost
        // entirely the two 16 k-cell grids — heights, ground, and the mostly
        // empty occupancy index, which postcard writes as one byte per free
        // cell. Worth keeping an eye on: it is the floor, and every building
        // and citizen adds to it.
        let w = World::new(1, 6);
        let bytes = postcard::to_allocvec(&w).unwrap().len();
        assert!(
            bytes < 150_000,
            "a fresh six-player world encodes to {bytes} bytes, over the §8 budget"
        );
        // And it really is dominated by the grids rather than by anything
        // that grows with the game.
        assert!(bytes > crate::map::CELLS * 2, "suspiciously small: {bytes} bytes");
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
            w.tick_alone();
        }
        assert_eq!(w.tick, 100);
        assert_eq!(w.citizens[0].food, start - FOOD_DECAY * 100);
        assert!(w.citizens[0].alive());

        // Empty by tick 250, then three days of starving.
        let empty_at = (NEED_FULL / FOOD_DECAY) as u32;
        while w.tick < empty_at {
            w.tick_alone();
        }
        assert_eq!(w.citizens[0].food, 0);
        assert!(w.citizens[0].starving());
        assert_eq!(w.citizens[0].state, State::Idle, "starving is a condition, not an activity");

        // Death lands exactly STARVE_TICKS ticks after the food ran out —
        // stated against `starved_for` rather than against a tick arithmetic
        // expression, because the first version of this test got that
        // arithmetic wrong by one and blamed the code.
        while w.citizens[0].starved_for < STARVE_TICKS - 1 {
            w.tick_alone();
        }
        assert!(w.citizens[0].alive(), "alive with one tick of the three days left");
        w.tick_alone();
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
            w.tick_alone();
        }
        assert!(w.citizens[0].starving());
        assert!(w.citizens[0].starved_for > 0);

        w.citizens[0].eat(NEED_FULL);
        w.tick_alone();
        assert!(!w.citizens[0].starving());
        assert_eq!(w.citizens[0].starved_for, 0);

        // And the clock starts from the beginning next time, rather than
        // resuming where it left off.
        while w.citizens[0].food > 0 {
            w.tick_alone();
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
        for t in 0..1000 {
            c.tick_needs(t);
        }
        assert_eq!(c, before, "a corpse does not get hungrier");
    }

    // ---- buildings ---------------------------------------------------------

    /// A cell of buildable ground with nothing on it, owned by nobody,
    /// searched outward from a player's hearth. For tests that need somewhere
    /// legal to build without caring where.
    fn free_spot(w: &World, p: usize, kind: Kind) -> (i32, i32) {
        let (hx, hy) = w.map.hearth_sites[p];
        for r in 3..60i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (hx + dx, hy + dy);
                    if w.can_place(PlayerId(p as u8), kind, Facing::EastWest, x, y).is_ok() {
                        return (x, y);
                    }
                }
            }
        }
        panic!("no legal spot for {kind:?} near hearth {p}");
    }

    #[test]
    fn a_run_begins_with_one_hearth_per_player_holding_its_stores() {
        for players in 2..=6u32 {
            let w = World::new(4, players);
            assert_eq!(w.buildings.len(), players as usize);
            for (p, b) in w.buildings.iter().enumerate() {
                assert_eq!(b.kind, Kind::Hearth);
                assert_eq!(b.owner, PlayerId(p as u8));
                assert!(b.standing_now(), "a hearth is not a building site");
                assert_eq!(b.store, Goods::of(0, STARTING_WOOD, STARTING_STONE, 0));

                // Centred on the site the map generator levelled for it.
                let (cx, cy) = w.map.hearth_sites[p];
                assert_eq!(b.centre(), (cx, cy));
                for (x, y) in b.cells() {
                    assert_eq!(
                        w.occupancy[Map::idx(x, y)],
                        Some(b.id),
                        "the hearth's own cell is not marked as its"
                    );
                }
            }
        }
    }

    #[test]
    fn the_occupancy_grid_agrees_with_the_building_list() {
        let mut w = World::new(5, 3);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        let id = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();

        let mut counted = 0;
        for (i, slot) in w.occupancy.iter().enumerate() {
            if let Some(bid) = slot {
                let b = &w.buildings[bid.0 as usize];
                assert!(
                    b.cells().any(|(cx, cy)| Map::idx(cx, cy) == i),
                    "cell {i} claims {bid:?}, which does not cover it"
                );
                counted += 1;
            }
        }
        let expected: usize = w.buildings.iter().map(|b| b.cells().count()).sum();
        assert_eq!(counted, expected, "every occupied cell is accounted for");
        assert_eq!(w.building_at(x, y).map(|b| b.id), Some(id));
    }

    #[test]
    fn placing_refuses_a_footprint_that_hangs_off_the_map() {
        let mut w = World::new(1, 2);
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Farm, Facing::EastWest, MAP_W - 1, 5),
            Err(RuleError::OffMap)
        );
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, Facing::EastWest, -1, 5), Err(RuleError::OffMap));
        assert!(w.place(PlayerId(0), Kind::Farm, Facing::EastWest, MAP_W - 1, 5).is_err());
        assert_eq!(w.buildings.len(), 2, "a rejected placement builds nothing");
    }

    #[test]
    fn placing_refuses_an_occupied_footprint() {
        let mut w = World::new(6, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();

        // Exactly on top.
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y), Err(RuleError::Occupied));
        // And merely overlapping by one cell, in each direction.
        for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1)] {
            assert_eq!(
                w.can_place(PlayerId(0), Kind::Cottage, Facing::EastWest, x + dx, y + dy),
                Err(RuleError::Occupied),
                "overlap at ({dx},{dy}) was allowed"
            );
        }
        // Another player's building is in the way too — the map is shared.
        assert_eq!(w.can_place(PlayerId(1), Kind::Cottage, Facing::EastWest, x, y), Err(RuleError::Occupied));
    }

    #[test]
    fn placing_refuses_the_wrong_ground() {
        let mut w = World::new(2, 2);
        let rock = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Rock)
            .expect("every map has rock");
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Dike, Facing::EastWest, rock.0, rock.1),
            Err(RuleError::WrongGround)
        );

        let wet = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Shallows)
            .expect("every map has shallows");
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Cottage, Facing::EastWest, wet.0, wet.1),
            Err(RuleError::WrongGround),
            "a cottage in the river"
        );
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Bridge, Facing::EastWest, wet.0, wet.1),
            Ok(()),
            "but a bridge belongs there"
        );

        let dry = free_spot(&w, 0, Kind::Dike);
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Bridge, Facing::EastWest, dry.0, dry.1),
            Err(RuleError::WrongGround),
            "and nowhere else"
        );
        let _ = w.place(PlayerId(0), Kind::Bridge, Facing::EastWest, wet.0, wet.1).unwrap();
    }

    #[test]
    fn nobody_gets_a_second_hearth_and_nobody_commands_a_city_that_is_not_theirs() {
        let w = World::new(3, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Hearth, Facing::EastWest, x, y),
            Err(RuleError::OneHearthOnly)
        );
        assert_eq!(
            w.can_place(PlayerId(9), Kind::Cottage, Facing::EastWest, x, y),
            Err(RuleError::NotYours),
            "a player who is not in this run"
        );
    }

    #[test]
    fn a_building_goes_up_by_hauling_then_building() {
        let mut w = World::new(8, 2);
        let (x, y) = free_spot(&w, 0, Kind::Granary);
        let id = w.place(PlayerId(0), Kind::Granary, Facing::EastWest, x, y).unwrap();

        // Take the wood out of the hearth, as a hauler would.
        let hearth = w.buildings[0].id;
        let carried = w.buildings[hearth.0 as usize].store.take(Good::Wood, 50);
        assert_eq!(carried, 50);
        assert_eq!(w.buildings[hearth.0 as usize].store.wood, STARTING_WOOD - 50);

        assert_eq!(w.deliver_to(id, Good::Wood, carried), 50);
        assert!(w.buildings[id.0 as usize].ready_to_build());
        for _ in 0..Kind::Granary.build_ticks() {
            w.build_at(id, BUILDER_EFFORT);
        }
        assert!(w.buildings[id.0 as usize].standing_now());
    }

    #[test]
    fn demolishing_frees_the_ground_and_returns_something() {
        let mut w = World::new(12, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        let id = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();
        w.deliver_to(id, Good::Wood, 30);
        w.build_at(id, Kind::Cottage.build_ticks());

        assert_eq!(w.demolish(PlayerId(1), id), Err(RuleError::NotYours));
        let salvage = w.demolish(PlayerId(0), id).unwrap();
        assert_eq!(salvage, Goods::wood(15), "half the wood back");
        assert_eq!(w.buildings[id.0 as usize].state, BuildState::Rubble);
        assert!(w.building_at(x, y).is_none(), "the ground is free again");
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y), Ok(()));
        assert_eq!(w.demolish(PlayerId(0), BuildingId(999)), Err(RuleError::NoSuchBuilding));
    }

    #[test]
    fn rubble_keeps_its_id() {
        // The reason buildings are never removed from the vector: an id is an
        // index for the whole run, so nothing has to be remapped when a flood
        // takes half a city.
        let mut w = World::new(13, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        let first = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();
        w.demolish(PlayerId(0), first).unwrap();
        let second = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, x, y).unwrap();
        assert_ne!(first, second, "the id is not reused");
        assert_eq!(w.buildings[first.0 as usize].state, BuildState::Rubble);
        for (i, b) in w.buildings.iter().enumerate() {
            assert_eq!(b.id, BuildingId(i as u16), "ids are still indices");
        }
    }

    #[test]
    fn a_dike_raises_the_ground_only_once_it_stands() {
        let mut w = World::new(15, 2);
        let (x, y) = free_spot(&w, 0, Kind::Dike);
        let base = w.map.height_at(x, y) as u16;
        let id = w.place(PlayerId(0), Kind::Dike, Facing::EastWest, x, y).unwrap();
        assert_eq!(w.effective_height(x, y), base, "a site holds nothing back");

        // The price is a balance constant and has moved once already; what is
        // under test is when a dike starts holding water back, not what a
        // level costs.
        let per = Kind::Dike.cost().stone;
        w.deliver_to(id, Good::Stone, per);
        w.build_at(id, Kind::Dike.build_ticks());
        assert_eq!(w.effective_height(x, y), base + DIKE_HEIGHT_PER_LEVEL);

        // Raising it puts it back under construction until the stone arrives.
        w.raise_dike(PlayerId(0), id).unwrap();
        assert_eq!(w.effective_height(x, y), base, "and it is a site again while it grows");
        assert_eq!(w.buildings[id.0 as usize].outstanding(), Goods::stone(per));
        w.deliver_to(id, Good::Stone, per);
        w.build_at(id, Kind::Dike.build_ticks());
        assert_eq!(w.effective_height(x, y), base + DIKE_HEIGHT_PER_LEVEL * 2);

        // A level-2 dike stops an age-1 surge of height 12 (design §5), and
        // this is the arithmetic that has to hold for that to be true.
        assert!(DIKE_HEIGHT_PER_LEVEL * 2 * 2 >= 12);
    }

    #[test]
    fn a_dike_cannot_grow_forever() {
        let mut w = World::new(16, 2);
        let (x, y) = free_spot(&w, 0, Kind::Dike);
        let id = w.place(PlayerId(0), Kind::Dike, Facing::EastWest, x, y).unwrap();
        w.deliver_to(id, Good::Stone, 40);
        w.build_at(id, Kind::Dike.build_ticks());

        for _ in 1..DIKE_MAX_LEVEL {
            w.raise_dike(PlayerId(0), id).unwrap();
            let cost = w.buildings[id.0 as usize].outstanding();
            w.deliver_to(id, Good::Stone, cost.stone);
            w.build_at(id, Kind::Dike.build_ticks());
        }
        assert_eq!(w.buildings[id.0 as usize].level, DIKE_MAX_LEVEL);
        assert_eq!(w.raise_dike(PlayerId(0), id), Err(RuleError::TooHigh));
        assert_eq!(w.raise_dike(PlayerId(1), id), Err(RuleError::NotYours));

        // Only a standing dike can be raised, and only a dike.
        let (cx, cy) = free_spot(&w, 0, Kind::Cottage);
        let cottage = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, cx, cy).unwrap();
        assert_eq!(w.raise_dike(PlayerId(0), cottage), Err(RuleError::NotStanding));
    }

    #[test]
    fn stores_are_listed_nearest_first() {
        let mut w = World::new(21, 2);
        let hearth = &w.buildings[0];
        let (hx, hy) = hearth.centre();

        let mut ids = Vec::new();
        for target in [6, 14] {
            let mut placed = None;
            'search: for r in target..target + 12i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        let (x, y) = (hx + dx, hy + dy);
                        if w.can_place(PlayerId(0), Kind::Stockpile, Facing::EastWest, x, y).is_ok() {
                            placed = Some(w.place(PlayerId(0), Kind::Stockpile, Facing::EastWest, x, y).unwrap());
                            break 'search;
                        }
                    }
                }
            }
            let id = placed.expect("somewhere to put a stockpile");
            w.build_at(id, Kind::Stockpile.build_ticks());
            ids.push(id);
        }

        // The hearth stores wood too, and it is the closest thing to itself.
        let near = w.stores_for(PlayerId(0), Good::Wood, hx, hy);
        assert_eq!(near.first(), Some(&BuildingId(0)));
        assert_eq!(near.len(), 3);

        // Distances are non-decreasing down the list — the property a hauler
        // relies on.
        let dist = |id: BuildingId, x: i32, y: i32| {
            let (bx, by) = w.buildings[id.0 as usize].centre();
            (bx - x).abs() + (by - y).abs()
        };
        for pair in near.windows(2) {
            assert!(dist(pair[0], hx, hy) <= dist(pair[1], hx, hy));
        }

        // Both new stockpiles are on the list, and the other player's hearth
        // is not — a store belongs to a city, not to the map.
        assert!(near.contains(&ids[0]) && near.contains(&ids[1]));
        assert_eq!(
            w.stores_for(PlayerId(1), Good::Wood, hx, hy),
            vec![BuildingId(1)],
            "player 1 has only their own hearth"
        );
        // And a granary holds food, not wood.
        assert!(!Kind::Granary.stores(Good::Wood));
    }

    #[test]
    fn a_site_is_not_a_store() {
        let mut w = World::new(22, 2);
        let (x, y) = free_spot(&w, 0, Kind::Stockpile);
        let id = w.place(PlayerId(0), Kind::Stockpile, Facing::EastWest, x, y).unwrap();
        assert!(
            !w.stores_for(PlayerId(0), Good::Wood, x, y).contains(&id),
            "an unfinished stockpile is a hole in the ground"
        );
        w.build_at(id, Kind::Stockpile.build_ticks());
        assert!(w.stores_for(PlayerId(0), Good::Wood, x, y).contains(&id));
    }

    // ---- walking -----------------------------------------------------------

    /// A world, a clear patch of ground, and one citizen standing in it.
    fn walker() -> (World, Nav, i32, i32) {
        let mut w = World::new(31, 2);
        let (mut ox, mut oy) = (0, 0);
        'find: for y in 8..MAP_H - 8 {
            for x in 8..MAP_W - 8 {
                if (-7..=7).all(|dy| (-7..=7).all(|dx| nav::passable(&w, x + dx, y + dy))) {
                    ox = x;
                    oy = y;
                    break 'find;
                }
            }
        }
        assert!(ox > 0, "no open ground to walk on");
        w.citizens[0].pos = V2::cell_centre(ox, oy);
        (w, Nav::new(), ox, oy)
    }

    #[test]
    fn a_citizen_walks_to_a_cell_and_stops_there() {
        let (mut w, mut nav, ox, oy) = walker();
        let goal = (ox + 5, oy);
        w.citizens[0].walk_to(Dest::Cell(goal.0 as u8, goal.1 as u8));
        assert_eq!(w.citizens[0].state, State::Walking);

        for _ in 0..400 {
            w.tick(&mut nav, &[]);
            if w.citizens[0].state != State::Walking {
                break;
            }
        }
        assert_eq!(w.citizens[0].pos.cell(), goal, "did not arrive");
        assert_eq!(w.citizens[0].state, State::Idle, "did not stop");
        assert_eq!(w.citizens[0].dest, None);
        assert_eq!(w.citizens[0].vel, V2::ZERO, "still drifting after arriving");
    }

    #[test]
    fn walking_takes_the_time_the_speed_says_it_should() {
        let (mut w, mut nav, ox, oy) = walker();
        w.citizens[0].walk_to(Dest::Cell((ox + 8) as u8, oy as u8));

        let mut ticks = 0;
        while w.citizens[0].state == State::Walking && ticks < 1000 {
            w.tick(&mut nav, &[]);
            ticks += 1;
        }
        // Eight cells at WALK_SPEED 256ths of a cell per tick, give or take
        // the tick it notices it has arrived.
        let want = 8 * 256 / WALK_SPEED;
        assert!(
            (want - 2..=want + 2).contains(&ticks),
            "walked eight cells in {ticks} ticks, expected about {want}"
        );
    }

    #[test]
    fn a_road_gets_you_there_in_half_the_time() {
        let (mut w, mut nav, ox, oy) = walker();
        let goal = (ox + 6, oy);

        let plain = {
            let mut w = w.clone();
            let mut nav = Nav::new();
            w.citizens[0].walk_to(Dest::Cell(goal.0 as u8, goal.1 as u8));
            let mut n = 0;
            while w.citizens[0].state == State::Walking && n < 2000 {
                w.tick(&mut nav, &[]);
                n += 1;
            }
            n
        };

        // Pave every cell from the citizen to the goal, the one it starts on
        // included — speed is read from the cell being left.
        for i in 0..=6 {
            let id = w.place(PlayerId(0), Kind::Road, Facing::EastWest, ox + i, oy).unwrap();
            assert!(w.build_at(id, Kind::Road.build_ticks()));
        }
        w.citizens[0].walk_to(Dest::Cell(goal.0 as u8, goal.1 as u8));
        let mut paved = 0;
        while w.citizens[0].state == State::Walking && paved < 2000 {
            w.tick(&mut nav, &[]);
            paved += 1;
        }

        assert_eq!(w.citizens[0].pos.cell(), goal);
        assert!(
            paved * 2 <= plain + 2,
            "the road saved nothing: {paved} ticks paved against {plain} on grass"
        );
    }

    #[test]
    fn tired_citizens_walk_at_half_speed() {
        let c = &World::new(1, 2).citizens[0];
        assert_eq!(c.speed(false, false), Fx(WALK_SPEED));
        assert_eq!(c.speed(true, false), Fx(WALK_SPEED * 2), "a road doubles it");
        assert_eq!(c.speed(false, true), Fx(WALK_SPEED / 2), "wading halves it");
        assert_eq!(
            c.speed(true, true),
            Fx(WALK_SPEED * 2),
            "a road over a ford is a bridge, and a bridge is a road"
        );

        let mut tired = c.clone();
        tired.rest = TIRED - 1;
        assert_eq!(tired.speed(false, false), Fx(WALK_SPEED / 2));
        assert_eq!(
            tired.speed(true, false),
            Fx(WALK_SPEED),
            "tired on a road is the plain rate, not a quarter of it"
        );
        assert_eq!(
            tired.speed(false, true),
            Fx(WALK_SPEED / 4),
            "tired and wading is as slow as it gets"
        );
    }

    #[test]
    fn everyone_walking_to_one_place_shares_one_field() {
        let (mut w, mut nav, ox, oy) = walker();
        let goal = Dest::Cell((ox + 4) as u8, oy as u8);
        for i in 0..8 {
            w.citizens[i].pos = V2::cell_centre(ox - 3 + (i as i32 % 3), oy - 1 + (i as i32 / 3));
            w.citizens[i].walk_to(goal);
        }
        w.tick(&mut nav, &[]);
        assert_eq!(nav.len(), 1, "eight citizens built {} fields", nav.len());

        // Long enough to run out of food, which must not have stopped anybody
        // getting there: starving is a condition, not an activity. Expressed
        // against the constant rather than as a number, because the length of
        // a day is a thing design §11 leaves open and has already changed once.
        let past_empty = (NEED_FULL / FOOD_DECAY) as u32 + 50;
        for _ in 0..past_empty {
            w.tick(&mut nav, &[]);
        }
        for i in 0..8 {
            assert_eq!(w.citizens[i].state, State::Idle, "citizen {i} never arrived");
            assert!(
                w.citizens[i].starving(),
                "citizen {i} has not eaten in {past_empty} ticks and is not starving"
            );
        }
        assert_eq!(nav.len(), 1, "and still one field at the end");
    }

    #[test]
    fn walking_to_a_building_stops_beside_it_rather_than_inside() {
        let (mut w, mut nav, ox, oy) = walker();
        let id = w.place(PlayerId(0), Kind::Granary, Facing::EastWest, ox + 5, oy).unwrap();
        w.deliver_to(id, Good::Wood, 50);
        assert!(w.build_at(id, Kind::Granary.build_ticks()));

        w.citizens[0].walk_to(Dest::Building(id));
        for _ in 0..500 {
            w.tick(&mut nav, &[]);
            if w.citizens[0].state != State::Walking {
                break;
            }
        }
        let (cx, cy) = w.citizens[0].pos.cell();
        assert_eq!(w.citizens[0].state, State::Idle, "never got there");
        assert!(nav::at_building(&w.buildings[id.0 as usize], cx, cy), "stopped short");
        assert!(
            w.building_at(cx, cy).is_none(),
            "ended up standing inside the granary at ({cx},{cy})"
        );
    }

    #[test]
    fn ordering_somebody_onto_a_rock_walks_them_up_to_it_and_no_further() {
        let (mut w, mut nav, _ox, _oy) = walker();
        // `MoveTo` a boulder is a legal order and the field is seeded there
        // whether anyone can stand on it or not. Getting as close as possible
        // is what the player meant.
        let rock = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Rock)
            .unwrap();

        w.citizens[0].walk_to(Dest::Cell(rock.0 as u8, rock.1 as u8));
        for _ in 0..3000 {
            w.tick(&mut nav, &[]);
            if w.citizens[0].state != State::Walking {
                break;
            }
        }
        let (cx, cy) = w.citizens[0].pos.cell();
        assert_eq!(w.citizens[0].state, State::Idle, "still trying to climb it");
        assert_ne!(w.map.ground_at(cx, cy), Ground::Rock, "stood on the rock");
        assert!(nav::passable(&w, cx, cy), "ended up somewhere nobody can stand");
    }

    #[test]
    fn a_citizen_standing_where_nobody_can_walks_out_of_it() {
        let (mut w, mut nav, ox, oy) = walker();
        // Standing on rock, which no flow field ever reaches.
        //
        // This assertion used to be that the citizen stopped and stayed put,
        // on the grounds that "the citizen is not stuck in a wall, it is
        // standing still, and whatever wanted it to move can ask again". That
        // reasoning holds when there is a path from where it stands and fails
        // completely when there is not: asking again changes nothing, and the
        // citizen is stuck for the rest of the run. It matters because every
        // citizen starts inside a building — a Hearth blocks movement and the
        // founding party spawns on its site — so "select everybody and send
        // them uphill" on the first day left whoever had not wandered off yet
        // standing in the fire, permanently unorderable. Walking out of the
        // footprint first is the only reading of the order that can be obeyed.
        let rock = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Rock)
            .unwrap();
        w.citizens[0].pos = V2::cell_centre(rock.0, rock.1);

        w.citizens[0].walk_to(Dest::Cell(ox as u8, oy as u8));
        for _ in 0..600 {
            w.tick(&mut nav, &[]);
        }
        let (cx, cy) = w.citizens[0].pos.cell();
        assert!(
            nav::passable(&w, cx, cy),
            "it is still standing where nobody can stand: ({cx},{cy})"
        );
    }

    #[test]
    fn the_dead_do_not_walk() {
        let (mut w, mut nav, ox, oy) = walker();
        w.citizens[0].walk_to(Dest::Cell((ox + 5) as u8, oy as u8));
        w.tick(&mut nav, &[]);
        assert_eq!(w.citizens[0].state, State::Walking);

        // Dying clears what only makes sense for the living, so a corpse is
        // not left holding a destination it will never reach.
        w.citizens[0].die();
        assert_eq!(w.citizens[0].dest, None);
        assert_eq!(w.citizens[0].vel, V2::ZERO);

        let before = w.citizens[0].pos;
        for _ in 0..50 {
            w.tick(&mut nav, &[]);
        }
        assert_eq!(w.citizens[0].pos, before, "a corpse went for a walk");

        // And they cannot be sent anywhere.
        w.citizens[0].walk_to(Dest::Cell(ox as u8, oy as u8));
        assert_eq!(w.citizens[0].state, State::Dead);
        assert_eq!(w.citizens[0].dest, None);
    }

    #[test]
    fn losing_the_destination_mid_walk_stops_the_citizen() {
        let (mut w, mut nav, ox, oy) = walker();
        let id = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, ox + 6, oy).unwrap();
        w.citizens[0].walk_to(Dest::Building(id));
        for _ in 0..10 {
            w.tick(&mut nav, &[]);
        }
        assert_eq!(w.citizens[0].state, State::Walking, "should still be on its way");

        // The flood takes it, or the player pulls it down.
        w.demolish(PlayerId(0), id).unwrap();
        w.tick(&mut nav, &[]);
        assert_eq!(w.citizens[0].state, State::Idle, "walked on to a hole in the ground");
        assert_eq!(w.citizens[0].dest, None);
    }

    #[test]
    fn a_day_is_a_day() {
        let mut w = World::new(1, 2);
        assert_eq!(w.day(), 1);
        for _ in 0..TICKS_PER_DAY {
            w.tick_alone();
        }
        assert_eq!(w.day(), 2);
    }
}
