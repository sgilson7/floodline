//! Roads between cities, and the barter that walks along them.
//!
//! Design §6 is the whole of it: a road is laid cell by cell along the
//! cheapest path, it is *joined* only when the city at the far end accepts it,
//! and a standing trade sends haulers along it once a day carrying goods that
//! can drown on the way. There is no market, no price and no currency — trade
//! is barter along a road you can watch, and watching it is the point.
//!
//! Road planning does not use `nav`'s flow fields. They answer "how does a
//! citizen walk to X", and a citizen cannot walk across shallows; a road can,
//! by becoming a bridge. Two questions, two cost functions.

use crate::balance::*;
use crate::building::{Good, Kind};
use crate::citizen::PlayerId;
use crate::map::{Ground, Map, CELLS, MAP_W};
use crate::world::World;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RoadId(pub u16);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TradeId(pub u16);

/// A road somebody laid, and who it reaches.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Road {
    pub id: RoadId,
    /// The city that paid for it.
    pub by: PlayerId,
    /// The city at the far end, if it ends at one. `None` is an internal road
    /// — perfectly useful for walking on, but nothing to accept.
    pub reaches: Option<PlayerId>,
    /// Whether the city at the far end has accepted it (design §6).
    pub joined: bool,
    /// Every cell of it, so that "is this link still there" is a question with
    /// an answer after a flood.
    pub cells: Vec<(u8, u8)>,
}

impl Road {
    /// Whether every cell of the road is still a standing road or bridge.
    ///
    /// This is what makes rebuilding a washed-out link a decision rather than
    /// a formality: the trade stops until somebody walks out and lays the
    /// broken cells again.
    pub fn intact(&self, world: &World) -> bool {
        self.cells.iter().all(|&(x, y)| {
            world
                .building_at(x as i32, y as i32)
                .map(|b| b.carries_traffic())
                .unwrap_or(false)
        })
    }

    /// Whether this road connects the two cities, both ways round.
    pub fn links(&self, a: PlayerId, b: PlayerId) -> bool {
        self.joined && ((self.by == a && self.reaches == Some(b)) || (self.by == b && self.reaches == Some(a)))
    }
}

/// A standing exchange, per day.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Trade {
    pub id: TradeId,
    /// Who proposed it, and so who gives `give`.
    pub from: PlayerId,
    pub with: PlayerId,
    pub give: (Good, u16),
    pub take: (Good, u16),
    /// Live only once the other player has said yes.
    pub accepted: bool,
}

impl Trade {
    pub fn involves(&self, p: PlayerId) -> bool {
        self.from == p || self.with == p
    }
}

/// What it costs to lay a road across a cell, or `None` if it cannot be done.
///
/// Reusing an existing road is nearly free, which is what makes a second road
/// out of a city follow the first for as long as it usefully can. Shallows
/// cost a bridge, which is expensive but allowed — design §6 says roads across
/// shallows need bridges, not that they cannot cross.
fn lay_cost(world: &World, x: i32, y: i32) -> Option<u32> {
    if !Map::contains(x, y) {
        return None;
    }
    if let Some(b) = world.building_at(x, y) {
        // An existing way is reused rather than rebuilt.
        if b.carries_traffic() || matches!(b.kind, Kind::Road | Kind::Bridge) {
            return Some(1);
        }
        // Anything else is somebody's building, and a road does not go
        // through it.
        return None;
    }
    match world.map.ground_at(x, y) {
        Ground::Grass | Ground::Sand => Some(ROAD_COST_GROUND),
        // A ford is the cheap crossing and it is still water: a road over it
        // is a bridge like any other, and bridging the ford is exactly the
        // improvement the ford exists to make worth paying for.
        Ground::Shallows | Ground::Ford => Some(ROAD_COST_WATER),
        Ground::Rock => None,
    }
}

/// The cheapest way to lay a road from one cell to another.
///
/// Four-connected, not eight: a diagonal road is two cells that only touch at
/// a corner, and a hauler cannot walk a corner-to-corner path any more than it
/// can squeeze between two buildings. A road has to be something you can
/// actually follow.
pub fn plan(world: &World, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    if !Map::contains(from.0, from.1) || !Map::contains(to.0, to.1) {
        return None;
    }
    if lay_cost(world, to.0, to.1).is_none() || lay_cost(world, from.0, from.1).is_none() {
        return None;
    }

    const NONE: u32 = u32::MAX;
    let mut dist = vec![NONE; CELLS];
    let mut prev = vec![NONE; CELLS];
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();

    let start = Map::idx(from.0, from.1) as u32;
    let goal = Map::idx(to.0, to.1);
    dist[start as usize] = 0;
    heap.push(Reverse((0, start)));

    while let Some(Reverse((d, i))) = heap.pop() {
        if i as usize == goal {
            break;
        }
        if d > dist[i as usize] {
            continue;
        }
        let x = i as i32 % MAP_W;
        let y = i as i32 / MAP_W;
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            let Some(step) = lay_cost(world, nx, ny) else {
                continue;
            };
            let ni = Map::idx(nx, ny);
            let nd = d + step;
            if nd < dist[ni] {
                dist[ni] = nd;
                prev[ni] = i;
                heap.push(Reverse((nd, ni as u32)));
            }
        }
    }

    if dist[goal] == NONE {
        return None;
    }

    let mut path = Vec::new();
    let mut at = goal;
    loop {
        path.push((at as i32 % MAP_W, at as i32 / MAP_W));
        if at == start as usize {
            break;
        }
        at = prev[at] as usize;
    }
    path.reverse();
    Some(path)
}

/// Which player's city a cell is at the edge of, if any but `mine`.
///
/// A road is joinable when it ends within reach of somebody else's building —
/// design §6's "a road that reaches another city's edge". Searched in building
/// order so two candidate cities resolve the same way on every peer.
pub fn city_at(world: &World, mine: PlayerId, x: i32, y: i32) -> Option<PlayerId> {
    world
        .buildings
        .iter()
        .filter(|b| b.owner != mine && b.standing_now() && b.kind != Kind::Road)
        .find(|b| {
            let (bx, by) = b.centre();
            (bx - x).abs() + (by - y).abs() <= ROAD_JOIN_REACH
        })
        .map(|b| b.owner)
}

/// The goods a city can spare, for a day's trade.
pub fn can_spare(world: &World, owner: PlayerId, good: Good, amount: u16) -> bool {
    world.treasury(owner).get(good) >= amount
}
