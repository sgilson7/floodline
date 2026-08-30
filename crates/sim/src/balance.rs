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

// ---- buildings -------------------------------------------------------------

/// What a city starts with, held at its Hearth.
///
/// Design §4 says "a stockpile of wood" and stops there, which leaves the MVP
/// unfinishable: the plan's list of buildings has no Quarry and its list of
/// jobs has no Quarrier, so there is no way to produce stone — and a Dike
/// costs stone, and surviving the flood behind a Dike is the entire point of
/// the vertical slice. So a city starts with stone as well. See DECISIONS.md.
///
/// Sized to cover roughly four Cottages and a Granary, plus three levels of
/// Dike, with nothing spare. A player who spends it all on housing has made a
/// choice about the flood, which is the choice the game is about.
pub const STARTING_WOOD: u16 = 200;
pub const STARTING_STONE: u16 = 120;

/// How much of its materials a demolished or ruined building gives back
/// (design §3.3: "rubble returns a fraction of its materials"), as a percent.
pub const RUBBLE_REFUND_PERCENT: u16 = 50;

/// Builder-ticks one builder contributes per tick at full rest. A tired
/// citizen works at half speed (design §3.2).
pub const BUILDER_EFFORT: u32 = 1;

/// How many builders can crowd onto one construction site.
pub const BUILDER_SLOTS: usize = 4;

/// A Dike level raises the effective ground by this much (design §3.3), and a
/// Dike can be built up to this many levels. Two levels stops an age-1 surge
/// of height 12 dead, which is the teaching moment in design §5.
pub const DIKE_HEIGHT_PER_LEVEL: u16 = 3;
pub const DIKE_MAX_LEVEL: u8 = 4;

// ---- getting about ---------------------------------------------------------

/// How far a citizen walks in a tick on open ground, in 256ths of a cell.
/// A quarter of a cell a tick is two and a half cells a second, which crosses
/// a city in a few seconds and the whole map in about a minute.
pub const WALK_SPEED: i32 = 64;

/// How many flow fields are kept before the least recently used is dropped.
/// A field is sixteen thousand cells; a dozen is plenty for one city's
/// granary, cottages and building sites, and a group sent to a cell shares
/// one between all of them.
pub const NAV_CACHE_MAX: usize = 24;

// ---- work ------------------------------------------------------------------

/// How much of a citizen's food need one unit of stored food fills.
///
/// This is the exchange rate between the two numbers that look like "food":
/// `Citizen::food` is a need on a 0..=1000 scale, and `Good::Food` is a thing
/// haulers carry. At 100, a citizen eats about eight units a day.
pub const FOOD_PER_UNIT: u16 = 100;

/// Farmer-ticks per unit of food produced. One farmer makes twenty-five units
/// a day, which feeds about three people; a three-slot farm feeds nine. A city
/// of eight is therefore one farm and some room to grow, not a city where
/// everybody farms.
pub const FARM_TICKS_PER_UNIT: u32 = 8;

/// How much a farm holds before its farmers stop, waiting for a hauler. Small
/// on purpose: a farm that could stockpile a week of food would make haulers
/// optional, and watching the food move is the point.
pub const FARM_BUFFER: u16 = 60;

/// Units a citizen can carry at once.
pub const CARRY_CAPACITY: u16 = 20;

/// Units of food eaten per tick at a granary, and rest recovered per tick in a
/// bed. Sleep has to beat `REST_DECAY` by enough to be worth the walk.
pub const EAT_RATE: u16 = 1;
pub const SLEEP_RATE: u16 = 20;

/// A citizen stops eating once this full, rather than at the brim, so it does
/// not spend its life at the granary topping up.
pub const FED_ENOUGH: u16 = 900;
pub const RESTED_ENOUGH: u16 = 950;

/// How long a ping stays on the map, in ticks. Three seconds: long enough to
/// look where somebody is pointing, short enough that the list of them cannot
/// grow into the snapshot.
pub const PING_LIFETIME: u32 = 3 * TICKS_PER_SECOND;

// ---- roads and trade -------------------------------------------------------

/// Cost of laying a road over a cell, relative to reusing an existing one,
/// which costs 1. Crossing water needs a bridge and is priced accordingly —
/// three times the ground, so a road goes round a wide river and across a
/// narrow one, which is what a road would do.
pub const ROAD_COST_GROUND: u32 = 10;
pub const ROAD_COST_WATER: u32 = 30;

/// How near another city's buildings a road has to end before that city can
/// accept it. Generous, because the alternative is a player being told their
/// road stopped one cell short.
pub const ROAD_JOIN_REACH: i32 = 6;

/// Haulers each city sends out per day per live trade. The goods are split
/// between them, so a bigger trade is a longer line of people rather than one
/// citizen carrying a mountain.
pub const CARAVAN_SIZE: usize = 3;
