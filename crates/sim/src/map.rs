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
    Sand,
    Grass,
    Rock,
}

impl Ground {
    /// Whether a building may stand here. Shallows need a bridge and rock
    /// cannot be dug, so both are out.
    pub fn buildable(self) -> bool {
        matches!(self, Ground::Grass | Ground::Sand)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
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
        let height = terrain(rng, low_corner, NOISE_AMPLITUDE);

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
            hearth_sites: Vec::new(),
        };
        map.place_hearth_sites(rng, players);
        map
    }

    /// One site per player, on a ring around the centre.
    ///
    /// A ring rather than rejection sampling because the plan asks for a hard
    /// guarantee — "sites at least 40 cells apart" — and rejection sampling
    /// can only offer one in practice: at six players on a map whose
    /// buildable ground depends on the seed, it will eventually fail to find a
    /// sixth spot and have to relax the very constraint it exists to keep. A
    /// ring gives the spacing by construction. The whole ring is rotated by a
    /// random amount, which changes which city is nearest the low corner
    /// without changing any distance between two cities.
    fn place_hearth_sites(&mut self, rng: &mut Rng, players: u32) {
        use crate::fx::{cos, sin, Fx};

        let rot = rng.below(256) as u8;
        let centre = (MAP_W / 2, MAP_H / 2);
        let r = Fx::cells(SITE_RING_RADIUS);

        let mut sites = Vec::with_capacity(players as usize);
        for i in 0..players {
            let angle = rot.wrapping_add(((i * 256) / players) as u8);
            let x = centre.0 + (r * cos(angle)).round();
            let y = centre.1 + (r * sin(angle)).round();

            // Jitter, so two runs with the same player count are not the same
            // six spots rotated. Bounded, because the spacing sum depends on
            // it — see SITE_RING_RADIUS.
            let x = x + rng.range(-SITE_JITTER, SITE_JITTER);
            let y = y + rng.range(-SITE_JITTER, SITE_JITTER);

            let (x, y) = self.snap_to_buildable(x, y);
            sites.push((x, y));
        }

        // Level a pad under each site last, so that snapping saw the terrain
        // as generated and every site ends up placeable regardless.
        for &(x, y) in &sites {
            self.level_pad(x, y, HEARTH_SIZE);
        }
        self.hearth_sites = sites;
    }

    /// The nearest buildable cell within `SITE_SNAP`, searched in a fixed
    /// order so the answer does not depend on anything but the map. Falls back
    /// to the cell asked for, which `level_pad` then makes buildable.
    fn snap_to_buildable(&self, x: i32, y: i32) -> (i32, i32) {
        if self.buildable(x, y) {
            return (x, y);
        }
        for ring in 1..=SITE_SNAP {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    // Only the cells this ring adds, not the ones inside it.
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if self.buildable(nx, ny) {
                        return (nx, ny);
                    }
                }
            }
        }
        (x, y)
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

/// The height field: a corner-to-corner ramp with signed value noise on top.
///
/// `amplitude` is a parameter rather than a constant read from `balance` so
/// that the tests can sweep it. It is the number that decides whether the map
/// reads as "a valley and a hill" or as "noise that happens to slope", and
/// picking it by eye on one seed is how the first two versions of this
/// function ended up wrong.
fn terrain(rng: &mut Rng, low_corner: Corner, amplitude: i32) -> Vec<u8> {
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
            let slope = if span == 0 { SLOPE_SPAN / 2 } else { d_low * SLOPE_SPAN / span };

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
                low + 80 < high,
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

            assert!(
                (SHALLOWS_PERCENT - 3..=SHALLOWS_PERCENT + 3).contains(&pct(Ground::Shallows)),
                "seed {seed}: {}% shallows, wanted about {SHALLOWS_PERCENT}%",
                pct(Ground::Shallows)
            );
            assert!(
                (ROCK_PERCENT - 3..=ROCK_PERCENT + 3).contains(&pct(Ground::Rock)),
                "seed {seed}: {}% rock, wanted about {ROCK_PERCENT}%",
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
    fn the_wet_ground_is_at_the_low_end() {
        // Quantile bands cut by height, so this is close to a tautology — but
        // it is the tautology the flood depends on, and if the ramp ever
        // stopped deciding which end is low it would stop being one.
        for seed in 0..20u64 {
            let m = gen(seed, 2);
            let (lx, ly) = m.low_corner.cell();
            let (hx, hy) = m.high_corner.cell();
            let near = |cx: i32, cy: i32, g: Ground| {
                let mut n = 0;
                for y in 0..MAP_H {
                    for x in 0..MAP_W {
                        if (x - cx).abs() + (y - cy).abs() < 40 && m.ground_at(x, y) == g {
                            n += 1;
                        }
                    }
                }
                n
            };
            assert!(
                near(lx, ly, Ground::Shallows) > near(hx, hy, Ground::Shallows),
                "seed {seed}: the low corner is not the wet one"
            );
        }
    }

    /// The guarantee the plan asks for, checked at every player count over
    /// enough seeds that a ring geometry mistake cannot hide in one of them.
    #[test]
    fn sites_are_far_enough_apart() {
        for players in 2..=6u32 {
            for seed in 0..200u64 {
                let m = gen(seed, players);
                assert_eq!(m.hearth_sites.len(), players as usize);
                for (i, &(ax, ay)) in m.hearth_sites.iter().enumerate() {
                    for &(bx, by) in &m.hearth_sites[i + 1..] {
                        let d2 = (ax - bx).pow(2) + (ay - by).pow(2);
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
    fn the_low_corner_is_nearer_to_some_cities_than_others() {
        // Design §6: comparable ground, but not identical — somebody starts
        // closer to where the water comes from, and the seed decides who.
        let (lx, ly) = {
            let m = gen(3, 4);
            m.low_corner.cell()
        };
        let m = gen(3, 4);
        let mut d: Vec<i32> = m
            .hearth_sites
            .iter()
            .map(|&(x, y)| (x - lx).abs() + (y - ly).abs())
            .collect();
        d.sort_unstable();
        assert!(
            d[d.len() - 1] - d[0] > 40,
            "every city was about as exposed as every other: {d:?}"
        );
    }

    #[test]
    fn the_rotation_moves_the_cities_between_seeds() {
        // Without the ring rotation every map would put city 0 in the same
        // place, which would make the seed much less interesting than it looks.
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

    /// Not a test — a measurement, run by hand with
    /// `cargo test -p sim probe -- --ignored --nocapture` when the terrain
    /// constants need choosing again.
    #[test]
    #[ignore]
    fn sweep_noise_amplitude() {
        for amp in [90i32, 110, 130, 150, 170, 190] {
            let mut wrong_corner = 0;
            let mut worst_ramp = i32::MAX;
            for seed in 0..300u64 {
                let mut rng = Rng::new(seed);
                let low = Corner::ALL[rng.below(4) as usize];
                let h = terrain(&mut rng, low, amp);
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
