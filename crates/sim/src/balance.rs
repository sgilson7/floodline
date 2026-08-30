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

/// Ticks in an in-game day: two minutes of wall clock.
///
/// Design §4 says "6 days, about 12 real minutes at 10 ticks/s with 200 ticks
/// per day", and those numbers contradict each other — six days of two hundred
/// ticks is twelve hundred ticks, which at ten a second is two minutes, not
/// twelve. §11 flags age length as an open guess, so this was asked rather
/// than decided, and the answer was to honour the prose: twelve minutes an
/// age, so a day is twelve hundred ticks.
///
/// It also settles a second contradiction. §5 wants the surge to pour for
/// about thirty seconds, and thirty seconds is three hundred ticks — longer
/// than a two-hundred-tick day, so the flood could not have fitted inside its
/// own impact day. At twelve hundred it has room to spread, pool behind a
/// dike and drain, which is most of what makes the flood readable.
pub const TICKS_PER_DAY: u32 = 1200;

/// Days in an age. Six, from design §4 — so an age is twelve minutes and a
/// full three-age MVP run is a little over half an hour.
///
/// §11 worries this is too slow for an evening with friends, and phase 5's
/// playtesting is where that gets answered with a stopwatch rather than
/// arithmetic. If it does turn out too long, this is the constant to change:
/// nothing else in `sim` assumes a particular number of days.
pub const DAYS_PER_AGE: u32 = 6;

// ---- citizens --------------------------------------------------------------

/// Needs run 0..=NEED_FULL (design §3.2).
pub const NEED_FULL: u16 = 1000;

/// Food falls by this much a tick, so a full citizen empties in a thousand
/// ticks — a little over four fifths of a day. Slightly less than a day on
/// purpose: a citizen who had to eat exactly once a day would put every
/// citizen in the city at the granary at the same hour, which looks like a bug
/// and plays like one.
pub const FOOD_DECAY: u16 = 1;

/// Rest falls by one point every `REST_DECAY_INTERVAL` ticks, so it empties in
/// two thousand — well over a day, and at a different rate from hunger, so the
/// two needs drift out of phase rather than always arriving together.
///
/// An interval rather than a smaller per-tick number because there is no
/// smaller whole number than one. Design §3.2 fixes the needs at 0..=1000, so
/// the only way to make a need slower than "one point a tick" is to skip
/// ticks.
pub const REST_DECAY: u16 = 1;
pub const REST_DECAY_INTERVAL: u32 = 2;

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
pub const SLOPE_SPAN: i32 = 40;

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
pub const NOISE_AMPLITUDE: i32 = 16;

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

/// Hearth sites sit on a line at a fixed distance from the corner the water
/// comes out of, spread along it. The "shore parallel".
///
/// **This replaced a ring around the map centre, and the reason is measured.**
/// `tests/playtest.rs::how_far_the_water_reaches` pours each age's surge onto
/// generated maps and reports the deepest water at each Manhattan distance
/// from the corner it came from:
///
/// | from the corner | age 1-2 (height 12) | age 3 (height 18) |
/// |---|---|---|
/// | 40  | median 108 - swimming   | median 162 - swimming |
/// | 55  | median 63, deepest 245  | median 100 - swimming |
/// | 70  | median 63, deepest 213  | median 72, deepest 308 |
/// | 85  | median 41, deepest 150  | median 69, deepest 245 |
/// | 100 | median 18, deepest 66   | median 33, deepest 152 |
/// | 115 | median 3                | deepest 50 |
/// | 130 | dry                     | deepest 32 |
///
/// Wading starts at 32, swimming at 96, and a citizen drowns after fifty ticks
/// out of its depth. So a city at 40 cells is dead on the first flood before
/// it has finished a second building, a city past 115 never sees water in a
/// whole three-age run, and the game is a game between about 70 and 105.
///
/// A ring of radius 54 around the map centre put its sites anywhere from about
/// 40 to about 150 cells out, because the centre of a 128-cell map is 128
/// Manhattan cells from its corner. Three full runs of four strategies showed
/// what that cost: on one seed the age-one flood took five of eight citizens
/// before the city had a granary, on another no water reached the city in any
/// of the three ages, and between those two nothing a player *did* — a dike,
/// running for high ground — moved the outcome as much as which spot the
/// rotation happened to hand them. The ring cannot be fixed by moving it: a
/// circle about a point is not equidistant from a corner, and one of radius 54
/// already spans 108 of the map's 128 cells.
///
/// So the sites go on the line `x + y = SHORE_DISTANCE` measured from the low
/// corner, spread evenly along it. Ninety-six puts an age-one flood in the
/// streets — wading, with pockets deep enough to drown somebody standing in
/// the wrong place — and makes an age-three flood properly dangerous.
pub const SHORE_DISTANCE: i32 = 96;

/// How close to the ends of the shore line a site may sit. The line runs from
/// one map edge to the other; without this the outermost cities are jammed
/// into the corners beside the low one — at four, a hearth landed three cells
/// from the map's west edge with nowhere to put a farm on that side. Eight is
/// the compromise: it costs the closest pair at six players two cells and buys
/// the end cities somewhere to build.
pub const SHORE_MARGIN: i32 = 8;

pub const SITE_JITTER: i32 = 1;
pub const SITE_SNAP: i32 = 2;

/// The plan asks for forty cells between cities. This is what the map gives.
///
/// Measured over two hundred seeds at each player count, by
/// `sites_are_far_enough_apart`, which prints the table with `--nocapture`:
///
/// | players | closest two cities |
/// |---|---|
/// | 2 | 108 |
/// | 3 | 51 |
/// | 4 | 31 |
/// | 5 | 22 |
/// | 6 | 17 |
///
/// Two and three clear the plan's forty; four is short and five and six are
/// well short, and none of that can be fixed where it looks like it should be.
/// The usable shore is `SHORE_DISTANCE - 2 * SHORE_MARGIN` cells of x, which
/// is about a hundred and thirteen cells of line. Six cities forty apart need
/// two hundred. The shore cannot be lengthened without moving it out of the
/// flood, and moving it out of the flood is what the ring did and what this
/// whole change exists to undo.
///
/// So: a guarantee of seventeen, and design section 11's "map size vs. citizen
/// count" gets a second reason to be an open question. Seventeen cells between
/// two three-by-three hearths leaves fourteen of clear ground, which is
/// cramped and playable; five or six players want a bigger map, not a
/// different rule. Given the choice between neighbours who can see each other
/// and whole cities standing outside the flood for a three-age run, the flood
/// wins: it is the game.
pub const MIN_SITE_SPACING: i32 = 17;

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

/// Farmer-ticks per unit of food produced.
///
/// A citizen burns a thousand points of food need a day and one unit fills a
/// hundred, so eating costs about twelve units a day. At thirty-two ticks a
/// unit a farmer makes thirty-seven a day and so feeds about three people, and
/// a three-slot farm feeds nine. A founding party of eight is therefore one
/// farm and some room to grow, rather than a city where everybody farms.
pub const FARM_TICKS_PER_UNIT: u32 = 32;

/// How much a farm holds before its farmers stop, waiting for a hauler. Small
/// on purpose: a farm that could stockpile a week of food would make haulers
/// optional, and watching the food move is the point.
pub const FARM_BUFFER: u16 = 60;

/// Units a citizen can carry at once.
pub const CARRY_CAPACITY: u16 = 20;

/// Units of food eaten per tick at a granary: a meal from hungry to full is
/// six units and six ticks, which is long enough to see somebody standing
/// there and short enough not to be a queue.
pub const EAT_RATE: u16 = 1;

/// Rest recovered per tick in a bed. Two a tick takes an exhausted citizen
/// from `TIRED` to `RESTED_ENOUGH` in about three hundred and seventy ticks —
/// roughly a third of a day, which is a night.
pub const SLEEP_RATE: u16 = 2;

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

// ---- ages ------------------------------------------------------------------

/// The last age of an MVP run. The plan: "Ages 1–3 exist (flood, escalating
/// height); the run ends when both cities fall or after age 3, whichever
/// first."
pub const MAX_AGE: u32 = 3;

/// How long the source corner keeps pouring water in, in ticks. Design §5:
/// about thirty seconds. The surge is not a scripted wave — it is a source
/// strong enough that the automaton produces a front.
pub const SURGE_TICKS: u32 = 30 * TICKS_PER_SECOND;

// ---- water -----------------------------------------------------------------

/// Water is measured in sixteenths of a unit of terrain height.
///
/// Depth and terrain have to be comparable — a surge of height 12 has to mean
/// something against a hill of height 12 — but comparing them at terrain's own
/// resolution does not survive integer division. Splitting a cell's outflow
/// between four equally-lower neighbours means dividing by four, and at
/// terrain resolution the answer is usually zero: a two-deep puddle on flat
/// ground never moves at all, and a puddle that does move gives the remainder
/// to whichever neighbour the loop happened to reach last, so it spreads
/// lopsidedly. The plan asks for a puddle that "spreads symmetrically", and
/// sixteenths are what make that true — the shares come out equal, and the few
/// sixteenths that division loses simply stay in the cell they were in, which
/// costs nothing and keeps the books balanced.
pub const DEPTH_SCALE: u16 = 16;

/// A depth in whole terrain-height units.
pub const fn depth(height_units: u16) -> u16 {
    height_units * DEPTH_SCALE
}

/// The most water one cell may pass to one neighbour in a tick.
///
/// The transfer rule already refuses to move more than would level the two
/// cells, so this is not what keeps the automaton stable — it is what stops a
/// deep column emptying into its neighbour in one tick and giving the front a
/// hard edge. Design §5 wants a front that advances about a cell a tick and
/// slows as it spreads, which is what a cap produces.
pub const MAX_TRANSFER: u16 = depth(8);

/// Water shallower than this is left alone: one sixteenth of a unit of
/// terrain height, which is a damp patch.
///
/// It has to be this small. The floor exists to stop the automaton shuffling
/// single sixteenths between cells that are already level, but it also decides
/// how thin a sheet of water can get before it stops moving — and a puddle
/// spreading on flat ground gets very thin indeed. At a quarter of a unit, a
/// column poured in the middle of the map froze at about sixteen cells across
/// and never reached an edge, so the map never drained: volume was conserved
/// and the water simply stopped. The transfer rule already refuses to
/// overshoot, so settling does not depend on this floor being generous.
pub const PUDDLE: u16 = 1;

/// Wading slows a citizen, swimming takes away its control, and long enough
/// under drowns it (design §3.4).
pub const WADE_DEPTH: u16 = depth(2);
pub const SWIM_DEPTH: u16 = depth(6);
pub const DROWN_TICKS: u32 = 5 * TICKS_PER_SECOND;

/// How much of a cell's flow a citizen picks up each tick, in 256ths. A body
/// is not a boat: it takes a fraction of the water's movement, and a strong
/// current still carries it off over a few seconds.
pub const WATER_DRAG: i32 = 3;

/// Flow a building shrugs off before the excess starts doing damage.
///
/// Measured, and measured twice, because the answer depends on the source
/// model and the source model changed underneath it. A surge whose corner
/// block was topped up without limit produced flow with a median near thirty
/// and a peak near three hundred and eighty; the capped surge that replaced it
/// produces a median of four, a ninety-ninth percentile around seventy, and a
/// peak near two hundred and fifty. Numbers calibrated against the first are
/// nonsense against the second — they left every building standing through the
/// front — and numbers for the second would have dissolved the whole map under
/// the first.
///
/// Against the surge as it now is: still and slow water does nothing, a strong
/// current takes wood apart, and only the front itself troubles stone. A dike
/// that is doing its job has still water piled against it and almost no flow
/// at all, which is design §3.4's "a building behind a dike sees zero flow and
/// takes nothing" applied to the dike as well; a dike standing in the front
/// does eventually go, which seems right — build it where the water arrives,
/// not where it comes out.
pub const RESIST_WOOD: u16 = depth(1);
pub const RESIST_STONE: u16 = depth(3);

/// The excess flow over a building's resistance is divided by this to get the
/// damage it takes that tick.
///
/// Calibrated so that design §5's "wooden buildings in the main flow break
/// within a few seconds" is true of the flow the flood actually has. A surge
/// fills a basin, and once it is in, the water is *deep and slow* —
/// twenty-eight sixteenths a tick nine cells from the source, against a peak
/// of two hundred and fifty that happens only at the leading edge and only for
/// a moment. Dividing the excess by sixteen, as the first version did, made
/// the damage zero everywhere except in that instant, and a cottage stood in
/// sixteen units of moving water indefinitely.
pub const FLOW_TOUGHNESS: u16 = 4;

/// The source corner is an 8 x 8 block (design §5).
pub const SURGE_SIZE: i32 = 8;

/// The shove the source gives the water, pointing at the middle of the map.
/// It is what makes the surge a wall coming at you rather than a puddle
/// spreading out of a corner.
/// How hard the source pumps inland, as a fraction of the surge's own height.
///
/// See `World::inject_surge`. It has to scale with the height or design §4's
/// escalation table is decoration: with a fixed pump, an age-one surge of
/// twelve and an age-four surge of twenty-four flooded exactly the same
/// fraction of the map, because the pump was providing all the water and the
/// height none of it. Halved, so the pump is a shove rather than a second
/// source.
pub const fn surge_push(height: u16) -> u16 {
    depth(height) / 2
}
