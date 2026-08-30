//! The world, and the one number that says whether two of them agree.
//!
//! Everything a run consists of hangs off `World`, including the `Rng`, and
//! the only way any of it changes is `tick` and `apply`. Design §7's rule —
//! "`gui` never constructs a `World` change except by handing a `Command` to
//! the lockstep" — is why the mutating methods are the short list they are.

use crate::balance::*;
use crate::building::{BuildState, Building, BuildingId, Good, Goods, Kind};
use crate::citizen::{Citizen, CitizenId, PlayerId, State};
use crate::fx::V2;
use crate::fx::Fx;
use crate::map::Map;
use crate::names::NAMES;
use crate::nav::{self, Dest, FlowField, Nav};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

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
    /// Bumped whenever a footprint appears or disappears.
    ///
    /// Flow fields are a cache outside `World` (see `nav`), and this is how
    /// they know they have gone stale. It lives in the world, and so in the
    /// checksum, because it counts something the world did: two peers whose
    /// generations differ have placed different numbers of buildings, and
    /// that is worth catching directly rather than inferring from the
    /// wreckage two minutes later.
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

        let mut world = World {
            seed,
            rng,
            tick: 0,
            map,
            citizens,
            buildings: Vec::new(),
            occupancy: vec![None; crate::map::CELLS],
            nav_generation: 0,
            players: (0..players).map(|p| PlayerId(p as u8)).collect(),
        };

        // The Hearths are already there when the run begins (design §4), each
        // holding what its city starts with. `level_pad` levelled a
        // HEARTH_SIZE square under every site during generation, so these
        // always fit.
        for p in 0..players {
            let (cx, cy) = world.map.hearth_sites[p as usize];
            let (w, h) = Kind::Hearth.size();
            let id = BuildingId(world.buildings.len() as u16);
            let mut hearth =
                Building::standing(id, PlayerId(p as u8), Kind::Hearth, cx - w / 2, cy - h / 2);
            hearth.store = Goods::of(0, STARTING_WOOD, STARTING_STONE);
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
        if !Building::fits_on_map(kind, x, y) {
            return Err(RuleError::OffMap);
        }
        if !Building::ground_suits(kind, &self.map, x, y) {
            return Err(RuleError::WrongGround);
        }
        for (cx, cy) in Building::footprint(kind, x, y) {
            if self.occupancy[Map::idx(cx, cy)].is_some() {
                return Err(RuleError::Occupied);
            }
        }
        Ok(())
    }

    /// Start a construction site. Materials still have to be hauled to it and
    /// builder-ticks spent on it before it is anything.
    pub fn place(
        &mut self,
        owner: PlayerId,
        kind: Kind,
        x: i32,
        y: i32,
    ) -> Result<BuildingId, RuleError> {
        self.can_place(owner, kind, x, y)?;
        let id = BuildingId(self.buildings.len() as u16);
        let b = Building::site(id, owner, kind, x, y);
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
        b.progress = 0;
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
        }
        ruined
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
    pub fn tick(&mut self, nav: &mut Nav) {
        for i in 0..self.citizens.len() {
            self.citizens[i].tick_needs();
        }
        self.walk(nav);
        self.tick += 1;
    }

    /// A tick without anywhere to walk to. For tests and for the phases where
    /// nothing has a destination yet.
    pub fn tick_alone(&mut self) {
        let mut nav = Nav::new();
        self.tick(&mut nav);
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
        if !nav::passable(self, cx + dx, cy + dy) {
            self.citizens[i].halt();
            return;
        }

        let on_road = self.building_at(cx, cy).map(|b| b.carries_traffic()).unwrap_or(false);
        let speed = self.citizens[i].speed(on_road);

        // The step is one of the eight neighbours, so its length is 1 or
        // sqrt(2); `with_len` scales it to the distance walked this tick and
        // keeps a diagonal from being 41% faster than a straight line.
        let dir = V2::new(Fx::cells(dx), Fx::cells(dy)).with_len(speed);
        self.citizens[i].vel = dir;
        self.citizens[i].pos += dir;
    }

    /// Whether citizen `i` has reached what it was walking to.
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
        for _ in 0..1000 {
            c.tick_needs();
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
                    if w.can_place(PlayerId(p as u8), kind, x, y).is_ok() {
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
                assert_eq!(b.store, Goods::of(0, STARTING_WOOD, STARTING_STONE));

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
        let id = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();

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
            w.can_place(PlayerId(0), Kind::Farm, MAP_W - 1, 5),
            Err(RuleError::OffMap)
        );
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, -1, 5), Err(RuleError::OffMap));
        assert!(w.place(PlayerId(0), Kind::Farm, MAP_W - 1, 5).is_err());
        assert_eq!(w.buildings.len(), 2, "a rejected placement builds nothing");
    }

    #[test]
    fn placing_refuses_an_occupied_footprint() {
        let mut w = World::new(6, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();

        // Exactly on top.
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, x, y), Err(RuleError::Occupied));
        // And merely overlapping by one cell, in each direction.
        for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1)] {
            assert_eq!(
                w.can_place(PlayerId(0), Kind::Cottage, x + dx, y + dy),
                Err(RuleError::Occupied),
                "overlap at ({dx},{dy}) was allowed"
            );
        }
        // Another player's building is in the way too — the map is shared.
        assert_eq!(w.can_place(PlayerId(1), Kind::Cottage, x, y), Err(RuleError::Occupied));
    }

    #[test]
    fn placing_refuses_the_wrong_ground() {
        let mut w = World::new(2, 2);
        let rock = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Rock)
            .expect("every map has rock");
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Dike, rock.0, rock.1),
            Err(RuleError::WrongGround)
        );

        let wet = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Shallows)
            .expect("every map has shallows");
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Cottage, wet.0, wet.1),
            Err(RuleError::WrongGround),
            "a cottage in the river"
        );
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Bridge, wet.0, wet.1),
            Ok(()),
            "but a bridge belongs there"
        );

        let dry = free_spot(&w, 0, Kind::Dike);
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Bridge, dry.0, dry.1),
            Err(RuleError::WrongGround),
            "and nowhere else"
        );
        let _ = w.place(PlayerId(0), Kind::Bridge, wet.0, wet.1).unwrap();
    }

    #[test]
    fn nobody_gets_a_second_hearth_and_nobody_commands_a_city_that_is_not_theirs() {
        let w = World::new(3, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        assert_eq!(
            w.can_place(PlayerId(0), Kind::Hearth, x, y),
            Err(RuleError::OneHearthOnly)
        );
        assert_eq!(
            w.can_place(PlayerId(9), Kind::Cottage, x, y),
            Err(RuleError::NotYours),
            "a player who is not in this run"
        );
    }

    #[test]
    fn a_building_goes_up_by_hauling_then_building() {
        let mut w = World::new(8, 2);
        let (x, y) = free_spot(&w, 0, Kind::Granary);
        let id = w.place(PlayerId(0), Kind::Granary, x, y).unwrap();

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
        let id = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();
        w.deliver_to(id, Good::Wood, 30);
        w.build_at(id, Kind::Cottage.build_ticks());

        assert_eq!(w.demolish(PlayerId(1), id), Err(RuleError::NotYours));
        let salvage = w.demolish(PlayerId(0), id).unwrap();
        assert_eq!(salvage, Goods::wood(15), "half the wood back");
        assert_eq!(w.buildings[id.0 as usize].state, BuildState::Rubble);
        assert!(w.building_at(x, y).is_none(), "the ground is free again");
        assert_eq!(w.can_place(PlayerId(0), Kind::Cottage, x, y), Ok(()));
        assert_eq!(w.demolish(PlayerId(0), BuildingId(999)), Err(RuleError::NoSuchBuilding));
    }

    #[test]
    fn rubble_keeps_its_id() {
        // The reason buildings are never removed from the vector: an id is an
        // index for the whole run, so nothing has to be remapped when a flood
        // takes half a city.
        let mut w = World::new(13, 2);
        let (x, y) = free_spot(&w, 0, Kind::Cottage);
        let first = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();
        w.demolish(PlayerId(0), first).unwrap();
        let second = w.place(PlayerId(0), Kind::Cottage, x, y).unwrap();
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
        let id = w.place(PlayerId(0), Kind::Dike, x, y).unwrap();
        assert_eq!(w.effective_height(x, y), base, "a site holds nothing back");

        w.deliver_to(id, Good::Stone, 40);
        w.build_at(id, Kind::Dike.build_ticks());
        assert_eq!(w.effective_height(x, y), base + DIKE_HEIGHT_PER_LEVEL);

        // Raising it puts it back under construction until the stone arrives.
        w.raise_dike(PlayerId(0), id).unwrap();
        assert_eq!(w.effective_height(x, y), base, "and it is a site again while it grows");
        assert_eq!(w.buildings[id.0 as usize].outstanding(), Goods::stone(40));
        w.deliver_to(id, Good::Stone, 40);
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
        let id = w.place(PlayerId(0), Kind::Dike, x, y).unwrap();
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
        let cottage = w.place(PlayerId(0), Kind::Cottage, cx, cy).unwrap();
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
                        if w.can_place(PlayerId(0), Kind::Stockpile, x, y).is_ok() {
                            placed = Some(w.place(PlayerId(0), Kind::Stockpile, x, y).unwrap());
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
        let id = w.place(PlayerId(0), Kind::Stockpile, x, y).unwrap();
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
            w.tick(&mut nav);
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
            w.tick(&mut nav);
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
                w.tick(&mut nav);
                n += 1;
            }
            n
        };

        // Pave every cell from the citizen to the goal, the one it starts on
        // included — speed is read from the cell being left.
        for i in 0..=6 {
            let id = w.place(PlayerId(0), Kind::Road, ox + i, oy).unwrap();
            assert!(w.build_at(id, Kind::Road.build_ticks()));
        }
        w.citizens[0].walk_to(Dest::Cell(goal.0 as u8, goal.1 as u8));
        let mut paved = 0;
        while w.citizens[0].state == State::Walking && paved < 2000 {
            w.tick(&mut nav);
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
        assert_eq!(c.speed(false), Fx(WALK_SPEED));
        assert_eq!(c.speed(true), Fx(WALK_SPEED * 2), "a road doubles it");

        let mut tired = c.clone();
        tired.rest = TIRED - 1;
        assert_eq!(tired.speed(false), Fx(WALK_SPEED / 2));
        assert_eq!(
            tired.speed(true),
            Fx(WALK_SPEED),
            "tired on a road is the plain rate, not a quarter of it"
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
        w.tick(&mut nav);
        assert_eq!(nav.len(), 1, "eight citizens built {} fields", nav.len());

        for _ in 0..400 {
            w.tick(&mut nav);
        }
        for i in 0..8 {
            assert_eq!(w.citizens[i].state, State::Idle, "citizen {i} never arrived");
            // Four hundred ticks is past the point where food runs out, and
            // that must not have stopped anybody: starving is a condition, not
            // an activity.
            assert!(w.citizens[i].starving(), "nobody has eaten in four hundred ticks");
        }
        assert_eq!(nav.len(), 1, "and still one field at the end");
    }

    #[test]
    fn walking_to_a_building_stops_beside_it_rather_than_inside() {
        let (mut w, mut nav, ox, oy) = walker();
        let id = w.place(PlayerId(0), Kind::Granary, ox + 5, oy).unwrap();
        w.deliver_to(id, Good::Wood, 50);
        assert!(w.build_at(id, Kind::Granary.build_ticks()));

        w.citizens[0].walk_to(Dest::Building(id));
        for _ in 0..500 {
            w.tick(&mut nav);
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
            w.tick(&mut nav);
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
    fn a_citizen_the_field_never_reached_stops_where_it_is() {
        let (mut w, mut nav, ox, oy) = walker();
        // Standing on rock, which no field ever reaches. This is the "the
        // path is gone" branch, and stopping is the honest answer: the citizen
        // is not stuck in a wall, it is standing still.
        let rock = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Rock)
            .unwrap();
        w.citizens[0].pos = V2::cell_centre(rock.0, rock.1);
        let before = w.citizens[0].pos;

        w.citizens[0].walk_to(Dest::Cell(ox as u8, oy as u8));
        w.tick(&mut nav);
        assert_eq!(w.citizens[0].state, State::Idle, "kept trying");
        assert_eq!(w.citizens[0].pos, before, "moved anyway");
        assert_eq!(w.citizens[0].dest, None);
    }

    #[test]
    fn the_dead_do_not_walk() {
        let (mut w, mut nav, ox, oy) = walker();
        w.citizens[0].walk_to(Dest::Cell((ox + 5) as u8, oy as u8));
        w.tick(&mut nav);
        assert_eq!(w.citizens[0].state, State::Walking);

        // Dying clears what only makes sense for the living, so a corpse is
        // not left holding a destination it will never reach.
        w.citizens[0].die();
        assert_eq!(w.citizens[0].dest, None);
        assert_eq!(w.citizens[0].vel, V2::ZERO);

        let before = w.citizens[0].pos;
        for _ in 0..50 {
            w.tick(&mut nav);
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
        let id = w.place(PlayerId(0), Kind::Cottage, ox + 6, oy).unwrap();
        w.citizens[0].walk_to(Dest::Building(id));
        for _ in 0..10 {
            w.tick(&mut nav);
        }
        assert_eq!(w.citizens[0].state, State::Walking, "should still be on its way");

        // The flood takes it, or the player pulls it down.
        w.demolish(PlayerId(0), id).unwrap();
        w.tick(&mut nav);
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
