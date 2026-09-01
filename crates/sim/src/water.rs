//! Shallow water, as a cellular automaton.
//!
//! Design §3.4's height-field method, entirely in integers. Each cell holds a
//! depth and the net movement through it; each tick, every cell offers water
//! to whichever of its four neighbours has a lower *surface* — terrain, plus
//! any dike, plus the water already there. That one rule gives water that
//! pools in valleys, backs up behind dikes, spills over when the level beats
//! them, and runs off the edges of the map.
//!
//! Two properties matter more than any particular number here, and both are
//! tested: **volume is conserved** except where water leaves the map, and the
//! automaton **never overshoots** — a cell will not push itself below a
//! neighbour it was above, which is what makes puddles settle instead of
//! sloshing forever. A sloshing puddle would not just look wrong; it would
//! never let the checksum settle either.

use crate::balance::*;
use crate::map::{Map, CELLS, MAP_H, MAP_W};
use serde::{Deserialize, Serialize};

/// The four neighbours, in a fixed order, and the direction each represents.
/// Diagonals are deliberately absent: water moving on a diagonal would cross
/// between two cells it never entered, and the volume bookkeeping stops
/// meaning anything.
const NEIGHBOURS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Water {
    /// Depth per cell, in sixteenths of a unit of terrain height. See
    /// `balance::DEPTH_SCALE` for why it is not simply terrain units.
    pub depth: Vec<u16>,
    /// Net movement through each cell this tick, per axis, in depth-units.
    /// This is what pushes citizens and breaks buildings.
    pub flow_x: Vec<i16>,
    pub flow_y: Vec<i16>,
    /// Everything that has run off the edges of the map. Kept so the
    /// conservation test can account for every unit ever created.
    pub drained: u64,
    /// Everything the sea has pushed *in* over the edges during a surge.
    pub poured: u64,
    /// Where the water has **ever** got to, one bit a cell.
    ///
    /// The high-water mark, and the thing both players in the M10.6 run asked
    /// for above everything else — in a game entirely about water height there
    /// was no way to ask where the water had been. One of them planned a whole
    /// age by screenshotting the previous flood at its peak and noting which
    /// pixels stayed green.
    ///
    /// **Ever, and not this age.** It was cleared when an age turned, on the
    /// reasoning that the mark should show the most recent flood and that
    /// doing so "is only ever a difference during a flood". That was the
    /// mistake: a flood lands on the *last* day of an age, so clearing at the
    /// age roll erased the record about a tick after it was written, and a
    /// player then spent days one to five of the next age looking at a blank
    /// map — which is the whole window in which a wall is sited and a refuge
    /// chosen. It is a difference on every day that is *not* a flood, which is
    /// five days in six.
    ///
    /// Measured: age one's flood marked 9 378 cells, and 6 532 were still
    /// showing on age 2 day 2 — the rest erased and only partly re-marked by
    /// water that had not drained. The missing third is the *edge* of the
    /// flood, which is exactly the line somebody is looking for.
    ///
    /// City 0 in the M12.11 run parked its last two citizens on ground that
    /// was unmarked after both previous floods, and the third took them. It
    /// read "no mark" as *the water never came here*; it meant *no data*.
    ///
    /// Cumulative costs nothing — the same bitset, never cleared — and since
    /// floods escalate the newest is also the largest, so this is what the
    /// original intent wanted in the first place.
    ///
    /// **It still under-reports the next flood**, because they escalate. That
    /// is not something the mark can fix and the panel says it instead: see
    /// `input::under_the_cursor`.
    ///
    /// **A bit and not a depth.** A `Welcome` with a snapshot was already
    /// 100 369 bytes against design §8's 150 KB, so the width had to be priced
    /// rather than chosen: a `u16` a cell is 32 KB of that headroom and a `u8`
    /// is 16, while a bit is two. What was asked for is a *line* — where did it
    /// reach — and a line is one bit. Measured after: 102 419 bytes, which is
    /// the 2 048 of the bitset and two more for its length.
    ///
    /// Cost in time, from `tests/profile.rs`: a flooding tick went from 0.75 ms
    /// to 0.78 against a 20 ms budget, because the marking is folded into the
    /// sweep `step` already makes rather than given a pass of its own.
    reached: Vec<u8>,
    /// How much water each cell's ground is holding, in depth-sixteenths.
    ///
    /// The ground is a sponge with a bottom in it. It drinks at
    /// `soak_rate` until it holds `soak_capacity` - *saturated* - and
    /// separately passes `AQUIFER_RATE` down to somewhere the game does not
    /// model, which frees room to drink again. So a cell under standing water
    /// loses its surface at the aquifer's pace and not the sponge's, which is
    /// the whole point: ground can be full and still be getting rid of water.
    ///
    /// **A byte a cell, and the width was priced rather than chosen.** A
    /// `Welcome` snapshot was 102 479 bytes against design §8's 150 KB, so a
    /// `u8` a cell is 16 KB of the 47 left and a `u16` would have been 32 -
    /// most of the headroom for one field. A byte holds 255 sixteenths and the
    /// largest capacity here is 24, so there is room to spare within the byte
    /// and none to spare outside it. Measured after: **118 867 bytes**, which
    /// leaves about 31 KB for everything this game has left to say.
    soaked: Vec<u8>,
    /// Steps since the ground last drank. See `balance::SOAK_EVERY`.
    ///
    /// **Kept here rather than read off `World::tick`**, and that is not
    /// tidiness. `step_water` is public so a test can drive the flood without
    /// a whole world turning over, and half the dike tests do exactly that -
    /// they pour, they step the water, and `world.tick` never moves. Phased on
    /// the world's clock, the ground drank on every one of those ticks instead
    /// of every twelfth, and five dike tests failed with "the surge did not
    /// lean on the wall at all", because it had been drunk before it arrived.
    ///
    /// One byte on the wire, and the automaton cannot be driven wrongly.
    soak_phase: u8,
}

impl Water {
    pub fn dry() -> Water {
        Water {
            depth: vec![0; CELLS],
            flow_x: vec![0; CELLS],
            flow_y: vec![0; CELLS],
            drained: 0,
            poured: 0,
            reached: vec![0; CELLS.div_ceil(8)],
            soaked: vec![0; CELLS],
            soak_phase: 0,
        }
    }

    /// How much water the ground is holding at this cell, in depth-sixteenths.
    pub fn soaked_at(&self, x: i32, y: i32) -> u16 {
        if Map::contains(x, y) {
            self.soaked[Map::idx(x, y)] as u16
        } else {
            0
        }
    }

    /// Whether this cell's ground has taken all it can and is now only passing
    /// water down. Standing water on saturated ground is standing water that
    /// will be there a while.
    pub fn saturated_at(&self, ground: crate::map::Ground, x: i32, y: i32) -> bool {
        let cap = crate::balance::soak_capacity(ground);
        cap > 0 && self.soaked_at(x, y) >= cap
    }

    /// Every unit the ground is holding. Neither on the surface nor gone, and
    /// `accounted` would not add up without it.
    pub fn held_by_ground(&self) -> u64 {
        self.soaked.iter().map(|&d| d as u64).sum()
    }

    /// Did the water reach this cell, deep enough to matter, since the age
    /// began?
    ///
    /// `WADE_DEPTH` is the line, which is design §3.4's own: below it the water
    /// is underfoot and harmless, at it a citizen is slowed. A mark that
    /// counted every damp cell would shade most of the map and say nothing.
    pub fn reached_at(&self, x: i32, y: i32) -> bool {
        if !Map::contains(x, y) {
            return false;
        }
        let i = Map::idx(x, y);
        self.reached[i / 8] & (1 << (i % 8)) != 0
    }

    /// How many cells the water has ever reached. For the tests and the
    /// probes; a player reads the map.
    pub fn marked_cells(&self) -> usize {
        self.reached.iter().map(|b| b.count_ones() as usize).sum()
    }



    pub fn depth_at(&self, x: i32, y: i32) -> u16 {
        if Map::contains(x, y) {
            self.depth[Map::idx(x, y)]
        } else {
            0
        }
    }

    /// Net movement through a cell, as (x, y) in depth-units.
    pub fn flow_at(&self, x: i32, y: i32) -> (i32, i32) {
        if Map::contains(x, y) {
            let i = Map::idx(x, y);
            (self.flow_x[i] as i32, self.flow_y[i] as i32)
        } else {
            (0, 0)
        }
    }

    /// How fast the water is going through a cell, regardless of direction.
    /// Taxicab rather than Euclidean: it needs no square root, and it is only
    /// ever compared against a threshold that was chosen to match it.
    pub fn speed_at(&self, x: i32, y: i32) -> u16 {
        let (fx, fy) = self.flow_at(x, y);
        (fx.abs() + fy.abs()).min(u16::MAX as i32) as u16
    }

    /// Water conserves: what is here, plus what has run off, is what was
    /// poured in — by the sea at the edges and by anything else that added to
    /// it. Used by the tests to account for every sixteenth.
    pub fn accounted(&self) -> u64 {
        self.volume() + self.drained + self.held_by_ground()
    }

    /// Every unit of water currently on the map.
    pub fn volume(&self) -> u64 {
        self.depth.iter().map(|&d| d as u64).sum()
    }

    pub fn wet_cells(&self) -> usize {
        self.depth.iter().filter(|&&d| d > 0).count()
    }

    /// Pour water in, as the surge does. Returns how much was actually added.
    pub fn add(&mut self, x: i32, y: i32, amount: u16) -> u16 {
        if !Map::contains(x, y) {
            return 0;
        }
        let i = Map::idx(x, y);
        let before = self.depth[i];
        self.depth[i] = before.saturating_add(amount);
        self.depth[i] - before
    }

    /// Raise a cell to at least `to`, which is what a source corner does.
    pub fn raise_to(&mut self, x: i32, y: i32, to: u16) -> u16 {
        if !Map::contains(x, y) {
            return 0;
        }
        let i = Map::idx(x, y);
        if self.depth[i] >= to {
            return 0;
        }
        let added = to - self.depth[i];
        self.depth[i] = to;
        added
    }

    /// One tick of the automaton.
    ///
    /// `ground` is the effective height of every cell — terrain plus any
    /// standing dike — passed in already computed, because working it out
    /// needs the building list and this module has no business knowing about
    /// buildings.
    ///
    /// `sea_surface` is the water level off the edges of the map, in the same
    /// sixteenths as everything else. Normally zero, which makes the edges a
    /// bottomless drain and is how the map dries out. **During a surge it is
    /// the height of the surge**, and that is not a detail: the source corner
    /// touches two edges of the map, so with a sea at zero an age-one flood
    /// poured in and ran straight back out beside itself — eight hundred
    /// thousand sixteenths off the edge against forty thousand ever on the
    /// map, and a front that stalled thirty cells in. A storm surge is the sea
    /// being high, so while it is happening the sea is high everywhere and the
    /// water has nowhere to go but inland.
    ///
    /// Two passes. The first works out what every cell will give away, using
    /// only the state at the start of the tick; the second applies it. One
    /// pass would make the result depend on the order cells were visited in,
    /// which is a desync with extra steps.
    pub fn step(&mut self, ground: &[i32], surfaces: &[crate::map::Ground], sea_surface: i32) {
        debug_assert_eq!(ground.len(), CELLS);
        debug_assert_eq!(surfaces.len(), CELLS);

        // What each cell will receive, and what it will lose. Signed, because
        // a cell both gives and takes in the same tick.
        let mut delta = vec![0i32; CELLS];
        let mut fx = vec![0i32; CELLS];
        let mut fy = vec![0i32; CELLS];
        let mut drained_now: u64 = 0;
        // What the ground takes in and passes down this tick. Kept apart from
        // `depth` for the same reason `delta` is: the first pass must see only
        // the state the tick began with, or the answer depends on which corner
        // of the map the loop started in.
        let mut soak_delta = vec![0i32; CELLS];
        // The ground works on a phase, not every tick. See `SOAK_EVERY`.
        self.soak_phase = (self.soak_phase + 1) % crate::balance::SOAK_EVERY as u8;
        // **And not at all while the sea is pouring in.**
        //
        // This is what keeps M5's dike balance intact, and it was measured
        // rather than assumed. Ground that drinks during a surge takes the
        // flood's leading edge with it - a spreading sheet is thin at the
        // front by definition - and with it on, a level-one wall that M5
        // measured as *breaking* under an age-one surge held instead, at 95%
        // strain and climbing. That is the central balance of the game moving
        // as a side effect of a drainage fix, which is not a trade anybody
        // asked for.
        //
        // It is also the honest model. Infiltration during a storm surge is
        // nothing against the sea arriving; between floods it is the only
        // thing happening. The water this mechanism exists to remove is the
        // pool left in a hollow afterwards, and that is exactly the water it
        // now touches.
        let soaking = self.soak_phase == 0 && sea_surface == 0;

        // Borrowed field by field rather than through `self`, so the mark can
        // be written inside the same sweep that reads the depths. `surface`
        // holds `depth` for the whole loop, and `reached` is a different field.
        let Water { depth, reached, soaked, .. } = self;

        // Ground is in terrain units and depth in sixteenths, so the surface
        // has to be spoken in sixteenths for the two to be compared at all.
        let surface = |i: usize| ground[i] * DEPTH_SCALE as i32 + depth[i] as i32;

        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let i = Map::idx(x, y);
                let here = depth[i];
                // Where the water got to, taken before it moves: a cell that
                // drains this tick was still under it when the tick began.
                //
                // In this loop rather than a pass of its own, because a second
                // sweep of sixteen thousand cells a tick is most of a tick's
                // budget on a map that is dry five days out of six.
                if here >= crate::balance::WADE_DEPTH {
                    reached[i / 8] |= 1 << (i % 8);
                }

                // What the ground does with it, in the same sweep. A pass of
                // its own would be sixteen thousand cells a tick on a map that
                // is dry five days out of six, which is the mistake the
                // high-water mark made once already.
                //
                // Before the `PUDDLE` test, deliberately: the aquifer has to
                // go on working under a cell that is nearly dry, or the last
                // sixteenth never leaves and the map keeps a film of water for
                // ever.
                // **Only water that is standing soaks in.** The flow field
                // from the tick before is already here and already says which
                // is which, so this costs a comparison and no state at all.
                //
                // Measured, and it is the difference between a model and a
                // sponge. Soaking every wet cell drinks a surge as it
                // advances: the leading edge of a spreading sheet is thin by
                // definition, and ground that takes twenty-four sixteenths out
                // of it takes most of what is there. A surge that stood 554
                // deep against a wall thirty-two cells inland reached it not
                // at all, and five dike tests failed with "the surge did not
                // lean on the wall".
                //
                // Water crossing the map at speed has not had time to go
                // anywhere; water that has come to rest in a hollow has. That
                // is both the physics and, more to the point, exactly the
                // water this exists to get rid of - the pool left behind after
                // a flood, which used to sit there until the run ended.
                let mut took = 0u16;
                // Wading depth, not swimming depth. See `SOAK_CEILING`.
                if soaking && here <= crate::balance::SOAK_CEILING {
                    let cap = crate::balance::soak_capacity(surfaces[i]);
                    let held = soaked[i] as u16;
                    // **Only saturated ground reaches the aquifer**, and that
                    // is the whole shape of this model rather than a detail.
                    //
                    // Draining every wet cell instead was measured and was far
                    // too strong: a surge advancing across the map is a *thin*
                    // sheet at its leading edge, and a sink on every wet cell
                    // drinks the edge as fast as it arrives. With the aquifer
                    // on every cell, a surge that used to stand 554 deep
                    // against a wall thirty-two cells inland never reached it
                    // at all, and five dike tests failed with "the surge did
                    // not lean on the wall".
                    //
                    // Requiring saturation splits the two behaviours the way
                    // the ground actually splits them: an advancing front pays
                    // the sponge once and moves on, while a pool that has sat
                    // long enough to fill the ground under it goes on losing
                    // to the aquifer for as long as it stands there. Standing
                    // water is what should drain; a flood on its way is not.
                    let down = if held >= cap && cap > 0 {
                        held.min(crate::balance::AQUIFER_RATE)
                    } else {
                        0
                    };
                    if down > 0 {
                        soak_delta[i] -= down as i32;
                        drained_now += down as u64;
                    }
                    let room = cap.saturating_sub(held - down);
                    took = crate::balance::soak_rate(surfaces[i])
                        .min(room)
                        .min(here.saturating_sub(crate::balance::DAMP));
                    if took > 0 {
                        soak_delta[i] += took as i32;
                        delta[i] -= took as i32;
                    }
                }

                if here <= PUDDLE {
                    continue;
                }
                let my_surface = surface(i);

                // Which neighbours are lower, and by how much. An off-map
                // neighbour is the sea: surface zero, bottomless, and what
                // goes there is gone.
                let mut deficit = [0i32; 4];
                let mut total_deficit = 0i32;
                let mut lower_surfaces = 0i32;
                let mut lower_count = 0i32;

                for (k, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
                    let (nx, ny) = (x + dx, y + dy);
                    let their_surface = if Map::contains(nx, ny) {
                        surface(Map::idx(nx, ny))
                    } else {
                        // Off the map is the sea, bottomless: what goes there
                        // is gone, which is how the map drains.
                        sea_surface
                    };
                    let d = my_surface - their_surface;
                    if d > 0 {
                        deficit[k] = d;
                        total_deficit += d;
                        lower_surfaces += their_surface;
                        lower_count += 1;
                    }
                }

                if lower_count == 0 {
                    continue;
                }

                // Give away only enough to reach the average of this cell and
                // the neighbours below it. This is what stops the automaton
                // overshooting — a cell never pushes itself below something it
                // was above — and it is why a puddle settles instead of
                // sloshing between two cells for ever.
                let average = (my_surface + lower_surfaces) / (lower_count + 1);
                let excess = (my_surface - average).max(0);
                // What the ground drank is no longer here to give away. Both
                // come out of `delta[i]` and together they may not take the
                // cell below nought.
                let give = excess
                    .min((here - took) as i32)
                    .min(MAX_TRANSFER as i32 * lower_count);

                if give <= 0 {
                    continue;
                }

                // Split it between the lower neighbours in proportion to how
                // far below they are, so water runs downhill fastest.
                let mut given = 0i32;
                for (k, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
                    if deficit[k] == 0 {
                        continue;
                    }
                    // Strictly proportional, with no remainder handling. The
                    // obvious alternative — hand the leftover to the last
                    // neighbour so the sum comes out exact — makes a puddle
                    // with four equally-lower neighbours spread lopsidedly,
                    // because "last" is whichever way the loop happens to run.
                    // The sixteenths that division loses simply stay put, which
                    // conserves volume just as well and keeps the puddle round.
                    let share = give * deficit[k] / total_deficit;
                    if share <= 0 {
                        continue;
                    }
                    given += share;

                    let (nx, ny) = (x + dx, y + dy);
                    if Map::contains(nx, ny) {
                        delta[Map::idx(nx, ny)] += share;
                    } else {
                        drained_now += share as u64;
                    }
                    // Flow is the movement itself, recorded on the cell it
                    // leaves so that a citizen standing there feels it.
                    fx[i] += dx * share;
                    fy[i] += dy * share;
                }
                delta[i] -= given;
            }
        }

        // The sea coming in.
        //
        // A storm surge is the sea being high, and a high sea does not politely
        // wait at the corner it was poured from — it comes over every low edge
        // it can reach. Without this the only water on the map is what a single
        // eight-by-eight block can push through its own perimeter, which
        // spreads about a dozen cells before it is too thin to drown anybody.
        // With it, everything below the surge's level fills, which is what a
        // surge does and what makes "get to high ground" the right advice.
        //
        // It stops by itself: water flows in only while the cell's surface is
        // below the sea's, so nothing can rise above sea level.
        if sea_surface > 0 {
            let mut edges: Vec<(usize, i32)> = Vec::new();
            for y in 0..MAP_H {
                for x in 0..MAP_W {
                    let on_edge = x == 0 || y == 0 || x == MAP_W - 1 || y == MAP_H - 1;
                    if !on_edge {
                        continue;
                    }
                    let i = Map::idx(x, y);
                    let head = sea_surface - (surface(i) + delta[i]);
                    if head > 0 {
                        edges.push((i, (head / 2).min(MAX_TRANSFER as i32)));
                    }
                }
            }
            for (i, amount) in edges {
                delta[i] += amount;
                drained_now = drained_now.saturating_sub(0);
                self.poured += amount as u64;
            }
        }

        for i in 0..CELLS {
            let d = self.depth[i] as i32 + delta[i];
            debug_assert!(d >= 0, "cell {i} went negative: {d}");
            self.depth[i] = d.clamp(0, u16::MAX as i32) as u16;
            if soak_delta[i] != 0 {
                let held = self.soaked[i] as i32 + soak_delta[i];
                debug_assert!(held >= 0, "cell {i} soaked went negative: {held}");
                self.soaked[i] = held.clamp(0, u8::MAX as i32) as u8;
            }
            self.flow_x[i] = fx[i].clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            self.flow_y[i] = fy[i].clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
        self.drained += drained_now;
    }
}
