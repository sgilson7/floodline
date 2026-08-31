//! The land: a height per cell, a ground type per cell, and where the cities
//! start.
//!
//! Generated from the seed and nothing else, so the score screen can show a
//! seed and mean it — an unfair map can be replayed and argued about. Value
//! noise with integer interpolation, because the alternative is a float.

use crate::balance::*;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const MAP_W: i32 = 128;
pub const MAP_H: i32 = 128;
pub const CELLS: usize = (MAP_W * MAP_H) as usize;

/// What a cell is made of. Ordered by how wet it is, which is not an accident:
/// nothing depends on the ordering yet, and if something ever does, this is
/// the ordering it will want.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Ground {
    Shallows,
    /// A reach of the river shallow enough to wade: water you can cross on
    /// foot, slowly, and cannot build on except with a bridge.
    ///
    /// The river cuts the map in two and every generated map guarantees at
    /// least one of these, so the far bank is reachable without a bridge and
    /// better with one. It sits next to `Shallows` in the ordering because
    /// that is what it is — the same water, less of it.
    Ford,
    Sand,
    Grass,
    Rock,
}

impl Ground {
    /// Whether a building may stand here. Shallows and a ford need a bridge
    /// and rock cannot be dug, so all three are out.
    pub fn buildable(self) -> bool {
        matches!(self, Ground::Grass | Ground::Sand)
    }

    /// Whether this is river or sea rather than land. A bridge goes over
    /// either.
    pub fn watery(self) -> bool {
        matches!(self, Ground::Shallows | Ground::Ford)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Corner {
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl Corner {
    pub const ALL: [Corner; 4] =
        [Corner::NorthWest, Corner::NorthEast, Corner::SouthWest, Corner::SouthEast];

    /// The cell at this corner.
    pub fn cell(self) -> (i32, i32) {
        match self {
            Corner::NorthWest => (0, 0),
            Corner::NorthEast => (MAP_W - 1, 0),
            Corner::SouthWest => (0, MAP_H - 1),
            Corner::SouthEast => (MAP_W - 1, MAP_H - 1),
        }
    }

    pub fn opposite(self) -> Corner {
        match self {
            Corner::NorthWest => Corner::SouthEast,
            Corner::NorthEast => Corner::SouthWest,
            Corner::SouthWest => Corner::NorthEast,
            Corner::SouthEast => Corner::NorthWest,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Map {
    /// Terrain height, 0..=255. Not the water surface — that is `depth` on
    /// top of this, and it arrives in phase 2.
    pub height: Vec<u8>,
    pub ground: Vec<Ground>,
    /// The height each ground band starts at, chosen per map so that the
    /// composition is the same on every seed. See `balance::SHALLOWS_PERCENT`.
    pub shallows_max: u8,
    pub sand_max: u8,
    pub rock_min: u8,
    /// Where the flood comes from in ages 1–3 (design §5).
    pub low_corner: Corner,
    pub high_corner: Corner,
    /// The channel's centreline, in order from its source — the mouth on the
    /// high side, where the surge comes out — to its mouth on the low side,
    /// where the water runs off the map. Every cell of it is on the map and
    /// each is a king's step from the last.
    pub river: Vec<(u8, u8)>,
    /// One per player, in player order.
    pub hearth_sites: Vec<(i32, i32)>,
}

/// Cells outside the map read as rock at full height, so callers walking a
/// neighbourhood never have to special-case the border.
impl Map {
    #[inline]
    pub fn contains(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < MAP_W && y < MAP_H
    }

    #[inline]
    pub fn idx(x: i32, y: i32) -> usize {
        (y * MAP_W + x) as usize
    }

    pub fn height_at(&self, x: i32, y: i32) -> u8 {
        if Map::contains(x, y) {
            self.height[Map::idx(x, y)]
        } else {
            255
        }
    }

    pub fn ground_at(&self, x: i32, y: i32) -> Ground {
        if Map::contains(x, y) {
            self.ground[Map::idx(x, y)]
        } else {
            Ground::Rock
        }
    }

    pub fn buildable(&self, x: i32, y: i32) -> bool {
        Map::contains(x, y) && self.ground[Map::idx(x, y)].buildable()
    }

    /// Generate the whole map. `players` is 2..=6; anything outside is clamped
    /// rather than rejected, because a map with no sites is not a thing any
    /// caller could do something useful with.
    pub fn generate(rng: &mut Rng, players: u32) -> Map {
        let players = players.clamp(2, 6);

        let low_corner = Corner::ALL[rng.below(4) as usize];
        let high_corner = low_corner.opposite();
        let mut height = terrain(rng, low_corner, NOISE_AMPLITUDE, SLOPE_SPAN);

        // Carved before the bands are worked out, so the channel counts as the
        // shallows it is rather than being painted as an exception afterwards.
        // Shallows are already impassable and unbuildable, so the river
        // divides the map for free and every rule that already knew about
        // water knows about the river.
        let river = river_path(rng, low_corner, high_corner);
        let channel = carve_river(&mut height, &river);
        let ford = place_ford(rng, &mut height, &river);

        let (shallows_max, sand_max, rock_min) = band_heights(&height);
        let ground: Vec<Ground> = height
            .iter()
            .map(|&h| {
                if h <= shallows_max {
                    Ground::Shallows
                } else if h <= sand_max {
                    Ground::Sand
                } else if h >= rock_min {
                    Ground::Rock
                } else {
                    Ground::Grass
                }
            })
            .collect();

        let mut map = Map {
            height,
            ground,
            shallows_max,
            sand_max,
            rock_min,
            low_corner,
            high_corner,
            river: river.iter().map(|&(x, y)| (x as u8, y as u8)).collect(),
            hearth_sites: Vec::new(),
        };
        // The channel is water whatever the bands made of it. See
        // `carve_river` for why this cannot be left to the height.
        for (x, y) in channel {
            map.ground[Map::idx(x, y)] = Ground::Shallows;
        }
        // And one reach of it is water you can wade. Painted after the
        // channel so the ford wins where they overlap, and only over water:
        // the ford's reach covers the banks as well as the floor, and a bank
        // the bands left as grass is already something you can walk on.
        for (x, y) in ford {
            let i = Map::idx(x, y);
            if map.ground[i].watery() {
                map.ground[i] = Ground::Ford;
            }
        }
        map.place_hearth_sites(rng, players);
        map
    }

    /// How far every cell is from the nearest water, in king's steps.
    ///
    /// A field rather than a distance to a line, because the channel meanders:
    /// a site fourteen cells out along the perpendicular from one reach can be
    /// one cell from the next bend, and measuring against the line it was
    /// placed from would never notice. Multi-source and eight-connected, which
    /// is the same neighbourhood a citizen walks.
    pub fn distance_to_water(&self) -> Vec<i32> {
        self.flood_fill(|g| g.watery())
    }

    /// Distance to the river's centreline, and which cell of it was nearest.
    ///
    /// The second half is what says which bank a cell is on: the channel's
    /// direction at the reach a cell belongs to, crossed with the way out to
    /// that cell, is positive on one side and negative on the other.
    fn distance_to_river(&self) -> (Vec<i32>, Vec<u16>) {
        let mut d = vec![i32::MAX; CELLS];
        let mut from = vec![0u16; CELLS];
        let mut queue = std::collections::VecDeque::new();
        for (i, &(x, y)) in self.river.iter().enumerate() {
            let c = Map::idx(x as i32, y as i32);
            if d[c] != 0 {
                d[c] = 0;
                from[c] = i as u16;
                queue.push_back((x as i32, y as i32));
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let here = Map::idx(x, y);
            let (dist, src) = (d[here], from[here]);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if !Map::contains(nx, ny) {
                        continue;
                    }
                    let n = Map::idx(nx, ny);
                    if d[n] > dist + 1 {
                        d[n] = dist + 1;
                        from[n] = src;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        (d, from)
    }

    /// Which cells a road could reach from each other: everything that is not
    /// rock, four-connected, as `road::plan` walks it.
    ///
    /// Only the largest such region is any use. Rock is eight percent of the
    /// map in one mass at the high corner, and a river bank that runs past it
    /// has pockets — `two_cities_found_a_road_and_trade_for_three_days` put a
    /// city in one, and the road between the two cities could not be laid at
    /// all. A city nobody can reach cannot trade, which is half of design §6,
    /// and the map is not allowed to decide that.
    fn main_region(&self) -> Vec<bool> {
        let mut seen = vec![false; CELLS];
        let mut best: Vec<bool> = Vec::new();
        let mut best_n = 0;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if seen[Map::idx(x, y)] || self.ground[Map::idx(x, y)] == Ground::Rock {
                    continue;
                }
                let mut here = vec![false; CELLS];
                let mut n = 0;
                let mut queue = std::collections::VecDeque::from([(x, y)]);
                seen[Map::idx(x, y)] = true;
                here[Map::idx(x, y)] = true;
                while let Some((cx, cy)) = queue.pop_front() {
                    n += 1;
                    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                        let (nx, ny) = (cx + dx, cy + dy);
                        if !Map::contains(nx, ny) {
                            continue;
                        }
                        let i = Map::idx(nx, ny);
                        if seen[i] || self.ground[i] == Ground::Rock {
                            continue;
                        }
                        seen[i] = true;
                        here[i] = true;
                        queue.push_back((nx, ny));
                    }
                }
                if n > best_n {
                    best_n = n;
                    best = here;
                }
            }
        }
        best
    }

    /// Eight-connected distance from every cell matching `seed`.
    fn flood_fill(&self, seed: impl Fn(Ground) -> bool) -> Vec<i32> {
        let mut d = vec![i32::MAX; CELLS];
        let mut queue = std::collections::VecDeque::new();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let i = Map::idx(x, y);
                if seed(self.ground[i]) {
                    d[i] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let here = d[Map::idx(x, y)];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if !Map::contains(nx, ny) {
                        continue;
                    }
                    let n = Map::idx(nx, ny);
                    if d[n] > here + 1 {
                        d[n] = here + 1;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        d
    }

    /// One site per player, on the banks of the river, spread along it.
    ///
    /// **This replaced the shore parallel, which replaced a ring.** The ring
    /// was wrong because a circle about the map's centre is not equidistant
    /// from a corner; the shore parallel was right for a flood that came out
    /// of a corner, and the flood does not come out of a corner any more. What
    /// decides a city's fate now is how far it sits from the bank the water
    /// spills over, so that is the thing held fixed.
    ///
    /// **Farthest-point choice from a candidate band, not an offset from a
    /// line.** Two earlier versions offset a site perpendicular to the channel
    /// and both were wrong for the same reason — a meander means the line a
    /// site was placed from is not the nearest water to it. The band is every
    /// cell whose distance to the river is within `SITE_JITTER_BAND` of
    /// `SHORE_DISTANCE`; players take turns picking the cell in it that is
    /// furthest from everybody already placed. That gives the spacing
    /// guarantee by construction rather than by hope, and it is the only
    /// version of this that has ever measured well at six players.
    ///
    /// Banks alternate, so the river runs *between* the players. That is the
    /// first time in this game that a bridge, a road and a trade have had a
    /// reason to exist, and it is what M6's mules will have to get across.
    fn place_hearth_sites(&mut self, rng: &mut Rng, players: u32) {
        let (river_d, river_from) = self.distance_to_river();
        // A city has to be able to quarry, and to be reachable at all.
        let rock_d = self.flood_fill(|g| g == Ground::Rock);
        let reachable = self.main_region();
        let path: Vec<(i32, i32)> =
            self.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
        let n = path.len() as i32;
        let edge = HEARTH_SIZE / 2 + SITE_SNAP;
        let lo = SHORE_MARGIN.min(n / 3);
        let hi = (n - 1 - SHORE_MARGIN).max(lo);

        // The band, split by bank. Built in map order, so it depends on
        // nothing but the map.
        let mut banks: [Vec<((i32, i32), bool)>; 2] = [Vec::new(), Vec::new()];
        for y in edge..MAP_H - edge {
            for x in edge..MAP_W - edge {
                let i = Map::idx(x, y);
                if !self.ground[i].buildable() || !reachable[i] {
                    continue;
                }
                // Room to build, and room for a road to get out again.
                let mut open = 0;
                for jy in -SITE_ELBOW..=SITE_ELBOW {
                    for jx in -SITE_ELBOW..=SITE_ELBOW {
                        if Map::contains(x + jx, y + jy)
                            && reachable[Map::idx(x + jx, y + jy)]
                        {
                            open += 1;
                        }
                    }
                }
                let side = 2 * SITE_ELBOW + 1;
                if open * 100 < side * side * SITE_ELBOW_PERCENT {
                    continue;
                }
                let d = river_d[i];
                if (d - SHORE_DISTANCE).abs() > SITE_JITTER_BAND {
                    continue;
                }
                // Not beside a mouth: a city there is jammed into a corner,
                // and the one at the upstream mouth meets the surge before it
                // has spread at all.
                let at = river_from[i] as i32;
                if at < lo || at > hi {
                    continue;
                }
                // And within reach of the water. See `SITE_HEADROOM`.
                let (rx, ry) = path[at as usize];
                let head = self.height[i] as i32 - self.height[Map::idx(rx, ry)] as i32;
                if head < SITE_HEADROOM.0 || head > SITE_HEADROOM.1 {
                    continue;
                }
                let a = path[(at - 4).clamp(0, n - 1) as usize];
                let b = path[(at + 4).clamp(0, n - 1) as usize];
                let cross = (x - rx) * (b.1 - a.1) - (y - ry) * (b.0 - a.0);
                banks[usize::from(cross < 0)].push(((x, y), rock_d[i] <= QUARRY_REACH));
            }
        }

        let far_from = |sites: &[(i32, i32)], (x, y): (i32, i32)| -> i64 {
            sites
                .iter()
                .map(|&(sx, sy)| ((x - sx).pow(2) + (y - sy).pow(2)) as i64)
                .min()
                .unwrap_or(i64::MAX)
        };

        let mut sites: Vec<(i32, i32)> = Vec::with_capacity(players as usize);
        for i in 0..players {
            // Alternating, and falling back to the other bank only if this one
            // has nowhere at all.
            let side = (i % 2) as usize;
            let here = if banks[side].is_empty() { 1 - side } else { side };
            let pool = &banks[here];
            if pool.is_empty() {
                sites.push((MAP_W / 2, MAP_H / 2));
                continue;
            }
            let pick = if sites.is_empty() {
                // The seed decides where the first city goes; everything after
                // it is decided by the geometry.
                pool[rng.below(pool.len() as u32) as usize].0
            } else {
                // Spacing until it is enough, then rock, then more spacing.
                //
                // A plain filter on "has rock within reach" was tried and it
                // cost far too much: the rock-reachable part of a bank is
                // small, and farthest-point choice inside it put two cities
                // one cell apart at six players. Ranking rather than filtering
                // buys the quarry only out of the spacing nobody was using.
                let floor = (MIN_SITE_SPACING as i64).pow(2);
                pool.iter()
                    .max_by_key(|&&(c, rock)| {
                        let apart = far_from(&sites, c);
                        (apart.min(floor), rock, apart)
                    })
                    .expect("the pool is not empty")
                    .0
            };
            sites.push(pick);
        }

        // Level a pad under each site last, so that the choice saw the terrain
        // as generated and every site ends up placeable regardless.
        for &(x, y) in &sites {
            self.level_pad(x, y, HEARTH_SIZE);
        }
        // And make sure every one of them can quarry. Ranking got the median
        // city to within thirty cells of rock, but a tenth were still sixty or
        // more away and the worst was a hundred — which is a city that cannot
        // build a dike, decided by the generator before anybody has played.
        for i in 0..sites.len() {
            self.ensure_rock_near(&sites, i);
        }
        self.hearth_sites = sites;
    }

    /// Put an outcrop within reach of a city that has none.
    ///
    /// The same kind of move as `level_pad`, and for the same reason: a seed
    /// that cannot be played is not a difficulty, it is a bug you shipped. The
    /// outcrop is small, is placed at the first spot a fixed search finds that
    /// is clear of every city and of the river, and is only ever added — no
    /// city loses rock it already had.
    fn ensure_rock_near(&mut self, sites: &[(i32, i32)], which: usize) {
        let (sx, sy) = sites[which];
        let rock = self.flood_fill(|g| g == Ground::Rock);
        if rock[Map::idx(sx, sy)] <= QUARRY_REACH {
            return;
        }
        let clear_of_cities = |x: i32, y: i32| {
            sites.iter().all(|&(cx, cy)| (x - cx).abs().max((y - cy).abs()) > HEARTH_SIZE + 3)
        };

        for ring in 8..=QUARRY_REACH {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let (x, y) = (sx + dx, sy + dy);
                    if x < 2 || y < 2 || x >= MAP_W - 2 || y >= MAP_H - 2 {
                        continue;
                    }
                    if !clear_of_cities(x, y) {
                        continue;
                    }
                    // A 3 x 3 of ordinary ground, so the outcrop neither
                    // dams the river nor swallows somebody's shoreline.
                    let room = (-1..=1).all(|jy| {
                        (-1..=1).all(|jx| self.ground[Map::idx(x + jx, y + jy)].buildable())
                    });
                    if !room {
                        continue;
                    }
                    for jy in -1..=1 {
                        for jx in -1..=1 {
                            let i = Map::idx(x + jx, y + jy);
                            self.ground[i] = Ground::Rock;
                            self.height[i] = self.height[i].max(self.rock_min);
                        }
                    }
                    return;
                }
            }
        }
    }

    /// Flatten a square to the centre cell's height and make it grass, so a
    /// Hearth always fits. Cities that start on a slope they cannot build on
    /// are not an interesting difficulty, just an unplayable seed.
    fn level_pad(&mut self, cx: i32, cy: i32, size: i32) {
        let h = self.height_at(cx, cy).max(self.shallows_max + 1);
        let half = size / 2;
        for y in (cy - half)..=(cy + half) {
            for x in (cx - half)..=(cx + half) {
                if Map::contains(x, y) {
                    let i = Map::idx(x, y);
                    self.height[i] = h;
                    self.ground[i] = Ground::Grass;
                }
            }
        }
    }
}

/// The channel's centreline, from its source on the high side to its mouth on
/// the low side.
///
/// Both mouths are on the map's edge and the two edges are opposite, so the
/// river cuts the map in two whatever the seed does with it. Which pair of
/// edges is a coin from the seed; where on each edge is drawn from the half
/// nearer the corner that end belongs to, so the channel always runs *down*
/// the ramp rather than along a contour. That is what keeps design §5's "high
/// ground is safe" true with a river on the map: the water comes out of the
/// high end and goes to the low one.
pub fn river_path(rng: &mut Rng, low: Corner, high: Corner) -> Vec<(i32, i32)> {
    let (lx, ly) = low.cell();
    let (hx, hy) = high.cell();
    let m = RIVER_MOUTH_MARGIN;

    // Somewhere in the half of an edge nearer a given end, `m` clear of the
    // corner so the channel never runs along an edge.
    fn half_near(near: i32, span: i32, m: i32, rng: &mut Rng) -> i32 {
        if near == 0 {
            rng.range(m, span / 2)
        } else {
            rng.range(span / 2, span - 1 - m)
        }
    }

    let (source, mouth) = if rng.chance(2) {
        // North to south, or south to north: the mouths are on the top and
        // bottom edges and the channel crosses every row.
        ((half_near(hx, MAP_W, m, rng), hy), (half_near(lx, MAP_W, m, rng), ly))
    } else {
        ((hx, half_near(hy, MAP_H, m, rng)), (lx, half_near(ly, MAP_H, m, rng)))
    };

    // Three to five bends, each pushed off the straight line by a seeded
    // amount perpendicular to it.
    let bends = rng.range(RIVER_BENDS.0, RIVER_BENDS.1);
    let (dx, dy) = (mouth.0 - source.0, mouth.1 - source.1);
    // The perpendicular, as a unit-ish direction. Integer: the run is long
    // enough that a taxicab perpendicular is indistinguishable from a real one
    // once the offsets are this large.
    let (px, py) = if dx.abs() >= dy.abs() { (0, 1) } else { (1, 0) };

    let mut points = vec![source];
    for i in 1..=bends {
        let t = i;
        let n = bends + 1;
        let off = rng.range(-RIVER_MEANDER, RIVER_MEANDER);
        points.push((
            (source.0 + dx * t / n + px * off).clamp(1, MAP_W - 2),
            (source.1 + dy * t / n + py * off).clamp(1, MAP_H - 2),
        ));
    }
    points.push(mouth);

    let mut path = Vec::new();
    for pair in points.windows(2) {
        let mut leg = walk(pair[0], pair[1]);
        if !path.is_empty() {
            leg.remove(0); // the joint belongs to one leg, not both
        }
        path.append(&mut leg);
    }
    path
}

/// Every cell on the straight line from `a` to `b`, `a` and `b` included.
///
/// A king's walk rather than Bresenham: it takes a diagonal step whenever both
/// axes still have ground to cover, which gives a line with no cell touching
/// only at a corner. That matters here because a channel with a corner-only
/// join is a channel a four-neighbour flood cannot get through.
fn walk(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x, mut y) = a;
    let mut out = vec![(x, y)];
    while (x, y) != b {
        if x != b.0 {
            x += (b.0 - x).signum();
        }
        if y != b.1 {
            y += (b.1 - y).signum();
        }
        out.push((x, y));
    }
    out
}

/// Cut the channel into the height field, and say which cells are its floor.
///
/// The bed is `RIVER_DEPTH` below the land, taken as a **running minimum** from
/// source to mouth so it only ever descends: a reach that went back uphill
/// would pond, and a river that ponds does not carry a wave. Outside the floor
/// the cut tapers back up over `RIVER_BANK` cells, and every write is a `min` —
/// a river never raises the ground it runs through.
///
/// Returns the floor cells, because `generate` has to paint them water and
/// cannot work that out from the height afterwards. **A river is water because
/// it is a river, not because it is low.** The ground bands are percentiles of
/// the height field, and a channel running down a ramp is above the waterline
/// for most of its length however deep it is cut — measured, in
/// `probe::what_the_river_costs`: three seeds in eight had two fifths of their
/// channel reading as dry land. So the floor is painted `Shallows` outright,
/// which is the one place in the generator besides `level_pad` where ground is
/// not a function of height, and it is deliberate in both.
pub fn carve_river(height: &mut [u8], path: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let reach = RIVER_HALF_WIDTH + RIVER_BANK;
    let rise = (RIVER_DEPTH / RIVER_BANK).max(1);
    let mut floor = Vec::with_capacity(path.len() * 9);
    // The bed, worked out for the whole channel before anything is cut, in
    // three passes: the land under it, that smoothed, and that made to
    // descend. Doing it cell by cell was the first version and it produced a
    // canyon — see `RIVER_SMOOTH`.
    let raw: Vec<i32> = path
        .iter()
        .map(|&(x, y)| {
            if Map::contains(x, y) { height[Map::idx(x, y)] as i32 } else { 0 }
        })
        .collect();
    let n_path = raw.len() as i32;
    let mut bedline: Vec<i32> = (0..n_path)
        .map(|i| {
            let lo = (i - RIVER_SMOOTH).max(0) as usize;
            let hi = (i + RIVER_SMOOTH).min(n_path - 1) as usize;
            let window = &raw[lo..=hi];
            let mean = window.iter().sum::<i32>() / window.len() as i32;
            (mean - RIVER_DEPTH).max(0)
        })
        .collect();
    // Down, never up: a reach that went back uphill would pond, and a river
    // that ponds does not carry a wave.
    for i in 1..bedline.len() {
        bedline[i] = bedline[i].min(bedline[i - 1]);
    }

    for (i, &(cx, cy)) in path.iter().enumerate() {
        if !Map::contains(cx, cy) {
            continue;
        }
        let bed = bedline[i];
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = (cx + dx, cy + dy);
                if !Map::contains(x, y) {
                    continue;
                }
                let d = dx.abs().max(dy.abs());
                let want = if d <= RIVER_HALF_WIDTH {
                    floor.push((x, y));
                    bed
                } else {
                    bed + (d - RIVER_HALF_WIDTH) * rise
                };
                let i = Map::idx(x, y);
                height[i] = height[i].min(want.clamp(0, 255) as u8);
            }
        }
    }
    floor
}

/// Raise a bar across one reach of the channel and return its cells.
///
/// Every map has exactly one, somewhere in the middle third of the river so it
/// is neither at a mouth nor always in the same place. It is the answer to the
/// obvious objection to putting a river through the middle of a game about
/// walking: the far bank is reachable on foot from the first day, slowly, and
/// a bridge is worth building the moment somebody can afford one.
/// It reaches the full width of the cut and not only the channel floor.
/// The bank taper is cut low too, and on a low-lying reach the bands make
/// those cells shallows as well — so a ford the width of the floor is a
/// crossing that stops two cells short of dry land on each side, which is to
/// say not a crossing at all. That was worth ten minutes and a flow field
/// that reached a third of the map.
pub fn place_ford(rng: &mut Rng, height: &mut [u8], path: &[(i32, i32)]) -> Vec<(i32, i32)> {
    if path.len() < 3 {
        return Vec::new();
    }
    let n = path.len() as i32;
    let reach = RIVER_HALF_WIDTH + RIVER_BANK;
    let start = rng.range(n / 3, (2 * n / 3 - FORD_LENGTH).max(n / 3));
    let mut cells = Vec::new();

    for i in start..(start + FORD_LENGTH).min(n) {
        let (cx, cy) = path[i as usize];
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = (cx + dx, cy + dy);
                if !Map::contains(x, y) {
                    continue;
                }
                let i = Map::idx(x, y);
                height[i] = height[i].saturating_add(FORD_RISE as u8);
                cells.push((x, y));
            }
        }
    }
    cells
}

/// The height field: a corner-to-corner ramp with signed value noise on top.
///
/// `amplitude` is a parameter rather than a constant read from `balance` so
/// that the tests can sweep it. It is the number that decides whether the map
/// reads as "a valley and a hill" or as "noise that happens to slope", and
/// picking it by eye on one seed is how the first two versions of this
/// function ended up wrong.
pub fn terrain(rng: &mut Rng, low_corner: Corner, amplitude: i32, slope_span: i32) -> Vec<u8> {
    // Four octaves, coarse to fine. The coarse one is the shape of the land
    // and the fine ones are the texture on it.
    let octaves = [(64i32, 8i32), (32, 4), (16, 2), (8, 1)];
    let lattices: Vec<Vec<u8>> = octaves.iter().map(|&(stride, _)| lattice(rng, stride)).collect();
    let total_weight: i32 = octaves.iter().map(|&(_, w)| w).sum();

    let (lx, ly) = low_corner.cell();
    let (hx, hy) = low_corner.opposite().cell();

    let mut height = vec![0u8; CELLS];
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            // Centred, so the noise is an offset from the ramp rather than a
            // value averaged with it. Averaging was the first attempt and it
            // pulled every map into the middle of the height scale.
            let mut noise = 0i32;
            for (i, &(stride, weight)) in octaves.iter().enumerate() {
                noise += (value_noise(&lattices[i], stride, x, y) - 128) * weight;
            }
            noise = noise * amplitude / (total_weight * 128);

            // Distance along the low-to-high diagonal, as 0..=SLOPE_SPAN.
            // Taxicab rather than Euclidean: it needs no square root, the
            // denominator is then constant across the whole map, and the
            // difference is a slightly diamond-shaped contour that nobody will
            // ever see on terrain this rough.
            let d_low = (x - lx).abs() + (y - ly).abs();
            let d_high = (x - hx).abs() + (y - hy).abs();
            let span = d_low + d_high; // constant along the diagonal
            let slope = if span == 0 { slope_span / 2 } else { d_low * slope_span / span };

            height[Map::idx(x, y)] = (slope + noise).clamp(0, 255) as u8;
        }
    }
    height
}

/// The height at which each ground band starts, so that roughly the intended
/// fraction of the map is each type whatever the noise did.
///
/// A 256-bucket histogram rather than a sort: the heights are already `u8`, so
/// counting them is one pass and finding the cut points is one more. No
/// allocation, no comparison sort, and — the part that matters — no float in
/// sight.
fn band_heights(height: &[u8]) -> (u8, u8, u8) {
    let mut hist = [0u32; 256];
    for &h in height {
        hist[h as usize] += 1;
    }
    let total = height.len() as u32;

    // The cut whose cumulative count is *closest* to the target, rather than
    // the first one at or past it. Heights are integers and the distribution
    // is peaked, so a single height can hold several percent of the map;
    // taking the first bucket over the line overshot the rock band by half
    // again on some seeds. Closest-cut keeps the error to half a bucket.
    let quantile = |percent: i32| -> u8 {
        let want = total as i64 * percent as i64 / 100;
        let mut seen = 0i64;
        let mut best = (i64::MAX, 0u8);
        for (h, &n) in hist.iter().enumerate() {
            seen += n as i64;
            let miss = (seen - want).abs();
            if miss < best.0 {
                best = (miss, h as u8);
            }
        }
        best.1
    };

    let shallows_max = quantile(SHALLOWS_PERCENT);
    // Saturating, so that a map flat enough to put two cut points on the same
    // height still yields bands in the right order rather than an empty one.
    let sand_max = quantile(SHALLOWS_PERCENT + SAND_PERCENT).max(shallows_max);
    let rock_min = quantile(100 - ROCK_PERCENT).max(sand_max + 1);
    (shallows_max, sand_max, rock_min)
}

/// A grid of random values at `stride` intervals, with one extra row and
/// column so interpolating the last cell has a corner to reach for.
fn lattice(rng: &mut Rng, stride: i32) -> Vec<u8> {
    let n = (MAP_W / stride + 2) as usize;
    (0..n * n).map(|_| rng.below(256) as u8).collect()
}

/// Bilinear value noise with a smoothstep weight, all integer.
fn value_noise(lat: &[u8], stride: i32, x: i32, y: i32) -> i32 {
    let n = (MAP_W / stride + 2) as usize;
    let (gx, gy) = (x / stride, y / stride);
    let at = |ix: i32, iy: i32| lat[iy as usize * n + ix as usize] as i32;

    let a = at(gx, gy);
    let b = at(gx + 1, gy);
    let c = at(gx, gy + 1);
    let d = at(gx + 1, gy + 1);

    let sx = smooth((x % stride) * 256 / stride);
    let sy = smooth((y % stride) * 256 / stride);

    let top = a + ((b - a) * sx >> 8);
    let bottom = c + ((d - c) * sx >> 8);
    top + ((bottom - top) * sy >> 8)
}

/// `t^2 * (3 - 2t)` on a 0..=256 scale. Straight linear interpolation between
/// lattice points leaves visible creases along every lattice line; this is the
/// cheapest curve that removes them, and it is exact in integers — at t = 256
/// it is 256, so the noise passes through its lattice values.
fn smooth(t: i32) -> i32 {
    (t * t * (768 - 2 * t)) >> 16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(seed: u64, players: u32) -> Map {
        Map::generate(&mut Rng::new(seed), players)
    }

    /// Mean height of the 16 x 16 block at a corner.
    fn corner_mean(m: &Map, c: Corner) -> i32 {
        let (cx, cy) = c.cell();
        let (sx, sy) = (cx.min(MAP_W - 16), cy.min(MAP_H - 16));
        let mut total = 0i32;
        for y in sy..sy + 16 {
            for x in sx..sx + 16 {
                total += m.height_at(x, y) as i32;
            }
        }
        total / 256
    }

    #[test]
    fn the_same_seed_makes_the_same_map() {
        for seed in [1u64, 2, 99, 0xDEAD_BEEF, u64::MAX] {
            let a = gen(seed, 4);
            let b = gen(seed, 4);
            assert_eq!(a, b, "seed {seed} generated two different maps");
        }
    }

    #[test]
    fn different_seeds_make_different_maps() {
        assert_ne!(gen(1, 2).height, gen(2, 2).height);
    }

    #[test]
    fn the_player_count_changes_nothing_but_the_sites() {
        // The terrain is drawn before the sites are chosen, so two runs on one
        // seed with different player counts share a landscape. That is worth
        // pinning: it is what lets a seed be described as "that map" rather
        // than "that map, at four players".
        let two = gen(77, 2);
        let six = gen(77, 6);
        assert_eq!(two.low_corner, six.low_corner);
        assert_eq!(two.hearth_sites.len(), 2);
        assert_eq!(six.hearth_sites.len(), 6);
    }

    #[test]
    fn every_map_has_a_low_corner_that_is_actually_low() {
        for seed in 0..60u64 {
            let m = gen(seed, 3);
            let low = corner_mean(&m, m.low_corner);
            let high = corner_mean(&m, m.high_corner);
            assert!(
                // A fraction of the relief rather than a fixed number: the
                // height scale is set by what a dike and a surge are worth
                // (see balance::SLOPE_SPAN) and has already changed once.
                low + SLOPE_SPAN / 4 < high,
                "seed {seed}: low corner {:?} averaged {low}, high corner {:?} averaged {high}",
                m.low_corner,
                m.high_corner
            );
            assert_eq!(m.high_corner, m.low_corner.opposite());
        }
    }

    /// Every seed is a playable map, not a lottery.
    ///
    /// This is the test that caught the first two attempts at the terrain:
    /// blending the ramp with the noise gave seed 0 thirty shallow cells and
    /// seed 7 a map with no rock on it at all. Quantile bands make the
    /// composition a property of the generator instead of a property of the
    /// draw, so the bounds here can be tight.
    #[test]
    fn every_map_has_its_shallows_and_its_rock() {
        for seed in 0..60u64 {
            let m = gen(seed, 2);
            let count = |g: Ground| m.ground.iter().filter(|&&x| x == g).count();
            let pct = |g: Ground| (count(g) * 100 / CELLS) as i32;

            // Bands, not exact percentages. Forty distinct heights make the
            // histogram the quantile walks coarse enough that one height can
            // hold several per cent of the map, so the cut cannot land where
            // it is aimed. What must hold is the thing the fixed-height
            // thresholds got wrong: every seed has a river to bridge and rock
            // to build round, rather than one seed in ten having neither.
            assert!(
                (5..=25).contains(&pct(Ground::Shallows)),
                "seed {seed}: {}% shallows, wanted roughly {SHALLOWS_PERCENT}%",
                pct(Ground::Shallows)
            );
            assert!(
                (3..=20).contains(&pct(Ground::Rock)),
                "seed {seed}: {}% rock, wanted roughly {ROCK_PERCENT}%",
                pct(Ground::Rock)
            );
            assert!(
                count(Ground::Grass) > CELLS / 2,
                "seed {seed}: only {} grass cells to build on",
                count(Ground::Grass)
            );
            // The bands have to stay in order even on a map flat enough for
            // two cut points to land on one height.
            assert!(m.shallows_max <= m.sand_max && m.sand_max < m.rock_min);
        }
    }

    #[test]
    fn every_map_has_a_river_that_cuts_it_in_two() {
        // M4's map guarantees, all in one place because they are one claim:
        // the river is a river, it divides the map, it is crossable, and it
        // did not eat the game.
        for players in 2..=6u32 {
            for seed in 0..80u64 {
                let m = gen(seed, players);
                let river: Vec<(i32, i32)> =
                    m.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
                assert!(river.len() > MAP_W as usize / 2, "seed {seed}: a stub of a river");

                // Both ends on an edge, and on opposite edges, so it cannot be
                // walked round.
                let on_edge = |&(x, y): &(i32, i32)| {
                    x == 0 || y == 0 || x == MAP_W - 1 || y == MAP_H - 1
                };
                let (a, b) = (river[0], river[river.len() - 1]);
                assert!(on_edge(&a) && on_edge(&b), "seed {seed}: {a:?} to {b:?} is not a river");
                let same_edge = (a.0 == b.0 && (a.0 == 0 || a.0 == MAP_W - 1))
                    || (a.1 == b.1 && (a.1 == 0 || a.1 == MAP_H - 1));
                assert!(!same_edge, "seed {seed}: both mouths on the same edge");

                // Every step is a king's step, so a four-neighbour flood can
                // run down it without falling out at a corner.
                for pair in river.windows(2) {
                    let (dx, dy) = (pair[1].0 - pair[0].0, pair[1].1 - pair[0].1);
                    assert!(
                        dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0),
                        "seed {seed}: the channel jumps from {:?} to {:?}",
                        pair[0],
                        pair[1]
                    );
                }

                // It is water, all the way along.
                for &(x, y) in &river {
                    assert!(
                        m.ground_at(x, y).watery(),
                        "seed {seed}: the channel is dry at ({x},{y})"
                    );
                }

                // And there is somewhere you can wade it.
                let fords = m.ground.iter().filter(|&&g| g == Ground::Ford).count();
                assert!(fords > 0, "seed {seed}: no ford, so half the map is unreachable");

                // Nobody starts in it, and the high ground is still dry.
                for &(x, y) in &m.hearth_sites {
                    assert!(!m.ground_at(x, y).watery(), "seed {seed}: a hearth is in the river");
                }
                let (hx, hy) = m.high_corner.cell();
                assert!(
                    !m.ground_at(hx, hy).watery(),
                    "seed {seed}: the high corner is under water"
                );
            }
        }
    }

    #[test]
    fn the_ford_joins_the_two_banks() {
        // The one thing the ford is for. Without it the river is a wall and
        // half the map is somewhere nobody can walk to, which is a different
        // game from the one design section 6 describes.
        use crate::nav::passable;
        for seed in 0..30u64 {
            let w = crate::world::World::new(seed, 2);
            let start = w.map.hearth_sites[0];
            let mut seen = vec![false; CELLS];
            let mut queue = std::collections::VecDeque::new();

            // Out of the hearth first: a citizen starts inside a building.
            for dy in -3..=3i32 {
                for dx in -3..=3i32 {
                    let (x, y) = (start.0 + dx, start.1 + dy);
                    if Map::contains(x, y) && passable(&w, x, y) && !seen[Map::idx(x, y)] {
                        seen[Map::idx(x, y)] = true;
                        queue.push_back((x, y));
                    }
                }
            }
            while let Some((x, y)) = queue.pop_front() {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (x + dx, y + dy);
                        if !Map::contains(nx, ny) || seen[Map::idx(nx, ny)] {
                            continue;
                        }
                        if passable(&w, nx, ny) {
                            seen[Map::idx(nx, ny)] = true;
                            queue.push_back((nx, ny));
                        }
                    }
                }
            }

            // The other city is on the far bank, and can be walked to.
            let far = w.map.hearth_sites[1];
            let reachable = (-3..=3i32).any(|dy| {
                (-3..=3i32).any(|dx| {
                    let (x, y) = (far.0 + dx, far.1 + dy);
                    Map::contains(x, y) && seen[Map::idx(x, y)]
                })
            });
            assert!(reachable, "seed {seed}: no way to walk from one city to the other");
        }
    }

    #[test]
    fn a_ford_under_water_is_not_a_ford() {
        // Design's own point, and the tutorial says it once: the crossing you
        // have been relying on goes under on the impact day.
        use crate::nav::passable;
        let mut w = crate::world::World::new(31, 2);
        let ford = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| w.map.ground_at(x, y) == Ground::Ford)
            .expect("every map has a ford");

        assert!(passable(&w, ford.0, ford.1), "a dry ford cannot be waded");
        w.water.raise_to(ford.0, ford.1, WADE_DEPTH);
        assert!(!passable(&w, ford.0, ford.1), "a flooded ford is still a crossing");
    }

    #[test]
    fn the_wet_ground_is_at_the_low_end() {
        // Quantile bands cut by height, so this is close to a tautology — but
        // it is the tautology the flood depends on, and if the ramp ever
        // stopped deciding which end is low it would stop being one.
        //
        // The river is excluded, and has to be: it is water all the way from
        // the high end to the low one by construction, so counting it would
        // measure the channel rather than the coast. What is under test is
        // that the *land* is still wetter at the low end, which is what makes
        // high ground safe.
        for seed in 0..20u64 {
            let m = gen(seed, 2);
            let river: std::collections::BTreeSet<(i32, i32)> = {
                let mut set = std::collections::BTreeSet::new();
                let reach = RIVER_HALF_WIDTH + RIVER_BANK;
                for &(rx, ry) in &m.river {
                    for dy in -reach..=reach {
                        for dx in -reach..=reach {
                            set.insert((rx as i32 + dx, ry as i32 + dy));
                        }
                    }
                }
                set
            };
            let (lx, ly) = m.low_corner.cell();
            let (hx, hy) = m.high_corner.cell();
            let near = |cx: i32, cy: i32| {
                let mut n = 0;
                for y in 0..MAP_H {
                    for x in 0..MAP_W {
                        if (x - cx).abs() + (y - cy).abs() < 40
                            && m.ground_at(x, y).watery()
                            && !river.contains(&(x, y))
                        {
                            n += 1;
                        }
                    }
                }
                n
            };
            assert!(
                near(lx, ly) > near(hx, hy),
                "seed {seed}: the low corner is not the wet one"
            );
        }
    }

    /// The guarantee the plan asks for, checked at every player count over
    /// enough seeds that a ring geometry mistake cannot hide in one of them.
    #[test]
    fn sites_are_far_enough_apart() {
        let mut worst = std::collections::BTreeMap::new();
        for players in 2..=6u32 {
            for seed in 0..200u64 {
                let m = gen(seed, players);
                assert_eq!(m.hearth_sites.len(), players as usize);
                for (i, &(ax, ay)) in m.hearth_sites.iter().enumerate() {
                    for &(bx, by) in &m.hearth_sites[i + 1..] {
                        let d2 = (ax - bx).pow(2) + (ay - by).pow(2);
                        let e = worst.entry(players).or_insert(i32::MAX);
                        *e = (*e).min(crate::fx::isqrt(d2 as i64) as i32);
                        let min = MIN_SITE_SPACING * MIN_SITE_SPACING;
                        assert!(
                            d2 >= min,
                            "{players}p seed {seed}: ({ax},{ay}) and ({bx},{by}) are \
                             {} apart, wanted {MIN_SITE_SPACING}",
                            crate::fx::isqrt(d2 as i64)
                        );
                    }
                }
            }
        }
        // Printed with --nocapture: the table in `MIN_SITE_SPACING` is the
        // arithmetic before jitter, and this is what it comes to in practice.
        println!("closest two cities, by player count: {worst:?}");
    }

    #[test]
    fn sites_leave_room_for_their_hearth() {
        let margin = HEARTH_SIZE / 2;
        for players in 2..=6u32 {
            for seed in 0..100u64 {
                let m = gen(seed, players);
                for &(x, y) in &m.hearth_sites {
                    assert!(
                        x - margin >= 0 && y - margin >= 0
                            && x + margin < MAP_W && y + margin < MAP_H,
                        "{players}p seed {seed}: site ({x},{y}) hangs off the map"
                    );
                }
            }
        }
    }

    #[test]
    fn a_hearth_fits_on_every_site() {
        let half = HEARTH_SIZE / 2;
        for players in 2..=6u32 {
            for seed in 0..100u64 {
                let m = gen(seed, players);
                for &(cx, cy) in &m.hearth_sites {
                    let h = m.height_at(cx, cy);
                    for y in cy - half..=cy + half {
                        for x in cx - half..=cx + half {
                            assert!(
                                m.buildable(x, y),
                                "{players}p seed {seed}: ({x},{y}) under site \
                                 ({cx},{cy}) is {:?}",
                                m.ground_at(x, y)
                            );
                            assert_eq!(
                                m.height_at(x, y),
                                h,
                                "{players}p seed {seed}: pad under ({cx},{cy}) is not level"
                            );
                        }
                    }
                    assert!(h > m.shallows_max, "a site was left in the shallows");
                }
            }
        }
    }

    #[test]
    fn the_river_is_comparably_near_to_every_city() {
        let mut spreads = std::collections::BTreeSet::new();
        // Design §6: "comparable (not identical)" ground.
        //
        // **This used to measure the distance to the low corner**, because the
        // flood used to come out of one. It read the other way round before
        // that — asserting the spread was *more* than forty cells — and it
        // passed, and that was the problem: a ring around the map centre gave
        // a spread of a hundred, which is not "not identical", it is one
        // player drowned in age one and another who never sees water.
        //
        // The water comes down a channel now, so the distance that decides a
        // city's game is its distance to the bank. Nobody is the same distance
        // out as anybody else, and nobody is in a different game.
        for players in 2..=6u32 {
            for seed in 0..60u64 {
                let m = gen(seed, players);
                let mut d: Vec<i32> = m
                    .hearth_sites
                    .iter()
                    .map(|&(x, y)| {
                        m.river
                            .iter()
                            .map(|&(rx, ry)| {
                                (x - rx as i32).abs().max((y - ry as i32).abs())
                            })
                            .min()
                            .expect("every map has a river")
                    })
                    .collect();
                d.sort_unstable();
                let spread = d[d.len() - 1] - d[0];
                spreads.insert(spread);
                assert!(
                    spread <= 2 * SITE_JITTER_BAND,
                    "{players}p seed {seed}: one city is in a different game: {d:?}"
                );
                assert!(
                    d[0] >= SHORE_DISTANCE - SITE_JITTER_BAND
                        && d[d.len() - 1] <= SHORE_DISTANCE + SITE_JITTER_BAND,
                    "{players}p seed {seed}: a city is off the band: {d:?}"
                );
                // Nobody starts in the water, and nobody starts unable to
                // reach it — the flood is the game.
                for &(x, y) in &m.hearth_sites {
                    assert!(!m.ground_at(x, y).watery(), "a hearth is in the river");
                }
            }
        }
        // And "not identical" across the run of seeds: the band and the
        // farthest-point choice do move cities relative to the water.
        assert!(spreads.len() > 3, "every map is laid out the same: {spreads:?}");
    }

    #[test]
    fn the_rotation_moves_the_cities_between_seeds() {
        // The shore parallel runs from whichever corner the water comes out
        // of, and the jitter moves each site along and across it, so city 0 is
        // somewhere different on every seed. Without that the seed would be
        // much less interesting than it looks.
        let firsts: Vec<(i32, i32)> = (0..20u64).map(|s| gen(s, 4).hearth_sites[0]).collect();
        let distinct = {
            let mut v = firsts.clone();
            v.sort_unstable();
            v.dedup();
            v.len()
        };
        assert!(distinct > 12, "city 0 landed in only {distinct} distinct spots: {firsts:?}");
    }

    #[test]
    fn out_of_bounds_reads_as_impassable_rock() {
        let m = gen(1, 2);
        assert_eq!(m.ground_at(-1, 5), Ground::Rock);
        assert_eq!(m.ground_at(MAP_W, 5), Ground::Rock);
        assert_eq!(m.height_at(5, -1), 255);
        assert!(!m.buildable(-1, -1));
    }

    #[test]
    fn the_noise_passes_through_its_lattice_points() {
        // smooth(0) == 0 and smooth(256) == 256, so a cell sitting exactly on
        // a lattice corner takes that corner's value. If this drifts, the
        // terrain grows creases along every lattice line.
        assert_eq!(smooth(0), 0);
        assert_eq!(smooth(256), 256);
        assert_eq!(smooth(128), 128);
        for t in 0..=256 {
            let s = smooth(t);
            assert!((0..=256).contains(&s), "smooth({t}) = {s}");
        }
    }

    #[test]
    fn clamping_the_player_count_does_not_panic() {
        assert_eq!(gen(1, 0).hearth_sites.len(), 2);
        assert_eq!(gen(1, 99).hearth_sites.len(), 6);
    }
}



#[cfg(test)]
mod probe {
    use super::*;

    /// Is there anywhere higher a citizen could actually walk to?
    ///
    /// M12.9. Both M11.9 players said the same thing in different words: every
    /// cell either of them could reach read 16 to 25, so the height readout
    /// M11.3 added had nothing to say, and design §3.2's *"get uphill"* is an
    /// order neither could obey. City 1: *"my whole reachable world is 16-18.
    /// On terrain with relief this would be the best verb in the game. Here it
    /// only told me I had no options."*
    ///
    /// **"16 to 25" is a claim from two runs, not a measurement.** This is the
    /// measurement, and it is the point of the milestone: for each hearth on
    /// ten seeds, the spread of terrain height within `QUARRY_REACH` cells -
    /// which is the furthest this game ever asks anybody to walk for anything.
    ///
    /// **What it arranges**: nothing. It reads generated maps and adds up.
    #[test]
    #[ignore]
    fn how_much_higher_a_citizen_can_get() {
        println!();
        println!("  seed          city   at the hearth   highest within reach   climb   cells above +2");
        let mut climbs: Vec<i32> = Vec::new();
        for seed in [31u64, 1_000_003, 0xF100_D11E, 7, 99, 12345, 2024, 555_555, 8_675_309, 42] {
            let m = Map::generate(&mut Rng::new(seed), 2);
            for (n, &(hx, hy)) in m.hearth_sites.iter().enumerate() {
                let base = m.height_at(hx, hy) as i32;
                let mut highest = base;
                let mut above = 0;
                for dy in -QUARRY_REACH..=QUARRY_REACH {
                    for dx in -QUARRY_REACH..=QUARRY_REACH {
                        if dx * dx + dy * dy > QUARRY_REACH * QUARRY_REACH {
                            continue;
                        }
                        let (x, y) = (hx + dx, hy + dy);
                        if !Map::contains(x, y) {
                            continue;
                        }
                        // Rock cannot be built on, but it can be stood on, and
                        // standing on it is the whole of "get uphill".
                        let h = m.height_at(x, y) as i32;
                        highest = highest.max(h);
                        if h >= base + 2 {
                            above += 1;
                        }
                    }
                }
                let climb = highest - base;
                climbs.push(climb);
                println!(
                    "  {seed:<12}  {n:>4}   {base:>13}   {highest:>20}   {climb:>5}   {above:>14}"
                );
            }
        }
        climbs.sort_unstable();
        let med = climbs[climbs.len() / 2];
        println!();
        println!(
            "  climb available: worst {}, median {}, best {}",
            climbs[0],
            med,
            climbs[climbs.len() - 1]
        );
        println!(
            "  a surge of 12 stands {} terrain units deep at the bank; SWIM_DEPTH is {} sixteenths",
            12,
            crate::balance::SWIM_DEPTH
        );
    }

    /// Where the cities end up relative to the channel, and how far apart.
    #[test]
    #[ignore]
    fn where_the_cities_sit() {
        println!();
        println!("  players   closest pair   nearest bank   furthest bank   in the river   worst rock   no quarry   med p90   maybe-planted   headroom lo/med/hi");
        for players in 2..=6u32 {
            let mut closest = i32::MAX;
            let (mut near, mut far) = (i32::MAX, 0);
            let mut in_river = 0;
            // How far a city has to walk to the nearest rock, which is the
            // only thing a quarry may be built beside — and a quarry is the
            // only source of the stone a dike costs.
            let mut worst_rock = 0;
            let mut no_rock = 0;
            let mut rocks: Vec<i32> = Vec::new();
            let mut heads: Vec<i32> = Vec::new();
            let mut nudged = 0;
            for seed in 0..200u64 {
                let m = Map::generate(&mut Rng::new(seed), players);
                for (i, &(ax, ay)) in m.hearth_sites.iter().enumerate() {
                    for &(bx, by) in &m.hearth_sites[i + 1..] {
                        let d2 = ((ax - bx).pow(2) + (ay - by).pow(2)) as i64;
                        closest = closest.min(crate::fx::isqrt(d2) as i32);
                    }
                    // Chebyshev to the nearest centreline cell, which is how
                    // far the site is from the water.
                    let d = m
                        .river
                        .iter()
                        .map(|&(rx, ry)| {
                            (ax - rx as i32).abs().max((ay - ry as i32).abs())
                        })
                        .min()
                        .unwrap_or(0);
                    near = near.min(d);
                    far = far.max(d);
                    if m.ground_at(ax, ay).watery() {
                        in_river += 1;
                    }
                    let rock = (0..MAP_H)
                        .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
                        .filter(|&(x, y)| m.ground_at(x, y) == Ground::Rock)
                        .map(|(x, y)| (ax - x).abs().max((ay - y).abs()))
                        .min()
                        .unwrap_or(999);
                    if rock > 40 {
                        no_rock += 1;
                    }
                    worst_rock = worst_rock.max(rock);
                    rocks.push(rock);
                    // How far the site stands above the river bed it is
                    // nearest to: the flood has to climb this to reach it.
                    let (rx, ry) = m
                        .river
                        .iter()
                        .map(|&(a, b)| (a as i32, b as i32))
                        .min_by_key(|&(a, b)| (ax - a).abs().max((ay - b).abs()))
                        .unwrap();
                    heads.push(m.height_at(ax, ay) as i32 - m.height_at(rx, ry) as i32);
                    // The outcrop is planted at ring 8 or just outside it, so
                    // a city with rock closer than that had its own.
                    if rock >= 8 && rock <= QUARRY_REACH {
                        nudged += 1;
                    }
                }
            }
            rocks.sort_unstable();
            heads.sort_unstable();
            println!(
                "  {players:>7}   {closest:>12}   {near:>12}   {far:>13}   {in_river:>12}   {worst_rock:>10}   {no_rock:>8}   {:>6}   {:>3}   {nudged:>6}   {:>3} {:>3} {:>3}",
                rocks[rocks.len() / 2],
                rocks[rocks.len() * 9 / 10],
                heads[0],
                heads[heads.len() / 2],
                heads[heads.len() - 1],
            );
        }
    }

    /// What the channel does to the map it is cut into: how much of the map
    /// it is, how much of it the ground bands would have made water on their
    /// own, and how much coast is left once it has taken its share.
    #[test]
    #[ignore]
    fn what_the_river_costs() {
        println!();
        println!("  seed   len   cells   %map   shallows_max   in band   coast %   dry high corner");
        let mut worst_in_band = 100;
        for seed in [3u64, 31, 97, 1000003, 4043362590, 7, 12345, 88888] {
            let m = Map::generate(&mut Rng::new(seed), 4);
            let river: Vec<(i32, i32)> =
                m.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();

            // Every cell the channel floor covers, not just the centreline.
            let mut floor = std::collections::BTreeSet::new();
            for &(cx, cy) in &river {
                for dy in -RIVER_HALF_WIDTH..=RIVER_HALF_WIDTH {
                    for dx in -RIVER_HALF_WIDTH..=RIVER_HALF_WIDTH {
                        if Map::contains(cx + dx, cy + dy) {
                            floor.insert((cx + dx, cy + dy));
                        }
                    }
                }
            }
            // What the ground bands alone would have made of the channel —
            // the number that says why the floor is painted rather than left
            // to the percentile. Every floor cell is water either way.
            let wet = floor
                .iter()
                .filter(|&&(x, y)| m.height_at(x, y) <= m.shallows_max)
                .count();
            let in_band = wet * 100 / floor.len();
            if let Some(&(bx, by)) = floor.iter().find(|&&(x, y)| !m.ground_at(x, y).watery()) {
                panic!(
                    "seed {seed}: the channel is {:?} at ({bx},{by}); sites {:?}",
                    m.ground_at(bx, by),
                    m.hearth_sites
                );
            }
            worst_in_band = worst_in_band.min(in_band);

            let all_shallows =
                m.ground.iter().filter(|&&g| g == Ground::Shallows).count();
            let coast = (all_shallows - wet) * 100 / CELLS;
            let (hx, hy) = m.high_corner.cell();
            println!(
                "  {seed:>10}   {:>3}   {:>5}   {:>4}   {:>12}   {in_band:>6}%   {coast:>6}%   {}",
                river.len(),
                floor.len(),
                floor.len() * 100 / CELLS,
                m.shallows_max,
                m.ground_at(hx, hy) != Ground::Shallows,
            );
        }
        println!();
        println!("  at worst {worst_in_band}% of a channel floor would have been water by height alone");
    }

    /// Not a test — a measurement, run by hand with
    /// `cargo test -p sim probe -- --ignored --nocapture` when the terrain
    /// constants need choosing again.
    #[test]
    #[ignore]
    fn sweep_noise_amplitude() {
        for amp in [SLOPE_SPAN / 8, SLOPE_SPAN / 5, SLOPE_SPAN * 3 / 10, SLOPE_SPAN * 2 / 5, SLOPE_SPAN / 2] {
            let mut wrong_corner = 0;
            let mut worst_ramp = i32::MAX;
            for seed in 0..300u64 {
                let mut rng = Rng::new(seed);
                let low = Corner::ALL[rng.below(4) as usize];
                let h = terrain(&mut rng, low, amp, SLOPE_SPAN);
                let (sh, _, _) = band_heights(&h);

                let (lx, ly) = low.cell();
                let (hx, hy) = low.opposite().cell();
                let wet = |cx: i32, cy: i32| {
                    let mut n = 0;
                    for y in 0..MAP_H {
                        for x in 0..MAP_W {
                            if (x - cx).abs() + (y - cy).abs() < 40
                                && h[Map::idx(x, y)] <= sh
                            {
                                n += 1;
                            }
                        }
                    }
                    n
                };
                if wet(lx, ly) <= wet(hx, hy) {
                    wrong_corner += 1;
                }
                let mean = |cx: i32, cy: i32| {
                    let (sx, sy) = (cx.min(MAP_W - 16), cy.min(MAP_H - 16));
                    let mut t = 0i32;
                    for y in sy..sy + 16 {
                        for x in sx..sx + 16 {
                            t += h[Map::idx(x, y)] as i32;
                        }
                    }
                    t / 256
                };
                worst_ramp = worst_ramp.min(mean(hx, hy) - mean(lx, ly));
            }
            println!(
                "amplitude {amp:3}: wrong wet corner {wrong_corner}/300, \
                 smallest corner-to-corner drop {worst_ramp}"
            );
        }
    }
}
