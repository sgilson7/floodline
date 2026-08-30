//! Fixed point, 8 fractional bits.
//!
//! Positions, velocities and every other quantity that would want to be a
//! float are `i32` in units of 1/256 of a cell. Not because integers are
//! faster — they are not, meaningfully — but because a float is the one thing
//! that can make two peers running the same code disagree. IEEE 754 does not
//! promise that `a * b` rounds the same way on a laptop's x86 and in a
//! browser's wasm engine, and a lockstep game that disagrees in the last bit
//! disagrees about everything one minute later.
//!
//! Two rules the rest of `sim` relies on:
//!
//! * Shifts right are arithmetic and therefore floor toward negative
//!   infinity. `Fx(-1).floor()` is `-1`, not `0`. That is a choice, and it is
//!   the same choice everywhere, which is the part that matters.
//! * Every multiply and divide goes through `i64` before coming back, so the
//!   intermediate cannot overflow at any coordinate a 128x128 map can hold.
//!   `[profile.release]` turns overflow checks on anyway: a panic is a loud
//!   identical failure on every peer, and a wrap is a silent desync.

use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use serde::{Deserialize, Serialize};

/// How many of an `Fx`'s low bits are the fraction.
pub const FRAC_BITS: u32 = 8;

/// One whole cell.
pub const ONE: Fx = Fx(1 << FRAC_BITS);
pub const ZERO: Fx = Fx(0);
/// Half a cell — where a citizen stands when it stands "on" a cell.
pub const HALF: Fx = Fx(1 << (FRAC_BITS - 1));

/// A fixed-point number: `raw / 256`.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct Fx(pub i32);

impl Fx {
    /// A whole number of cells.
    pub const fn cells(n: i32) -> Fx {
        Fx(n << FRAC_BITS)
    }

    /// The underlying 1/256ths.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// The cell this falls in, rounding toward negative infinity, so that
    /// walking left across zero does not stall on cell 0 for two steps.
    pub const fn floor(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Nearest whole cell, halves going up.
    pub const fn round(self) -> i32 {
        (self.0 + (1 << (FRAC_BITS - 1))) >> FRAC_BITS
    }

    /// What is left after `floor`. Always in `0..ONE`, including for
    /// negatives, which is what makes it usable as a lerp weight.
    pub const fn frac(self) -> Fx {
        Fx(self.0 & ((1 << FRAC_BITS) - 1))
    }

    pub const fn abs(self) -> Fx {
        Fx(if self.0 < 0 { -self.0 } else { self.0 })
    }

    pub const fn min(self, other: Fx) -> Fx {
        if self.0 < other.0 {
            self
        } else {
            other
        }
    }

    pub const fn max(self, other: Fx) -> Fx {
        if self.0 > other.0 {
            self
        } else {
            other
        }
    }

    pub const fn clamp(self, lo: Fx, hi: Fx) -> Fx {
        self.max(lo).min(hi)
    }

    /// Square root, by Newton's method on integers.
    ///
    /// Negatives return zero rather than panicking: the callers are lengths
    /// and speeds, and a length is never legitimately negative, so a negative
    /// here is a bug upstream that should not also take the frame down.
    pub const fn sqrt(self) -> Fx {
        if self.0 <= 0 {
            return ZERO;
        }
        // sqrt(v / 256) * 256 == sqrt(v * 256), and v * 256 needs the i64.
        Fx(isqrt((self.0 as i64) << FRAC_BITS) as i32)
    }
}

/// Floor of the square root, by Newton's method. Converges in a handful of
/// steps and uses nothing but integer division, so it is the same number on
/// every machine.
pub const fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, r: Fx) -> Fx {
        Fx(self.0 + r.0)
    }
}
impl Sub for Fx {
    type Output = Fx;
    fn sub(self, r: Fx) -> Fx {
        Fx(self.0 - r.0)
    }
}
impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(-self.0)
    }
}
impl AddAssign for Fx {
    fn add_assign(&mut self, r: Fx) {
        self.0 += r.0;
    }
}
impl SubAssign for Fx {
    fn sub_assign(&mut self, r: Fx) {
        self.0 -= r.0;
    }
}

impl Mul for Fx {
    type Output = Fx;
    /// Truncates toward negative infinity, because `>>` does.
    fn mul(self, r: Fx) -> Fx {
        Fx(((self.0 as i64 * r.0 as i64) >> FRAC_BITS) as i32)
    }
}

impl Div for Fx {
    type Output = Fx;
    /// Truncates toward zero, because integer division does. Panics on a zero
    /// divisor, exactly as `i32 / 0` would — identically on every peer.
    fn div(self, r: Fx) -> Fx {
        Fx((((self.0 as i64) << FRAC_BITS) / r.0 as i64) as i32)
    }
}

impl Mul<i32> for Fx {
    type Output = Fx;
    fn mul(self, r: i32) -> Fx {
        Fx(self.0 * r)
    }
}

impl Div<i32> for Fx {
    type Output = Fx;
    fn div(self, r: i32) -> Fx {
        Fx(self.0 / r)
    }
}

/// A sixteenth of a turn is 16; a whole turn wraps a `u8` back to zero, which
/// is why angles are `u8` and never need normalising.
pub type Turns = u8;

/// `sin(2*pi*i/256) * 256`, rounded. Written out rather than computed because
/// computing it would need the floats this module exists to avoid.
#[rustfmt::skip]
const SIN: [i32; 256] = [
    0, 6, 13, 19, 25, 31, 38, 44,
    50, 56, 62, 68, 74, 80, 86, 92,
    98, 104, 109, 115, 121, 126, 132, 137,
    142, 147, 152, 157, 162, 167, 172, 177,
    181, 185, 190, 194, 198, 202, 206, 209,
    213, 216, 220, 223, 226, 229, 231, 234,
    237, 239, 241, 243, 245, 247, 248, 250,
    251, 252, 253, 254, 255, 255, 256, 256,
    256, 256, 256, 255, 255, 254, 253, 252,
    251, 250, 248, 247, 245, 243, 241, 239,
    237, 234, 231, 229, 226, 223, 220, 216,
    213, 209, 206, 202, 198, 194, 190, 185,
    181, 177, 172, 167, 162, 157, 152, 147,
    142, 137, 132, 126, 121, 115, 109, 104,
    98, 92, 86, 80, 74, 68, 62, 56,
    50, 44, 38, 31, 25, 19, 13, 6,
    0, -6, -13, -19, -25, -31, -38, -44,
    -50, -56, -62, -68, -74, -80, -86, -92,
    -98, -104, -109, -115, -121, -126, -132, -137,
    -142, -147, -152, -157, -162, -167, -172, -177,
    -181, -185, -190, -194, -198, -202, -206, -209,
    -213, -216, -220, -223, -226, -229, -231, -234,
    -237, -239, -241, -243, -245, -247, -248, -250,
    -251, -252, -253, -254, -255, -255, -256, -256,
    -256, -256, -256, -255, -255, -254, -253, -252,
    -251, -250, -248, -247, -245, -243, -241, -239,
    -237, -234, -231, -229, -226, -223, -220, -216,
    -213, -209, -206, -202, -198, -194, -190, -185,
    -181, -177, -172, -167, -162, -157, -152, -147,
    -142, -137, -132, -126, -121, -115, -109, -104,
    -98, -92, -86, -80, -74, -68, -62, -56,
    -50, -44, -38, -31, -25, -19, -13, -6,
];

pub fn sin(a: Turns) -> Fx {
    Fx(SIN[a as usize])
}

/// A quarter turn ahead of sine, and `u8` arithmetic wraps for free.
pub fn cos(a: Turns) -> Fx {
    Fx(SIN[a.wrapping_add(64) as usize])
}

/// A point or a displacement, in cells.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct V2 {
    pub x: Fx,
    pub y: Fx,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: ZERO, y: ZERO };

    pub const fn new(x: Fx, y: Fx) -> V2 {
        V2 { x, y }
    }

    /// The centre of a cell, which is where anything standing on one stands.
    pub const fn cell_centre(cx: i32, cy: i32) -> V2 {
        V2 { x: Fx(cx << FRAC_BITS | HALF.0), y: Fx(cy << FRAC_BITS | HALF.0) }
    }

    /// Which cell this is in.
    pub const fn cell(self) -> (i32, i32) {
        (self.x.floor(), self.y.floor())
    }

    /// Squared length. Kept separate because comparing two distances never
    /// needs the square root, and the square root is the expensive part.
    pub fn len_sq(self) -> Fx {
        self.x * self.x + self.y * self.y
    }

    pub fn len(self) -> Fx {
        self.len_sq().sqrt()
    }

    /// Scaled to length `to`, or zero if it has no direction to keep.
    pub fn with_len(self, to: Fx) -> V2 {
        let l = self.len();
        if l.0 == 0 {
            V2::ZERO
        } else {
            V2 { x: self.x * to / l, y: self.y * to / l }
        }
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, r: V2) -> V2 {
        V2 { x: self.x + r.x, y: self.y + r.y }
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, r: V2) -> V2 {
        V2 { x: self.x - r.x, y: self.y - r.y }
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, r: V2) {
        self.x += r.x;
        self.y += r.y;
    }
}
impl Mul<Fx> for V2 {
    type Output = V2;
    fn mul(self, r: Fx) -> V2 {
        V2 { x: self.x * r, y: self.y * r }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_cells_survive_the_round_trip() {
        for n in -200..200 {
            assert_eq!(Fx::cells(n).floor(), n);
            assert_eq!(Fx::cells(n).round(), n);
            assert_eq!(Fx::cells(n).frac(), ZERO);
        }
    }

    #[test]
    fn floor_goes_down_on_both_sides_of_zero() {
        // The whole reason `>>` is used rather than `/`: truncation toward
        // zero would make cell 0 twice as wide as every other cell, and a
        // citizen walking left would appear to pause there.
        assert_eq!(Fx(255).floor(), 0);
        assert_eq!(Fx(256).floor(), 1);
        assert_eq!(Fx(-1).floor(), -1);
        assert_eq!(Fx(-256).floor(), -1);
        assert_eq!(Fx(-257).floor(), -2);
        // And `frac` stays a usable weight below zero.
        for raw in -1000..1000 {
            let f = Fx(raw).frac();
            assert!(f >= ZERO && f < ONE, "frac({raw}) = {f:?}");
            assert_eq!(Fx::cells(Fx(raw).floor()) + f, Fx(raw));
        }
    }

    #[test]
    fn one_is_the_multiplicative_identity() {
        for raw in [-30000, -257, -1, 0, 1, 256, 1000, 30000] {
            assert_eq!(Fx(raw) * ONE, Fx(raw));
            assert_eq!(Fx(raw) / ONE, Fx(raw));
        }
    }

    #[test]
    fn multiplication_matches_the_rational_it_stands_for() {
        // a/256 * b/256 == ab/65536, and the result is that in 256ths.
        for a in (-2000..2000).step_by(37) {
            for b in (-2000..2000).step_by(53) {
                let want = ((a as i64 * b as i64) >> FRAC_BITS) as i32;
                assert_eq!((Fx(a) * Fx(b)).raw(), want, "{a} * {b}");
            }
        }
    }

    #[test]
    fn multiplication_of_map_scale_values_does_not_overflow() {
        // The largest coordinate the map can hold, squared, is the worst case
        // any distance calculation reaches.
        let far = Fx::cells(128);
        let d = V2::new(far, far);
        assert!(d.len_sq().raw() > 0, "squared length stayed positive");
        // 128 * sqrt(2) = 181.019…, and we want it to a fraction of a cell.
        assert_eq!(d.len().floor(), 181);
    }

    #[test]
    fn sqrt_is_the_floor_of_the_real_one() {
        for n in 0..5000i64 {
            let r = isqrt(n);
            assert!(r * r <= n, "isqrt({n}) = {r} too big");
            assert!((r + 1) * (r + 1) > n, "isqrt({n}) = {r} too small");
        }
        assert_eq!(isqrt(-9), 0, "negatives are zero, not a panic");
    }

    #[test]
    fn fx_sqrt_is_exactly_the_floor_of_the_fixed_point_root() {
        // Stated as the bracketing property rather than as "square it and see
        // if you get back", because a flooring root is off by up to one raw
        // step and squaring turns that into an error proportional to the root
        // itself — a fixed tolerance would either be too loose near 1 or too
        // tight near 200. This is exact.
        for raw in (1..60_000).step_by(7) {
            let v = Fx(raw);
            let r = v.sqrt().raw() as i64;
            let scaled = (raw as i64) << FRAC_BITS;
            assert!(r * r <= scaled, "sqrt({raw}) = {r}, too big");
            assert!((r + 1) * (r + 1) > scaled, "sqrt({raw}) = {r}, too small");
        }
        // Perfect squares come out exact, which is the case anyone reading a
        // number on screen will check first.
        assert_eq!(Fx::cells(4).sqrt(), Fx::cells(2));
        assert_eq!(Fx::cells(9).sqrt(), Fx::cells(3));
        assert_eq!(Fx::cells(144).sqrt(), Fx::cells(12));
        assert_eq!(Fx(-5).sqrt(), ZERO);
    }

    #[test]
    fn the_sine_table_is_a_circle() {
        assert_eq!(sin(0), ZERO);
        assert_eq!(sin(64), ONE);
        assert_eq!(sin(128), ZERO);
        assert_eq!(sin(192), -ONE);
        assert_eq!(cos(0), ONE);
        assert_eq!(cos(64), ZERO);

        for a in 0..=255u8 {
            // Odd symmetry about half a turn.
            assert_eq!(sin(a.wrapping_add(128)), -sin(a), "sin symmetry at {a}");
            // And the identity that says the table really is a circle. The
            // slack is two fixed-point steps: each entry is rounded, and
            // squaring doubles that error.
            let unit = sin(a) * sin(a) + cos(a) * cos(a);
            assert!(
                (unit - ONE).abs() <= Fx(3),
                "sin^2+cos^2 at {a} was {unit:?}, not {ONE:?}"
            );
        }
    }

    #[test]
    fn a_unit_vector_has_unit_length() {
        for a in 0..=255u8 {
            let v = V2::new(cos(a), sin(a));
            let l = v.len();
            assert!((l - ONE).abs() <= Fx(2), "|(cos,sin)| at {a} was {l:?}");
        }
    }

    #[test]
    fn with_len_rescales_and_survives_a_zero_vector() {
        let v = V2::new(Fx::cells(3), Fx::cells(4));
        assert_eq!(v.len(), Fx::cells(5));
        let half = v.with_len(Fx::cells(1));
        assert!((half.len() - Fx::cells(1)).abs() <= Fx(2));
        assert_eq!(V2::ZERO.with_len(Fx::cells(1)), V2::ZERO);
    }

    #[test]
    fn cell_centre_lands_in_its_own_cell() {
        for cx in 0..128 {
            for cy in (0..128).step_by(17) {
                let p = V2::cell_centre(cx, cy);
                assert_eq!(p.cell(), (cx, cy));
                assert_eq!(p.x.frac(), HALF);
            }
        }
    }
}
