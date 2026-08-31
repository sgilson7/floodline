//! Every number that is a judgement rather than a fact.
//!
//! Collected in one file because the plan's phase 5 ends with "playtest the
//! flood until it is fun", and that is going to mean changing numbers. When it
//! does, they should all be here, next to the reasoning, instead of scattered
//! through the rules as literals nobody can find twice.
//!
//! Anything here may move. Nothing here may become a float.

use crate::fx::Fx;

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

// ---- trade on the road -----------------------------------------------------

/// What a mule carries to the other city, in wood.
///
/// Ten, which is a hauler's load and change: the point of a mule is that it
/// goes a long way and comes back with something the city cannot make, not
/// that it moves a lot at once. One trader is one mule, so a post's job slots
/// are the trade rate.
pub const MULE_LOAD: u16 = 10;

/// What the other city pays for it, in gold.
///
/// **Provisional.** Gold buys levels and a level is one more pair of hands, so
/// what a round trip is worth cannot be settled until M7 has priced an upgrade.
/// Five for ten wood is a starting point that makes a post worth manning
/// within an age and no more than that.
pub const MULE_PAY: u16 = 5;

/// How close a mule has to get to count as arrived. A cart does not have to
/// stand on the doorstep.
pub const MULE_ARRIVED: i32 = 2;

/// How fast a mule walks on open ground, in 256ths of a cell a tick.
///
/// The same as a citizen at full rest, and it doubles on a road for the same
/// reason — which is what the plan means by "the road bonus it inherits for
/// free": a road is worth laying between two cities because the thing that
/// walks it goes twice as fast.
pub const MULE_SPEED: i32 = WALK_SPEED;

// ---- the river -------------------------------------------------------------

/// How many cells either side of the centreline are channel floor. Two makes
/// the river five cells across, which is wide enough to read as water with
/// banks and narrow enough that a bridge is a short thing to build.
pub const RIVER_HALF_WIDTH: i32 = 2;

/// How many further cells the bank is tapered over, so the channel has sides
/// rather than a cliff. The taper matters to the flood as much as to the eye:
/// a vertical wall of terrain reflects a surge, and design §5 wants it to
/// spill over the low bank and take the low country.
pub const RIVER_BANK: i32 = 3;

/// How far the channel floor is cut below the land it runs through.
///
/// Relative to the terrain rather than an absolute profile, because the map is
/// a ramp from a high corner to a low one and a river on a ramp is a river all
/// the way down: an absolute bed would be a canyon at the top and a puddle at
/// the bottom. Six against a relief of `SLOPE_SPAN` = 40 is a channel with
/// banks you can see and a floor a surge overtops well before it reaches the
/// low country, which is design §5's spill.
///
/// The cut is taken as a running minimum from source to mouth, so the bed only
/// ever descends. A reach that went uphill would pond, and a river that ponds
/// does not carry a wave.
pub const RIVER_DEPTH: i32 = 6;

/// How many cells either side of a point the channel's own height is averaged
/// over before the bed is cut from it.
///
/// A running minimum on the raw terrain is a trap, and it took the arrival
/// probe to find it. The land along a meander is noisy — `NOISE_AMPLITUDE` is
/// sixteen against a relief of forty — so one hollow drags the bed down and,
/// because a minimum never comes back up, every reach below it is cut to that
/// depth. On seed 31 the channel crossed a hollow early and spent the rest of
/// its length as a canyon at height zero beside a city standing at thirty. The
/// surge filled the canyon and the city never got its feet wet, which is not a
/// flood, it is a moat.
///
/// Averaging over seventeen cells leaves the ramp and takes out the noise, so
/// the bed follows the land down instead of remembering the lowest place it
/// has been.
pub const RIVER_SMOOTH: i32 = 8;

/// How many cells of the centreline the ford covers. Four, plus the width of
/// the channel, is a crossing wide enough to find on a map drawn at eight
/// pixels a cell and narrow enough that finding it is a thing you do.
pub const FORD_LENGTH: i32 = 4;

/// How much the channel floor is raised across the ford, out of `RIVER_DEPTH`.
/// A bar rather than a dam: half the channel's depth is a riffle you can
/// wade, and the water still runs over it.
pub const FORD_RISE: i32 = 3;

/// How far each of the meander's control points may be pushed sideways off
/// the straight line between the two mouths.
///
/// A river that is a straight line reads as a canal, and — the part that
/// matters for the game — a straight line between two edges divides the map
/// into two halves of predictable shape. A meander makes one bank roomier than
/// the other in a place the seed chooses.
pub const RIVER_MEANDER: i32 = 18;

/// How many control points the meander is bent at, either side of this range.
/// Three to five, as the plan asks.
pub const RIVER_BENDS: (i32, i32) = (3, 5);

/// How far from a corner a mouth may sit, so the channel never runs along an
/// edge or clips the corner itself.
pub const RIVER_MOUTH_MARGIN: i32 = 12;

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

/// How far from the river bank a hearth site sits.
///
/// **This used to mean "from the low corner", and the change is the whole
/// point of M4.** The flood came out of a corner and the sites went on the
/// line `x + y = 96` measured from it, because 70 to 105 was the band where an
/// age-one surge reached a city without drowning it. That table — the deepest
/// water at each distance from a corner — is still in `git log` and is now
/// about a flood that no longer exists.
///
/// The flood comes down a channel and spills over its banks, so what decides a
/// city's fate is its distance from the *bank*. Sites go on both banks, spread
/// along the channel, at this distance measured perpendicular to it.
/// `probe::how_far_the_spill_reaches` is the measurement that sets it, and the
/// table lives there rather than here because it is about the river.
pub const SHORE_DISTANCE: i32 = 14;

/// How much of the channel, at each end, no city is put beside.
///
/// A site opposite a mouth is a site in the corner of the map with nowhere to
/// build on three sides, and — worse — a site at the upstream mouth is a site
/// the surge arrives at before it has spread at all. Measured in cells of
/// centreline, not of map.
pub const SHORE_MARGIN: i32 = 16;

/// How high above the river bed a hearth site may stand, and how far below it.
///
/// The flood fills the channel to roughly `Disaster::height` above the bed and
/// spills from there, so this is the number that decides whether the water
/// reaches a city at all — the same job `SHORE_DISTANCE` used to do on its own
/// when the flood came out of a corner and distance was the only thing that
/// mattered. `map::probe::where_the_cities_sit` found sites standing anywhere
/// from fourteen below the bed to seventeen above it, and
/// `playtest::when_the_water_arrives` found what that costs: one city in six
/// never got its feet wet in three ages, which is the failure the shore
/// parallel was written to stop.
///
/// Minus four to plus twelve, against an age-one surge of twelve: everybody
/// is inside the reach of the first flood, and nobody is more than four below
/// the bed. Below the bed is a hollow and drowns you; too far above it and you
/// are not playing. Swept against `SITE_JITTER_BAND`, because the two together
/// are what decide how many cells are left to choose a city from — at minus
/// two to plus eight the closest pair at six players fell from eighteen cells
/// to seven.
pub const SITE_HEADROOM: (i32, i32) = (-4, 12);

/// How much open ground a city needs around it, as a percentage of the square
/// of side `2 * SITE_ELBOW + 1` centred on the site.
///
/// Being *in* the map's main region is not enough. A site jammed against a
/// rock face is in the main region and is sealed off the moment its own
/// hearth, farm, granary and cottage go down — which is how
/// `two_cities_found_a_road_and_trade_for_three_days` ended up unable to lay a
/// road between two cities that were both, technically, reachable.
pub const SITE_ELBOW: i32 = 6;
pub const SITE_ELBOW_PERCENT: i32 = 70;

/// How far from a hearth site the nearest rock may be.
///
/// A quarry has to be cut out of something and a quarry is the only source of
/// stone, which is what a dike costs — so a city with no rock within reach is
/// a city that cannot defend itself, decided by the map before anybody has
/// played a turn. That is the mistake `SHORE_DISTANCE` was written to stop
/// making, and moving the cities to the river bank made it again: rock is the
/// top eight percent of the height field and a river bank is low country, so
/// `probe::where_the_cities_sit` found 46% of two-player sites with no rock
/// inside forty cells and the worst a hundred away.
///
/// So the candidate band asks for rock as well as for water, and thirty cells
/// is the answer: a hauler's walk, not a day's march.
pub const QUARRY_REACH: i32 = 30;

pub const SITE_JITTER: i32 = 1;
pub const SITE_SNAP: i32 = 2;

/// How far off `SHORE_DISTANCE` a site may sit, either way.
///
/// This is the "comparable (not identical)" of design §6, and it is also what
/// makes the candidate band wide enough to choose from: a band one cell thick
/// on a meandering river is a few hundred cells and the spacing that can be
/// got out of it is poor. Three either way is about a fifth of the map's dry
/// area and costs nobody more than three cells of shoreline.
pub const SITE_JITTER_BAND: i32 = 4;

/// The plan asks for forty cells between cities. This is what the map gives.
///
/// Measured over two hundred seeds at each player count by
/// `map::probe::where_the_cities_sit`, with sites chosen farthest-point from
/// the band of cells `SHORE_DISTANCE` from the river — and used as the floor
/// that choice aims at, so the two cannot drift apart.
///
/// | players | shore parallel | river band |
/// |---|---|---|
/// | 2 | 108 | 36 |
/// | 3 | 51 | 18 |
/// | 4 | 31 | 18 |
/// | 5 | 22 | 18 |
/// | 6 | 17 | 18 |
///
/// Eighteen at every count above two, which is what a floor looks like when
/// the choice is aiming at it: the ranking stops buying distance once it has
/// enough and spends the rest on getting the city within reach of rock.
///
/// A two-player map lost most of a distance it had no use for, and a
/// six-player map gained a cell. The band is what costs the big numbers: a
/// line across the map has room to spread five cities along it, and a band
/// hugging a river does not. What the band buys is that every city is the same
/// distance from the water — see `SHORE_DISTANCE` — which is the thing that
/// decides whether a player is in the game at all.
pub const MIN_SITE_SPACING: i32 = 18;

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
/// **The stone was twenty times too little and nobody had checked.** It was
/// 120, "roughly four Cottages and a Granary plus three levels of Dike", which
/// is one dike cell built three levels high. `tests/playtest.rs` measures what
/// a wall has to be to change the outcome of a run — about thirty-four cells
/// along the shore — and at the old price that is 2 720 stone at two levels
/// and 5 440 at four. A city could afford one twentieth of one, so the flood
/// answer the whole design turns on could not be built at all, and the probe
/// had to put its wall up by fiat to measure anything.
///
/// So the price came down and the purse went up, to the point where a player
/// gets **one good wall in a run**: 720 stone buys seventy-two dike-levels,
/// which is thirty-six cells at two levels — a bank long enough to matter — or
/// eighteen cells at four. Nothing produces stone, so that is the whole run's
/// worth of it, and where to put it and how high to build it is the decision
/// the flood is asking. Farms cost ten stone each, which is a real bite out of
/// the same purse and is meant to be — and so, now, is the forty stone a
/// forester's hut costs. The two producers are bought with what the other one
/// makes: a city starts holding the stone and wanting the wood, so stone buys
/// the hut that cuts timber and wood buys the quarry that cuts stone. Before
/// that, both cost wood, which meant the wood shortage funded its own cure.
pub const STARTING_WOOD: u16 = 200;
pub const STARTING_STONE: u16 = 720;

/// How much of its materials a demolished or ruined building gives back
/// (design §3.3: "rubble returns a fraction of its materials"), as a percent.
pub const RUBBLE_REFUND_PERCENT: u16 = 50;

/// Builder-ticks one builder contributes per tick at full rest. A tired
/// citizen works at half speed (design §3.2).
pub const BUILDER_EFFORT: u32 = 1;

/// How many builders can crowd onto one construction site.
pub const BUILDER_SLOTS: usize = 4;

/// How much of a dike's build time raising it by a level costs, as a percent.
///
/// Raising a bank is adding a course to something that is already there, not
/// building it again — and the difference is what makes a level-two wall a
/// thing a city of eight can have. `playtest.rs` measured the alternative: at
/// full price a wall worth having takes six thousand builder-ticks, half the
/// city is on it for the whole age, two or three people are left farming, and
/// the city starves before the water arrives. Every `dike` run died in age
/// one, which is a wall that costs you the run whether or not it works.
///
/// Half. A dike raised to level two is one and a half dikes' work rather than
/// two, and the stone is unchanged — the earth is the same earth either way.
pub const DIKE_RAISE_PERCENT: u32 = 50;

/// A Dike level raises the effective ground by this much (design §3.3), and a
/// Dike can be built up to this many levels. Two levels stops an age-1 surge
/// of height 12 dead, which is the teaching moment in design §5.
pub const DIKE_HEIGHT_PER_LEVEL: u16 = 3;
pub const DIKE_MAX_LEVEL: u8 = 4;

/// How long one dike segment is, in cells. A wall of 1 x 1 blocks was a wall
/// you built cell by cell and a wall the water met one cell at a time; three
/// is the shortest run that has a middle, which is what lets a segment feel a
/// pressure that is not simply the cell in front of it.
pub const DIKE_LENGTH: i32 = 3;

// ---- getting about ---------------------------------------------------------

/// How far a citizen walks in a tick on open ground, in 256ths of a cell.
/// A quarter of a cell a tick is two and a half cells a second, which crosses
/// a city in a few seconds and the whole map in about a minute.
pub const WALK_SPEED: i32 = 64;

/// How close two citizens may stand, as a squared distance in `Fx`.
///
/// Design §3.2 draws a citizen as a body on a cell, and eight of them used to
/// occupy the same eighth of a cell at the hearth: one circle, with a number
/// of people inside it. Half a cell apart is enough to read as a crowd rather
/// than a smudge, and small enough that a three-slot farm still has room for
/// its three workers to stand at it.
pub const ELBOW_ROOM: Fx = Fx(128); // half a cell
pub const ELBOW_ROOM_SQ: Fx = Fx((128 * 128) >> 8);

/// How hard one tick pushes two people apart, in 1/256ths of a cell.
///
/// A twelfth of a cell: gentle enough that being jostled does not throw
/// anybody off a bridge, and firm enough that a knot of eight untangles in
/// about a second. Not a spring — a constant — because a spring needs a
/// division per pair and this runs on every citizen every tick.
pub const ELBOW_PUSH: Fx = Fx(21);

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

/// Worker-ticks per unit of wood and of stone.
///
/// Measured against what a city actually spends rather than picked. Two
/// workers at sixty-four ticks a unit make about thirty-seven wood a day,
/// which is a cottage and a bit; at ninety-six, about twenty-five stone, which
/// is two and a half dike levels. Six days of an age is then roughly two
/// buildings or fifteen dike cells from one hut and one quarry, with two of
/// your eight standing at each — so the shortage is real, the answer to it is
/// real, and manning both of them is most of a city.
///
/// `tests/city.rs::a_forester_and_a_quarry_pay_for_a_building_in_a_day` holds
/// the arithmetic to what the game actually does.
pub const FOREST_TICKS_PER_UNIT: u32 = 64;
pub const QUARRY_TICKS_PER_UNIT: u32 = 96;

/// How much a forester's hut or a quarry holds before its workers stop,
/// waiting for a hauler. Larger than a farm's because wood and stone are
/// carried in bursts to whatever is being built rather than trickling to a
/// granary every day.
pub const PRODUCER_BUFFER: u16 = 100;

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

// ---- what the water does to a wall -----------------------------------------

/// What standing water counts for, in the units `Water::speed_at` uses.
///
/// The plan said pressure on a dike is `depth * speed`, and measuring it said
/// that is zero exactly where a wall earns its keep. Water dammed by a wall
/// stops: `dike_pressure_on_flat_ground` found fifty-one sixteenths piled
/// against a level-one wall moving at a speed of *two*, so the product was
/// four hundred and it rounded to nothing. A dam is loaded by the depth it
/// holds, and the flow is what makes the leading edge worse than the pool
/// behind it — so speed is a term that adds, not a factor that gates.
///
/// Set to the speed at which moving water doubles the push of standing water,
/// which is around the flow the surge front actually carries.
pub const STILL_PUSH: u32 = 16;

/// Pressure on one cell of a dike's wet side is `depth * (STILL_PUSH + speed)`,
/// and the three cells of the side are summed. That product is large — a deep,
/// fast cell is tens of thousands — so it is divided by this before it is
/// added to `Building::stress`, which keeps a `u32` accumulator good for
/// thousands of ticks of the worst flow the game can produce.
///
/// The division rounds down, so a side carrying less than this in total adds
/// nothing at all. That is the intended shape and not a rounding bug: a wall
/// is not worn away by an inch of water leaning on it.
pub const PRESSURE_SCALE: u32 = 256;

/// Stress a dike sheds each tick with no water against it.
///
/// Design's point, and the plan's: a dike that survives one surge is weaker
/// for the next but not doomed. The gap between floods is an age, which is
/// `DAYS_PER_AGE * TICKS_PER_DAY` = 7 200 ticks, so this is the number that
/// decides whether "weaker for the next" means anything at all. At two a tick
/// a wall sheds 14 400 over an age: a level-one segment (6 000) is clear again
/// well before the next flood and a level-two one that came through at
/// nine-tenths of its limit meets the next surge still carrying two fifths of
/// the last. That is the shape design §5 wants; the exact number is M5's.
pub const STRESS_RELIEF: u32 = 2;

/// How much a dike's own footing may vary from the book figure, as a percent
/// either way.
///
/// No two stretches of bank are alike, and without this they are: a hard
/// threshold sitting in the middle of the load distribution is exactly where
/// the fraction broken is most sensitive to the load, so the same rule gave
/// 67% of a level-one wall gone on one seed and 93% on another —
/// `which_dikes_break` prints the spread. Making the *wall* vary by more than
/// the flood does between maps turns a knife-edge into a slope, which is what
/// makes "a lot of them break and not all of them" a thing that can be aimed
/// at rather than a coincidence.
///
/// Drawn once, when the segment is placed, and kept on the building — so it is
/// in the checksum and the snapshot like everything else, and two peers cannot
/// disagree about which wall was the weak one.
pub const FOOTING_SPREAD: u8 = 25;

/// What a dike can take before it is rubble, in scaled pressure-ticks, by
/// level.
///
/// **Measured against the river**, by `dikes::which_dikes_break`, which walls
/// both banks at three distances across ten seeds with segments alternating
/// between level one and level two and reports what the flood takes. The plan
/// asks for a fraction rather than a rule — "a lot of level one dikes break
/// and not all of them; many level twos hold" — so the target is a band:
/// 60–80% of level one gone at age one, 70–90% of level two standing, and both
/// worse by age three.
///
/// | from the channel | age 1: L1 gone / L2 gone | age 3: L1 gone / L2 gone |
/// |---|---|---|
/// | 6 cells  | 82% / 40% | 89% / 65% |
/// | 12 cells | 73% / 16% | 83% / 34% |
/// | 20 cells | 59% / 7%  | 75% / 18% |
/// | all      | **71% / 21%** | 82% / 39% |
///
/// So at age one 71% of a level-one wall is gone and 79% of a level-two wall
/// is standing — both in the middle of the target — and by age three it is 82%
/// and 61%. The gradient with distance is the part worth looking at: a wall on
/// the bank is nearly all taken and one twenty cells back is nearly all left,
/// which is the choice the drag tool is for.
///
/// Seven seeds in ten hit both bands. The plan asked for eight, and the three
/// that miss are maps whose flood is unusually weak or strong rather than
/// walls that behave oddly — `[20_000, 55_000, …]`, `[16_000, 52_000, …]` and
/// `[14_000, 50_000, …]` all measured seven as well, so the residual is the
/// spread between maps and not the choice of number. Narrowing it means
/// normalising the flood between seeds, and two attempts at that are recorded
/// in DECISIONS.md — holding the surge's surface instead of its depth (kept,
/// it is the better model) and capping the water on the ground (dropped, it
/// did not move the outliers).
pub const DIKE_STRESS_LIMIT: [u32; DIKE_MAX_LEVEL as usize] =
    [15_000, 48_000, 90_000, 145_000];

/// What a dike of this level can take. Levels are 1-based.
pub fn dike_stress_limit(level: u8) -> u32 {
    let i = (level.max(1) as usize - 1).min(DIKE_STRESS_LIMIT.len() - 1);
    DIKE_STRESS_LIMIT[i]
}

/// How many cells of the channel, from its upstream mouth, the source holds.
///
/// **This used to be `SURGE_SIZE`, the side of the 8 x 8 block at a corner.**
/// A river has no corner to fill, so the source is a reach: the first
/// `SURGE_REACH` cells of the centreline and the whole width of its cut held
/// at the age's height, and the next `SURGE_REACH` held at half of it. That
/// second reach is the pump — it is what makes a front rather than a puddle,
/// and it is the same trick the corner version used one block inland.
///
/// Forty, measured by `playtest::when_the_water_arrives`, which reports what
/// the water does at each city's hearth:
///
/// | reach | age 1 peak | age 3 peak | wades at | dry again |
/// |---|---|---|---|---|
/// | 20 | 25-43 | 35-71 | 115-800, sometimes never | 465-2530 |
/// | 40 | 72-82 | 105-115 | 71-137 | 1380-3079 |
/// | 80 | 79-125 | 125-171 | 55-135 | 1622-3835 |
///
/// Wading starts at 32 and swimming at 96. At twenty the flood is a damp
/// patch and two cities in six never got their feet wet; at eighty an age-one
/// flood is already over your head, which leaves the escalation nowhere to go.
/// Forty is the one where age one wets you and age three drowns you, which is
/// design §4's table read back out of the water.
///
/// Eighty cells of a hundred-and-thirty-cell channel are held in all, so the
/// flood comes down the upper two thirds of the river and the lower third is
/// what it runs off through.
pub const SURGE_REACH: i32 = 40;

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
