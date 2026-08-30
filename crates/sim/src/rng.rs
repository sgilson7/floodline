//! The one generator.
//!
//! There is exactly one of these and it lives in `World`. Nothing else in
//! `sim` may produce a random number, and `sim` may not reach for `rand`,
//! because the seed is the only thing that decides what a run contains and
//! every peer has to draw from it in the same order. A second generator
//! somewhere, or a draw made on one peer and not another, is a desync that
//! shows up minutes later as two different floods.
//!
//! xorshift64*, taken from gear-master's `crates/engine/src/rng.rs`, which
//! already wanted exactly this: small, seeded, and replayable in a test.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // A zero state would stick at zero forever.
        Rng { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn next_u32(&mut self) -> u32 {
        // The high bits of a xorshift* are the well-mixed ones.
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in `0..n`. Returns 0 when `n` is 0, rather than dividing by it.
    ///
    /// Modulo, not rejection sampling: the bias is one part in 2^64/n, which
    /// for any n this game asks for is far below anything a player could
    /// notice, and rejection sampling would make the number of draws depend on
    /// the values drawn. That is a bad trade here — a variable number of draws
    /// is exactly the shape of bug that makes two peers diverge if one of them
    /// ever gets a slightly different n.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as u32
    }

    /// Uniform in `lo..=hi`. Swaps them rather than panicking if they arrive
    /// the wrong way round.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    /// One in `n`.
    pub fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }

    /// Fisher-Yates, so drawing without replacement is "shuffle and take".
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }
        for i in (1..items.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values a fresh `Rng::new(1)` produces, written down.
    ///
    /// This is the test that would catch someone "improving" the generator.
    /// Every saved seed, every recorded desync and every replayed map depends
    /// on these exact numbers, so changing them is a decision, not a tidy-up.
    ///
    /// The numbers were computed by a separate implementation of xorshift64*
    /// and then checked against this one, rather than copied out of a run of
    /// the code they are testing. A golden test written the other way round
    /// only asserts that the code still does whatever it did, which is not
    /// the same as asserting it does the right thing.
    #[test]
    fn the_sequence_is_pinned() {
        let mut r = Rng::new(1);
        let got: Vec<u64> = (0..4).map(|_| r.next_u64()).collect();
        assert_eq!(
            got,
            vec![
                5180492295206395165,
                12380297144915551517,
                13389498078930870103,
                5599127315341312413,
            ]
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_sequence() {
        let mut x = Rng::new(7);
        let mut y = Rng::new(7);
        for _ in 0..1000 {
            assert_eq!(x.next_u64(), y.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn a_zero_seed_does_not_stick() {
        let mut r = Rng::new(0);
        let a = r.next_u64();
        let b = r.next_u64();
        assert_ne!(a, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn below_stays_in_range() {
        let mut r = Rng::new(99);
        for n in 1..40u32 {
            for _ in 0..100 {
                assert!(r.below(n) < n);
            }
        }
        assert_eq!(r.below(0), 0, "degenerate case must not divide by zero");
    }

    #[test]
    fn range_covers_both_ends_and_nothing_outside() {
        let mut r = Rng::new(4);
        let mut seen_lo = false;
        let mut seen_hi = false;
        for _ in 0..2000 {
            let v = r.range(-3, 3);
            assert!((-3..=3).contains(&v), "{v} outside -3..=3");
            seen_lo |= v == -3;
            seen_hi |= v == 3;
        }
        assert!(seen_lo && seen_hi, "both ends are reachable");
        assert_eq!(r.range(5, 5), 5);
        assert!((1..=9).contains(&r.range(9, 1)), "reversed bounds still work");
    }

    #[test]
    fn below_is_roughly_uniform() {
        // Not a statistics test, a "did somebody break the shift" test: with
        // 60 000 draws over 6 buckets, a working generator lands every bucket
        // within a few percent of 10 000 and a broken one is obvious.
        let mut r = Rng::new(12345);
        let mut counts = [0u32; 6];
        for _ in 0..60_000 {
            counts[r.below(6) as usize] += 1;
        }
        for (i, c) in counts.iter().enumerate() {
            assert!((9_000..11_000).contains(c), "bucket {i} got {c}");
        }
    }

    #[test]
    fn shuffle_keeps_every_element_and_moves_them() {
        let mut r = Rng::new(5);
        let mut v: Vec<usize> = (0..40).collect();
        r.shuffle(&mut v);
        let mut sorted = v.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..40).collect::<Vec<_>>());
        assert_ne!(v, sorted, "and actually reorders them");

        // Degenerate lengths do not panic.
        r.shuffle(&mut [] as &mut [u8]);
        r.shuffle(&mut [1]);
    }

    #[test]
    fn shuffling_is_reproducible() {
        let run = || {
            let mut r = Rng::new(808);
            let mut v: Vec<usize> = (0..20).collect();
            r.shuffle(&mut v);
            v
        };
        assert_eq!(run(), run());
    }
}
