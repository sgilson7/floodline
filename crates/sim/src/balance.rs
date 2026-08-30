//! Every number that is a judgement rather than a fact.
//!
//! Collected in one file because the plan's phase 5 ends with "playtest the
//! flood until it is fun", and that is going to mean changing numbers. When it
//! does, they should all be here, next to the reasoning, instead of scattered
//! through the rules as literals nobody can find twice.
//!
//! Anything here may move. Nothing here may become a float.

// ---- time ------------------------------------------------------------------

/// Simulation ticks per second (design §3.1).
pub const TICKS_PER_SECOND: u32 = 10;

/// Ticks in an in-game day (design §4): 20 seconds of wall clock.
pub const TICKS_PER_DAY: u32 = 200;

/// Days in an age. Design §4 guesses six, and design §11 flags the guess as
/// open — twelve real minutes an age may be too slow for an evening with
/// friends. Left at six until phase 5 can time a real run.
pub const DAYS_PER_AGE: u32 = 6;

// ---- citizens --------------------------------------------------------------

/// Needs run 0..=NEED_FULL (design §3.2).
pub const NEED_FULL: u16 = 1000;

/// Food falls by this much a tick, so a full citizen empties in 250 ticks —
/// a day and a quarter. Slightly longer than a day on purpose: a citizen who
/// has to eat exactly once a day would have every citizen in a city queue at
/// the granary at the same hour, which looks like a bug and plays like one.
pub const FOOD_DECAY: u16 = 4;

/// Rest falls a little slower, emptying in 333 ticks, so sleep and hunger
/// drift out of phase with each other rather than arriving together.
pub const REST_DECAY: u16 = 3;

/// A citizen on empty starves after three days (design §3.2).
pub const STARVE_TICKS: u32 = 3 * TICKS_PER_DAY;

/// Below this, a citizen looks for food rather than carrying on working.
pub const HUNGRY: u16 = 300;

/// Below this, a citizen works at half speed (design §3.2).
pub const TIRED: u16 = 200;

/// The founding party (design §4).
pub const FOUNDING_CITIZENS: u32 = 8;

// ---- the map ---------------------------------------------------------------

/// Terrain is a corner-to-corner ramp with noise laid over it, rather than a
/// blend of the two. Blending was the first attempt and it made a map with no
/// shallows and no rock on it: averaging four octaves of noise pulls hard
/// toward the middle, and averaging *that* with the ramp pulled the whole
/// height range into 40..220, where none of the ground thresholds live.
///
/// So the ramp spans the full height scale by itself and the noise is a
/// signed offset added on top. The low corner then reads as genuinely low
/// because it is, rather than because it is a bit below average.
pub const SLOPE_SPAN: i32 = 255;

/// How far the noise pushes a cell off the ramp, at the extremes. Large
/// enough that a lowland has ponds in it and a highland has outcrops; small
/// enough that the ramp still decides which end of the map is which.
///
/// Chosen by measurement, not by eye — `map::probe::sweep_noise_amplitude`
/// counts, over 300 seeds, how often the *high* corner ends up wetter than
/// the low one, which is the failure that would put the flood's source on the
/// wrong end of the map:
///
/// ```text
/// amplitude  90: 0/300 wrong, smallest corner-to-corner drop 142
/// amplitude 110: 0/300 wrong, smallest corner-to-corner drop 123
/// amplitude 130: 1/300 wrong, smallest corner-to-corner drop 104
/// amplitude 170: 5/300 wrong, smallest corner-to-corner drop  67
/// ```
///
/// 110 is the largest value that never got it wrong. Re-run the sweep before
/// changing this or the octave weights.
pub const NOISE_AMPLITUDE: i32 = 110;

/// How much of the map is each ground type, in percent of its cells.
///
/// Fractions rather than fixed heights on the 0..=255 scale, because fixed
/// heights make the map a lottery: the coarse noise octave is one draw, and
/// when it comes out flat the whole map lands inside the grass band with no
/// shallows for a river and no rock anywhere. Seeds 0 and 7 were exactly that.
/// Cutting the bands by quantile instead means every seed is a playable map
/// with the same rough composition, and the seed decides where the wet ground
/// is rather than whether there is any.
pub const SHALLOWS_PERCENT: i32 = 12;
pub const SAND_PERCENT: i32 = 6;
pub const ROCK_PERCENT: i32 = 8;

/// Hearth sites are placed on a ring around the map centre. The radius is
/// what makes the spacing guarantee hold: at six players the ring's shortest
/// chord is about 53 cells, and jitter and snapping can each pull two
/// neighbours about 11 cells closer together, which still clears the 40 the
/// plan asks for. Raising the player count or the jitter means redoing that
/// sum — `sites_are_far_enough_apart` is the test that will notice.
pub const SITE_RING_RADIUS: i32 = 54;
pub const SITE_JITTER: i32 = 2;
pub const SITE_SNAP: i32 = 2;
pub const MIN_SITE_SPACING: i32 = 40;

/// The Hearth's footprint, and so the size of the flat pad the generator
/// levels under each site.
pub const HEARTH_SIZE: i32 = 3;
