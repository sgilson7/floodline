//! Citizens take up room: they do not stand inside each other, and they do not
//! walk through walls.
//!
//! Two rules, run after everything else has moved somebody — walking, the
//! flood pushing bodies about, a citizen stepping out of a building it started
//! inside. Doing it once at the end rather than inside each of those is what
//! keeps the invariant true whatever moved them.
//!
//! **Everything here is integer and every loop has a fixed order**, for the
//! usual reason: two peers that resolved a crowd in different orders would
//! push the same two people in different directions and the game would be a
//! different game on each machine within a second. Citizens are visited by id,
//! neighbours are found through a cell index built in id order, and the push
//! is whole 1/256ths.

use crate::balance::*;
use crate::fx::{Fx, V2};
use crate::map::{Map, CELLS};
use crate::nav;
use crate::world::World;

/// A cell index of who is standing where, rebuilt each tick.
///
/// A flat `Vec` over the map rather than a map of lists: sixteen thousand
/// `u16`s is thirty-two kilobytes, it is cleared with a memset, and it makes
/// "who else is near me" nine lookups instead of five hundred comparisons. At
/// five hundred citizens the pairwise version is a quarter of a million
/// distance checks a tick against a budget of twenty milliseconds.
pub struct Crowd {
    /// First citizen in each cell, or `NONE`.
    head: Vec<u16>,
    /// Next citizen in the same cell, by citizen id.
    next: Vec<u16>,
}

const NONE: u16 = u16::MAX;

impl Default for Crowd {
    fn default() -> Crowd {
        Crowd::new()
    }
}

impl Crowd {
    pub fn new() -> Crowd {
        Crowd { head: vec![NONE; CELLS], next: Vec::new() }
    }

    fn fill(&mut self, world: &World) {
        for h in self.head.iter_mut() {
            *h = NONE;
        }
        self.next.clear();
        self.next.resize(world.citizens.len(), NONE);
        // Backwards, so each cell's list comes out in ascending id order and
        // the order two citizens are compared in never depends on anything
        // but the world.
        for i in (0..world.citizens.len()).rev() {
            let c = &world.citizens[i];
            if !c.alive() {
                continue;
            }
            let (x, y) = c.pos.cell();
            if !Map::contains(x, y) {
                continue;
            }
            let cell = Map::idx(x, y);
            self.next[i] = self.head[cell];
            self.head[cell] = i as u16;
        }
    }
}

impl World {
    /// Push anybody standing on top of anybody else apart, and put anybody
    /// standing in a wall back outside it.
    pub(crate) fn settle_crowd(&mut self) {
        // Built fresh each tick rather than threaded through `tick` beside
        // `Nav`: it is thirty-two kilobytes of `u16` and a memset, which at ten
        // ticks a second is nothing, and it keeps the signature of the one door
        // into the world from growing a second cache.
        let mut crowd = Crowd::new();
        crowd.fill(self);

        // One pass. Not iterated to convergence: a crowd resolves over several
        // ticks anyway because everybody is walking, and a fixed amount of
        // work per tick is worth more here than a perfect answer this tick.
        for i in 0..self.citizens.len() {
            if !self.citizens[i].alive() {
                continue;
            }
            let me = self.citizens[i].pos;
            let (cx, cy) = me.cell();
            if !Map::contains(cx, cy) {
                continue;
            }

            let mut push = V2::ZERO;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if !Map::contains(nx, ny) {
                        continue;
                    }
                    let mut j = crowd.head[Map::idx(nx, ny)];
                    while j != NONE {
                        let other = j as usize;
                        j = crowd.next[other];
                        if other == i {
                            continue;
                        }
                        let away = me - self.citizens[other].pos;
                        let gap = away.len_sq();
                        if gap.0 >= ELBOW_ROOM_SQ.0 {
                            continue;
                        }
                        // Two people on the same spot have no direction to be
                        // pushed apart along, so the lower id steps aside in a
                        // fixed one. Whoever it is, it is the same on every
                        // peer.
                        //
                        // "On the same spot" is generous on purpose. `with_len`
                        // normalises by dividing by a length that is itself
                        // computed in 1/256ths, so a vector shorter than about
                        // a sixteenth of a cell has a squared length of zero
                        // and normalises to *nothing at all* — the first
                        // version used a one-256th unit vector here and pushed
                        // by exactly zero for ever.
                        let dir = if gap.0 < 1 {
                            let sign = if i < other { -1 } else { 1 };
                            V2::new(Fx::cells(sign), Fx(0))
                        } else {
                            away
                        };
                        push = push + dir.with_len(ELBOW_PUSH);
                    }
                }
            }

            if push.x.0 == 0 && push.y.0 == 0 {
                continue;
            }
            let want = me + push;
            self.citizens[i].pos = self.somewhere_to_stand(me, want);
        }

        // And nobody ends a tick inside a wall, whatever put them there.
        for i in 0..self.citizens.len() {
            if !self.citizens[i].alive() {
                continue;
            }
            let (x, y) = self.citizens[i].pos.cell();
            if nav::passable(self, x, y) {
                continue;
            }
            if let Some(out) = self.nearest_standing_room(x, y) {
                self.citizens[i].pos = out;
            }
        }
    }

    /// `want`, unless that is inside something; then as far along the way as
    /// keeps its feet on ground somebody can stand on.
    ///
    /// Each axis on its own, so somebody pushed diagonally against a wall
    /// slides along it instead of stopping dead — which is what stops a crowd
    /// at a granary door from jamming solid.
    fn somewhere_to_stand(&self, from: V2, want: V2) -> V2 {
        let mut out = from;
        let try_x = V2::new(want.x, out.y);
        if self.standing_room(try_x) {
            out = try_x;
        }
        let try_y = V2::new(out.x, want.y);
        if self.standing_room(try_y) {
            out = try_y;
        }
        out
    }

    fn standing_room(&self, at: V2) -> bool {
        let (x, y) = at.cell();
        nav::passable(self, x, y)
    }

    /// The middle of the nearest cell anybody could stand in, searched in a
    /// fixed order. `None` if there is none within reach, which on this map
    /// there always is.
    fn nearest_standing_room(&self, cx: i32, cy: i32) -> Option<V2> {
        for r in 1..=4i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if nav::passable(self, cx + dx, cy + dy) {
                        return Some(V2::cell_centre(cx + dx, cy + dy));
                    }
                }
            }
        }
        None
    }
}
