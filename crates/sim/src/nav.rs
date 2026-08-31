//! Getting there: one flow field per destination, shared by everyone going to
//! it.
//!
//! Design §3.2 is specific about why. Five hundred citizens heading for the
//! granary cost one Dijkstra outward from the granary, not five hundred A*
//! searches inward — and the field only has to be rebuilt when the terrain or
//! the footprints change, which is rarely.
//!
//! **Flow fields are not part of `World`.** Each one is sixteen thousand cells,
//! and a dozen of them would be more state than the entire rest of the game;
//! putting them in `World` would put them in the snapshot a late joiner
//! receives and in the checksum computed every tick. They are a cache,
//! rebuilt identically on every peer from world state that *is* checksummed,
//! so a peer that rebuilds one differently has already diverged somewhere the
//! checksum can see.

use crate::balance::*;
use crate::building::{Building, BuildingId};
use crate::map::{Ground, Map, CELLS, MAP_H, MAP_W};
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Somewhere a citizen can be sent.
///
/// Part of `World` — a citizen's destination survives a snapshot, because a
/// late joiner has to see everyone still walking where they were walking.
/// The *field* that gets them there is the cache, and that is not.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Dest {
    Building(BuildingId),
    Cell(u8, u8),
}

/// The eight neighbours, in a fixed order. Every tie in this file breaks by
/// this order, which is what makes two peers pick the same step.
pub const DIRS: [(i32, i32); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// No direction — the goal, or nowhere to go from here.
pub const NO_DIR: u8 = 255;

/// Cost units. A straight step costs the cell's own cost; a diagonal costs
/// 14/10 of it, which is the usual integer stand-in for the square root of two
/// and is exact in the only sense that matters: it is the same fraction
/// everywhere.
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;

/// What it costs to step onto a cell, before the diagonal multiplier. Roads
/// are half, which is design §6's "citizens walk twice as fast on roads"
/// stated as a cost so that paths prefer roads rather than merely being
/// quicker along them by accident.
const COST_ROAD: u32 = 10;
const COST_GROUND: u32 = 20;
/// Six times open ground. Wading is slow and unpleasant and the pathing has to
/// agree with the walking, which halves a citizen's speed on a ford.
const COST_FORD: u32 = 120;

/// Unreachable.
pub const FAR: u32 = u32::MAX;

/// Cost to the goal from every cell, and the step to take.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlowField {
    pub dist: Vec<u32>,
    /// Index into `DIRS`, or `NO_DIR`.
    pub dir: Vec<u8>,
    /// The world generation this was built against.
    pub generation: u32,
}

impl FlowField {
    pub fn step_at(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        if !Map::contains(x, y) {
            return None;
        }
        match self.dir[Map::idx(x, y)] {
            NO_DIR => None,
            d => Some(DIRS[d as usize]),
        }
    }

    pub fn dist_at(&self, x: i32, y: i32) -> u32 {
        if Map::contains(x, y) {
            self.dist[Map::idx(x, y)]
        } else {
            FAR
        }
    }

    pub fn reachable(&self, x: i32, y: i32) -> bool {
        self.dist_at(x, y) != FAR
    }

    /// Dijkstra outward from `goals`.
    ///
    /// A binary heap keyed on `(cost, cell index)` rather than on cost alone:
    /// two cells at the same cost must come off the queue in the same order on
    /// every machine, and the index is what makes that ordering total.
    pub fn build(world: &World, goals: &[(i32, i32)]) -> FlowField {
        FlowField::build_into(world, goals, None)
    }

    /// The same, but able to walk *into* one building's footprint.
    ///
    /// A field built for `Dest::Building(id)` passes `Some(id)`, so the three
    /// farmers walking to a farm go inside it instead of piling up on
    /// whichever corner the field reached first. Everybody else is walking
    /// somewhere else, on a different field, and still cannot walk through it.
    ///
    /// One rule and not two: **you may go inside the place you are going to**.
    /// The plan says "somebody whose workplace is that building", which is a
    /// narrower rule that a shared flow field cannot express — a hauler
    /// carrying wheat to the granary is not employed there, and it should walk
    /// in at the door like anybody with business inside.
    pub fn build_into(
        world: &World,
        goals: &[(i32, i32)],
        into: Option<crate::building::BuildingId>,
    ) -> FlowField {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let mut dist = vec![FAR; CELLS];
        let mut dir = vec![NO_DIR; CELLS];
        let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();

        for &(gx, gy) in goals {
            if Map::contains(gx, gy) {
                let i = Map::idx(gx, gy);
                if dist[i] != 0 {
                    dist[i] = 0;
                    heap.push(Reverse((0, i as u32)));
                }
            }
        }

        while let Some(Reverse((d, i))) = heap.pop() {
            if d > dist[i as usize] {
                continue; // a shorter way here was already settled
            }
            let x = i as i32 % MAP_W;
            let y = i as i32 / MAP_W;

            for (k, &(dx, dy)) in DIRS.iter().enumerate() {
                let open = |x: i32, y: i32| passable_into(world, x, y, into);
                let (nx, ny) = (x + dx, y + dy);
                if !open(nx, ny) {
                    continue;
                }
                // No cutting the corner of a building or a rock: a diagonal
                // step needs both of the cells it squeezes between.
                if dx != 0 && dy != 0 && (!open(x + dx, y) || !open(x, y + dy)) {
                    continue;
                }
                let step = enter_cost(world, nx, ny)
                    * if dx != 0 && dy != 0 { DIAGONAL } else { STRAIGHT }
                    / 10;
                let nd = d + step;
                let ni = Map::idx(nx, ny);
                if nd < dist[ni] {
                    dist[ni] = nd;
                    // The field points *back* toward the goal, so the step
                    // stored is the reverse of the one just taken.
                    dir[ni] = ((k + 4) % 8) as u8;
                    heap.push(Reverse((nd, ni as u32)));
                }
            }
        }

        FlowField { dist, dir, generation: world.nav_generation }
    }
}

/// Whether a citizen can stand on a cell.
///
/// Shallows are impassable unless something has been built across them, which
/// is what a Bridge is for. A ford is the exception: it is water you can wade,
/// and every map has at least one so the far bank of the river is reachable
/// before anybody has built anything. A construction site is not blocking —
/// builders have to stand in it.
///
/// **A ford closes when the water comes.** A surge running down the channel
/// makes the crossing you have been relying on as deep as the rest of it, and
/// the tutorial says so once. This is the only place passability asks about
/// the water, and it asks at the same depth a citizen starts wading at, so
/// "you cannot path across it" and "you would be swept off it" arrive
/// together rather than a few ticks apart.
pub fn passable(world: &World, x: i32, y: i32) -> bool {
    if !Map::contains(x, y) {
        return false;
    }
    if let Some(b) = world.building_at(x, y) {
        if b.blocks_movement() {
            return false;
        }
        if b.carries_traffic() {
            return true; // a bridge over water is a road
        }
    }
    match world.map.ground_at(x, y) {
        Ground::Rock | Ground::Shallows => false,
        Ground::Ford => world.water.depth_at(x, y) < WADE_DEPTH,
        Ground::Grass | Ground::Sand => true,
    }
}

/// Whether a cell can be walked, for somebody on their way into `into`.
///
/// The exception the plan asks for, and the only one: a building's own cells
/// are open to whoever is walking to it. Everything else still cannot be
/// walked through, which is what keeps a city a place with buildings in it
/// rather than a field with decorations on.
pub fn passable_into(
    world: &World,
    x: i32,
    y: i32,
    into: Option<crate::building::BuildingId>,
) -> bool {
    if let (Some(id), Some(b)) = (into, world.building_at(x, y)) {
        if b.id == id && b.state != crate::building::BuildState::Rubble {
            return Map::contains(x, y);
        }
    }
    passable(world, x, y)
}

/// The cost of stepping onto a cell.
///
/// A ford costs several times open ground, which is the whole relationship the
/// river is for: crossable without a bridge, and worth the long way round to
/// one the moment somebody builds it. A bridge is a road and costs a road.
fn enter_cost(world: &World, x: i32, y: i32) -> u32 {
    if let Some(b) = world.building_at(x, y) {
        if b.carries_traffic() {
            return COST_ROAD;
        }
    }
    if world.map.ground_at(x, y) == Ground::Ford {
        COST_FORD
    } else {
        COST_GROUND
    }
}

/// The cache. Held next to a `World`, never inside one.
#[derive(Clone, Default, Debug)]
pub struct Nav {
    fields: BTreeMap<Dest, FlowField>,
    /// Least-recently-used last, so eviction takes the front.
    order: Vec<Dest>,
}

impl Nav {
    pub fn new() -> Nav {
        Nav::default()
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Throw everything away. Cheaper than reasoning about which fields a
    /// change touched, and a placement is rare next to a tick.
    pub fn clear(&mut self) {
        self.fields.clear();
        self.order.clear();
    }

    /// The field for `dest`, building or rebuilding it if it is missing or was
    /// computed before the last change to the map.
    pub fn field(&mut self, world: &World, dest: Dest) -> &FlowField {
        let stale = self
            .fields
            .get(&dest)
            .map(|f| f.generation != world.nav_generation)
            .unwrap_or(true);

        if stale {
            let goals = goal_cells(world, dest);
            // A field aimed at a building can walk into it. See `build_into`.
            let into = match dest {
                Dest::Building(id) => Some(id),
                Dest::Cell(..) => None,
            };
            let field = FlowField::build_into(world, &goals, into);
            if self.fields.insert(dest, field).is_none() {
                self.order.push(dest);
                if self.order.len() > NAV_CACHE_MAX {
                    let oldest = self.order.remove(0);
                    self.fields.remove(&oldest);
                }
            }
        }
        self.touch(dest);
        &self.fields[&dest]
    }

    /// Move `dest` to the back of the eviction queue.
    fn touch(&mut self, dest: Dest) {
        if let Some(i) = self.order.iter().position(|&d| d == dest) {
            let d = self.order.remove(i);
            self.order.push(d);
        }
    }
}

/// The cells a field is seeded from.
///
/// A building's goal is its own footprint, so the field points at the whole
/// thing rather than at one corner of it. A missing building has no goal at
/// all, which yields a field where nothing is reachable — the right answer for
/// "walk to the granary that just washed away".
pub fn goal_cells(world: &World, dest: Dest) -> Vec<(i32, i32)> {
    match dest {
        Dest::Cell(x, y) => vec![(x as i32, y as i32)],
        Dest::Building(id) => match world.buildings.get(id.0 as usize) {
            // Seeded at the middle rather than over the whole footprint.
            //
            // Every cell at distance zero means the field is flat across the
            // building, so the first cell of it anybody touches is as good as
            // any other and three farmers stack on whichever corner they
            // reached — which is what a farm has looked like since phase 1.
            // One goal in the middle gives the inside of the building a
            // gradient, and they spread through it.
            Some(b) if b.state != crate::building::BuildState::Rubble => vec![b.centre()],
            _ => Vec::new(),
        },
    }
}

/// Whether a cell is at or beside a building's footprint — near enough to
/// deliver to it, work in it, or eat at it.
pub fn at_building(b: &Building, x: i32, y: i32) -> bool {
    let (w, h) = b.size();
    let (bx, by) = (b.x as i32, b.y as i32);
    x >= bx - 1 && y >= by - 1 && x <= bx + w && y <= by + h
}

/// Sanity check used by the tests and by nothing else: that the field's
/// directions really do descend toward the goal.
pub fn descends(field: &FlowField, x: i32, y: i32) -> bool {
    match field.step_at(x, y) {
        None => true,
        Some((dx, dy)) => field.dist_at(x + dx, y + dy) < field.dist_at(x, y),
    }
}

/// Where a citizen ends up if it follows the field from `(x, y)`, and how many
/// steps it took. `None` if it walked in a circle, which it must not.
pub fn follow(field: &FlowField, x: i32, y: i32, limit: usize) -> Option<((i32, i32), usize)> {
    let (mut cx, mut cy) = (x, y);
    for n in 0..limit {
        match field.step_at(cx, cy) {
            None => return Some(((cx, cy), n)),
            Some((dx, dy)) => {
                cx += dx;
                cy += dy;
                if !Map::contains(cx, cy) {
                    return None;
                }
            }
        }
    }
    None
}

/// Only used to keep the unused-import warning honest about `MAP_H`.
const _: () = assert!(MAP_H == 128);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::{Facing, Good, Kind};
    use crate::citizen::PlayerId;

    /// How much clear ground the layout tests need around their centre.
    const ROOM: i32 = 6;

    /// A world and the middle of a clear patch of ground in it, big enough to
    /// put buildings around without bumping into the terrain.
    ///
    /// Searched for rather than written down. A hard-coded cell would be a
    /// hostage to the map generator: the first version of this helper picked a
    /// hearth site, which is inside a 3 x 3 Hearth — a *blocking* building —
    /// so nothing in the patch was passable at all.
    fn open_world() -> (World, i32, i32) {
        let w = World::new(31, 2);
        for y in ROOM..MAP_H - ROOM {
            for x in ROOM..MAP_W - ROOM {
                let clear = (-ROOM..=ROOM)
                    .all(|dy| (-ROOM..=ROOM).all(|dx| passable(&w, x + dx, y + dy)));
                if clear {
                    return (w, x, y);
                }
            }
        }
        panic!("no {}x{} patch of open ground anywhere on the map", 2 * ROOM + 1, 2 * ROOM + 1);
    }

    #[test]
    fn a_field_descends_to_its_goal_from_everywhere_it_reaches() {
        // The goal is an open cell, not a hearth: a Hearth is a 3 x 3 blocking
        // building, and a field seeded at its centre cannot expand out of its
        // own footprint. That is why `goal_cells` seeds a building's whole
        // footprint rather than its middle, and it is worth having found out
        // here rather than in a citizen who would not walk.
        let (w, gx, gy) = open_world();
        let f = FlowField::build(&w, &[(gx, gy)]);

        assert_eq!(f.dist_at(gx, gy), 0, "the goal costs nothing to reach");
        assert_eq!(f.step_at(gx, gy), None, "and has nowhere further to go");

        let mut reachable = 0;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if !f.reachable(x, y) {
                    continue;
                }
                reachable += 1;
                assert!(descends(&f, x, y), "({x},{y}) does not step downhill");
                // Following it always arrives, and never loops.
                let (end, _) = follow(&f, x, y, 4 * CELLS)
                    .unwrap_or_else(|| panic!("walked in a circle from ({x},{y})"));
                assert_eq!(end, (gx, gy), "from ({x},{y})");
            }
        }
        assert!(reachable > CELLS / 2, "only {reachable} cells could reach the hearth");
    }

    #[test]
    fn unreachable_ground_has_no_direction() {
        let (w, gx, gy) = open_world();
        let f = FlowField::build(&w, &[(gx, gy)]);
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if !f.reachable(x, y) {
                    assert_eq!(f.step_at(x, y), None, "({x},{y}) points somewhere");
                }
                // Rock and open water are never reachable and never stepped on.
                if !passable(&w, x, y) {
                    assert!(!f.reachable(x, y) || f.dist_at(x, y) == 0);
                }
            }
        }
    }

    #[test]
    fn nothing_walks_on_rock_or_into_the_river() {
        let w = World::new(7, 2);
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                // A building of its own decides the answer; this is about the
                // ground under it.
                if w.building_at(x, y).is_some() {
                    continue;
                }
                match w.map.ground_at(x, y) {
                    Ground::Rock | Ground::Shallows => {
                        assert!(!passable(&w, x, y), "({x},{y}) was walkable")
                    }
                    // A dry ford is the one piece of water you may wade.
                    Ground::Ford | Ground::Grass | Ground::Sand => {
                        assert!(passable(&w, x, y), "({x},{y}) was not walkable")
                    }
                }
            }
        }
        // The hearths, which are blocking buildings on buildable ground.
        for b in &w.buildings {
            for (x, y) in b.cells() {
                assert!(!passable(&w, x, y), "walked through a hearth at ({x},{y})");
            }
        }
        assert!(!passable(&w, -1, 0), "and not off the edge");
        assert!(!passable(&w, MAP_W, 0));
    }

    #[test]
    fn a_bridge_makes_water_walkable_and_losing_it_makes_it_water_again() {
        let mut w = World::new(2, 2);
        let wet = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Shallows)
            .unwrap();

        assert!(!passable(&w, wet.0, wet.1));
        let id = w.place(PlayerId(0), Kind::Bridge, Facing::EastWest, wet.0, wet.1).unwrap();
        assert!(!passable(&w, wet.0, wet.1), "a half-built bridge is still a river");

        w.deliver_to(id, Good::Wood, 20);
        w.build_at(id, Kind::Bridge.build_ticks());
        assert!(passable(&w, wet.0, wet.1), "and a finished one is a way across");

        w.damage_building(id, Kind::Bridge.integrity());
        assert!(!passable(&w, wet.0, wet.1), "and a broken one is a river again");
    }

    #[test]
    fn a_building_is_walked_around_but_a_dike_is_walked_over() {
        let (mut w, hx, hy) = open_world();
        let spot = (hx + 4, hy);

        let cottage = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, spot.0, spot.1).unwrap();
        assert!(passable(&w, spot.0, spot.1), "a site is walked through");
        w.deliver_to(cottage, Good::Wood, 30);
        w.build_at(cottage, Kind::Cottage.build_ticks());
        assert!(!passable(&w, spot.0, spot.1), "a cottage is walked around");

        let dspot = (hx - 4, hy);
        let dike = w.place(PlayerId(0), Kind::Dike, Facing::EastWest, dspot.0, dspot.1).unwrap();
        w.deliver_to(dike, Good::Stone, 40);
        w.build_at(dike, Kind::Dike.build_ticks());
        assert!(
            passable(&w, dspot.0, dspot.1),
            "a dike must not wall its own city in"
        );
    }

    #[test]
    fn a_road_costs_half_and_so_a_path_prefers_it() {
        let (mut w, hx, hy) = open_world();
        let goal = (hx, hy);

        // Straight out along a row, six cells of plain ground.
        let plain = FlowField::build(&w, &[goal]);
        let before = plain.dist_at(hx + 6, hy);

        for i in 1..=6 {
            let id = w.place(PlayerId(0), Kind::Road, Facing::EastWest, hx + i, hy).unwrap();
            assert!(w.build_at(id, Kind::Road.build_ticks()), "roads are free to build");
        }

        let paved = FlowField::build(&w, &[goal]);
        let after = paved.dist_at(hx + 6, hy);
        assert!(after < before, "paving a route did not make it cheaper: {after} vs {before}");
        // Six road cells at 10 rather than six ground cells at 20.
        assert_eq!(after, 60, "six cells of road");
        assert_eq!(before, 120, "six cells of ground");
    }

    #[test]
    fn a_diagonal_does_not_cut_a_corner() {
        let (mut w, hx, hy) = open_world();
        // Two cottages meeting at a corner, leaving a diagonal gap that must
        // not be squeezed through.
        let a = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, hx + 1, hy + 1).unwrap();
        let b = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, hx + 3, hy + 3).unwrap();
        for id in [a, b] {
            w.deliver_to(id, Good::Wood, 30);
            w.build_at(id, Kind::Cottage.build_ticks());
        }

        // (hx+3, hy+2) is diagonally past the corner where the two footprints
        // meet. Getting there must cost more than one diagonal step from
        // (hx+2, hy+1) would, because that step is not allowed.
        let f = FlowField::build(&w, &[(hx + 2, hy + 1)]);
        let corner_hop = f.dist_at(hx + 3, hy + 2);
        assert!(
            corner_hop > DIAGONAL * COST_GROUND / 10,
            "squeezed diagonally between two buildings for {corner_hop}"
        );
    }

    #[test]
    fn a_field_is_seeded_at_the_middle_and_reaches_the_whole_footprint() {
        // **This used to seed every cell of the footprint**, which made the
        // field flat across the building: the first cell of it anybody touched
        // was as good as any other, so three farmers stacked on whichever
        // corner they reached first and a farm looked like one person. One
        // goal in the middle gives the inside a gradient and they spread
        // through it — and every cell is still reachable, because a field
        // aimed at a building may walk into it.
        let (mut w, hx, hy) = open_world();
        let id = w.place(PlayerId(0), Kind::Farm, Facing::EastWest, hx + 3, hy - 1).unwrap();
        let cells: Vec<(i32, i32)> = w.buildings[id.0 as usize].cells().collect();
        assert_eq!(cells.len(), 9);
        let (mx, my) = w.buildings[id.0 as usize].centre();

        let mut nav = Nav::new();
        let f = nav.field(&w, Dest::Building(id));
        assert_eq!(f.dist_at(mx, my), 0, "the middle is the goal");
        for &(x, y) in &cells {
            assert!(f.reachable(x, y), "({x},{y}) of the farm cannot be walked to");
            if (x, y) != (mx, my) {
                assert!(f.dist_at(x, y) > 0, "({x},{y}) is flat with the middle");
            }
        }

        // And nobody else may walk through it, once it is built: a field
        // aimed somewhere else finds a standing farm solid. A *site* is not
        // solid and never was — builders have to stand in it.
        for g in crate::building::Good::ALL {
            let want = w.buildings[id.0 as usize].outstanding().get(g);
            if want > 0 {
                w.deliver_to(id, g, want);
            }
        }
        assert!(w.build_at(id, Kind::Farm.build_ticks()));
        let mut nav = Nav::new();
        let past = nav.field(&w, Dest::Cell((hx + 7) as u8, hy as u8));
        for &(x, y) in &cells {
            assert!(!past.reachable(x, y), "({x},{y}) was a thoroughfare");
        }
    }

    #[test]
    fn a_field_to_a_building_that_is_gone_reaches_nothing() {
        let (mut w, hx, hy) = open_world();
        let id = w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, hx + 3, hy).unwrap();
        let mut nav = Nav::new();
        assert!(nav.field(&w, Dest::Building(id)).reachable(hx, hy), "before");

        w.demolish(PlayerId(0), id).unwrap();
        assert!(
            !nav.field(&w, Dest::Building(id)).reachable(hx, hy),
            "a field to rubble should lead nowhere, not somewhere wrong"
        );
    }

    #[test]
    fn the_cache_rebuilds_only_when_the_map_changes() {
        let (mut w, hx, hy) = open_world();
        let dest = Dest::Cell(hx as u8, hy as u8);
        let mut nav = Nav::new();

        let gen0 = nav.field(&w, dest).generation;
        assert_eq!(gen0, w.nav_generation);
        assert_eq!(nav.len(), 1);

        // Asking again does not rebuild.
        assert_eq!(nav.field(&w, dest).generation, gen0);

        // Placing something does.
        w.place(PlayerId(0), Kind::Cottage, Facing::EastWest, hx + 5, hy).unwrap();
        assert!(w.nav_generation > gen0);
        assert_eq!(nav.field(&w, dest).generation, w.nav_generation);
        assert_eq!(nav.len(), 1, "rebuilt in place rather than added");
    }

    #[test]
    fn the_cache_does_not_grow_without_limit() {
        let (w, hx, hy) = open_world();
        let mut nav = Nav::new();
        for i in 0..(NAV_CACHE_MAX as i32 + 10) {
            nav.field(&w, Dest::Cell((hx + i % 20) as u8, (hy + i / 20) as u8));
            assert!(nav.len() <= NAV_CACHE_MAX, "cache grew to {}", nav.len());
        }
        assert_eq!(nav.len(), NAV_CACHE_MAX);
    }

    #[test]
    fn two_worlds_build_the_same_field() {
        // The fields are a cache and are not checksummed, so this is the test
        // that says two peers still navigate identically.
        let a = World::new(77, 3);
        let b = World::new(77, 3);
        for dest in [Dest::Building(BuildingId(0)), Dest::Cell(64, 64)] {
            let fa = Nav::new().field(&a, dest).clone();
            let fb = Nav::new().field(&b, dest).clone();
            assert_eq!(fa, fb, "{dest:?}");
        }
    }

    #[test]
    fn at_building_covers_the_footprint_and_one_cell_round_it() {
        let w = World::new(1, 2);
        let b = &w.buildings[0]; // a 3x3 hearth
        let (bx, by) = (b.x as i32, b.y as i32);
        assert!(at_building(b, bx, by), "its own corner");
        assert!(at_building(b, bx + 2, by + 2), "its far corner");
        assert!(at_building(b, bx - 1, by - 1), "diagonally beside it");
        assert!(at_building(b, bx + 3, by + 3));
        assert!(!at_building(b, bx - 2, by), "two cells away is not beside it");
        assert!(!at_building(b, bx + 4, by));
    }
}
