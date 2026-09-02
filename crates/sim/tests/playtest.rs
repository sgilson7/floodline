//! Design step 7: "playtest the flood until it is fun."
//!
//! This is not that. A person has to decide whether a game is fun and no test
//! can, and `PROGRESS.md` says plainly that nobody has played FLOODLINE yet.
//! What this file does is the part that *can* be settled without a person:
//! whether the decisions a player is being asked to make have different
//! outcomes. A game where the careful player and the idle one both live, or
//! both die, is not unbalanced — it is not a game, and no amount of judgement
//! about how it feels will fix it.
//!
//! Four strategies, played through `World::apply` and nothing else, on
//! generated terrain across several seeds, for a full three-age run:
//!
//! * **idle** — found a city and do nothing. The floor.
//! * **grow** — cottages, a farm and a granary, citizens assigned. No dike.
//! * **dike** — grow, and put a wall between the city and the low corner.
//! * **flee** — grow, and run uphill when the elders get uneasy.
//!
//! Run it, with the numbers:
//!
//!     cargo test -p sim --release --test playtest -- --ignored --nocapture

use sim::balance::*;
use sim::building::{BuildState, Facing, Kind};
use sim::citizen::{CitizenId, PlayerId};
use sim::command::Command;
use sim::map::{Map, MAP_H, MAP_W};
use sim::nav::Nav;
use sim::world::World;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Play {
    Idle,
    Grow,
    Dike,
    Flee,
    /// A wall *and* the sense to get behind it. What a player who has been
    /// through one flood would do.
    Both,
}

impl Play {
    const ALL: [Play; 5] = [Play::Idle, Play::Grow, Play::Dike, Play::Flee, Play::Both];

    fn name(self) -> &'static str {
        match self {
            Play::Idle => "idle",
            Play::Grow => "grow",
            Play::Dike => "dike",
            Play::Flee => "flee",
            Play::Both => "both",
        }
    }

    fn builds(self) -> bool {
        self != Play::Idle
    }
}

/// What one run was like, age by age.
struct Report {
    seed: u64,
    play: Play,
    /// Population at the end of each age.
    alive: Vec<u32>,
    /// Deepest water anywhere within fifteen cells of the hearth, in
    /// sixteenths. Measured over the quarter rather than at the one cell,
    /// because a city is a neighbourhood and its people walk about in it.
    soaked: Vec<u16>,
    /// How far the hearth is from the corner the water comes out of.
    from_the_corner: i32,
    /// Buildings standing at the end of each age.
    standing: Vec<usize>,
    /// How many dike cells were actually built, and what they would have cost
    /// a player who had to pay for them.
    wall: usize,
    wall_cost: u16,
    /// Water at the hearth itself, deepest in each age.
    at_the_fire: Vec<u16>,
    /// Stone in hand on the day the water came.
    stone_on_the_day: Vec<u16>,
    /// Children born, children who reached `COMING_OF_AGE`, and adult-ticks
    /// those grown children actually worked. M12.10's question.
    born: u32,
    came_of_age: u32,
    adult_ticks: u32,
    households: u32,
    settled: u32,
    homed: u32,
    nurseries: u32,
    spare_beds: u32,
    toward: u32,
    made: u32,
    ages: u32,
    /// How everybody who died, died. Read off the body the way the panel's
    /// toll reads it: `drowning_for` above nought is water, an empty stomach
    /// is hunger.
    drowned: u32,
    starved: u32,
    otherwise: u32,
}

/// One run of the table: a seed, a strategy, and whatever it was handed.
///
/// A struct rather than four arguments, because two of them are `bool` and
/// `play(seed, p, false, true)` is a call nobody can read.
#[derive(Copy, Clone)]
struct Run {
    seed: u64,
    play: Play,
    /// A finished level-two wall on the line `order_a_wall` draws, given free
    /// on the day the paid strategies order theirs. **Not something a player
    /// can have.** It is here to separate the two questions four playtests
    /// have run together: whether a finished wall changes the outcome of a
    /// flood, and whether a city of eight can get one up.
    wall_by_fiat: bool,
    /// Print a line a day.
    diary: bool,
}

impl Run {
    fn new(seed: u64, play: Play) -> Run {
        Run { seed, play, wall_by_fiat: false, diary: false }
    }
    fn walled(mut self) -> Run {
        self.wall_by_fiat = true;
        self
    }
    fn watched(mut self) -> Run {
        self.diary = true;
        self
    }
}

const ME: PlayerId = PlayerId(0);

/// A cell for `kind` near `(hx, hy)`, spiralling out until one is legal.
fn spot(w: &World, kind: Kind, hx: i32, hy: i32) -> Option<(i32, i32)> {
    for r in 3..30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if w.can_place(ME, kind, Facing::EastWest, hx + dx, hy + dy).is_ok() {
                    return Some((hx + dx, hy + dy));
                }
            }
        }
    }
    None
}

/// The deepest water in the city's quarter, not just under the fire.
fn deepest_near(w: &World, hx: i32, hy: i32) -> u16 {
    let mut deepest = 0;
    for dy in (-15..=15).step_by(3) {
        for dx in (-15..=15i32).step_by(3) {
            deepest = deepest.max(w.water.depth_at(hx + dx, hy + dy));
        }
    }
    deepest
}

fn play(run: Run) -> Report {
    let Run { seed, play, .. } = run;
    let mut w = World::new(seed, 2);
    let mut nav = Nav::new();
    let (hx, hy) = w.map.hearth_sites[0];

    let mut report = Report {
        seed,
        play,
        alive: Vec::new(),
        soaked: Vec::new(),
        from_the_corner: (hx - w.map.low_corner.cell().0).abs()
            + (hy - w.map.low_corner.cell().1).abs(),
        standing: Vec::new(),
        wall: 0,
        wall_cost: 0,
        at_the_fire: Vec::new(),
        stone_on_the_day: Vec::new(),
        born: 0,
        came_of_age: 0,
        adult_ticks: 0,
        households: 0,
        settled: 0,
        homed: 0,
        nurseries: 0,
        spare_beds: 0,
        toward: 0,
        made: 0,
        ages: 0,
        drowned: 0,
        starved: 0,
        otherwise: 0,
    };

    // What the strategy intends to put down, in order, one per day. Spread out
    // rather than all at once because materials are hauled and built by the
    // same eight people, and a city that orders six buildings on day one has
    // six half-built sites when the water arrives.
    // Food first, and a granary the same day: a citizen can only eat at a
    // granary, and the founding party's own food empties inside one day.
    //
    // **The `grow` plan grows, since M12.10.** It was
    // `[Farm, Granary, Cottage, Cottage, Farm]` for every strategy that builds
    // anything - no nursery, and nobody ever put two people in one cottage. So
    // across every seed and every run of this table, the column called "grow"
    // produced **nought children**, and what it actually measured was a city
    // with two spare cottages. The same fault as `how_a_city_grows`, one
    // milestone later and in the other direction: that probe arranged the
    // condition it was measuring, and this one never arranged it at all.
    let plan: Vec<Kind> = match play {
        p if !p.builds() => Vec::new(),
        Play::Grow | Play::Both => vec![
            Kind::Farm,
            Kind::Granary,
            Kind::Cottage,
            Kind::Cottage,
            Kind::Nursery,
            // A *third* cottage, and it is the finding rather than a tweak.
            // Citizens take a bed to sleep in whether or not anybody told them
            // to, so eight people fill two four-bed cottages exactly - and
            // `have_children` needs a spare bed. A city that had housed
            // everybody was a city that could not have a child, and nothing
            // anywhere says so.
            Kind::Cottage,
            Kind::Farm,
        ],
        _ => vec![Kind::Farm, Kind::Granary, Kind::Cottage, Kind::Cottage, Kind::Farm],
    };
    let mut placed = 0usize;
    let mut last_day = u32::MAX;
    let mut diked = false;
    let mut fled = false;
    let mut returned = false;
    let mut deepest_this_age = 0u16;
    let mut at_the_fire = 0u16;

    while w.finished().is_none() && w.tick < MAX_AGE * DAYS_PER_AGE * TICKS_PER_DAY + 5000 {
        let day = w.day();
        if day != last_day {
            last_day = day;

            // One building a day, and everybody who is not already at a farm
            // put to work at the newest thing that will have them.
            // Two on the first day, because the farm and the granary are both
            // needed before the founding party's own food runs out.
            let want = if day <= 1 { 2 } else { 1 };
            for _ in 0..want {
                if placed >= plan.len() {
                    break;
                }
                let kind = plan[placed];
                if let Some((x, y)) = spot(&w, kind, hx, hy) {
                    if w.apply(ME, &Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 }).is_ok() {
                        placed += 1;
                    }
                }
            }
            // Farms first, wall second. The other way round was tried and
            // measured: builders got first pick, the fields went short, and on
            // two seeds in three the city starved before the water arrived
            // with no food at the hearth at all. A player builds a wall out of
            // the hands the harvest can spare, not the other way about.
            assign_to_farms(&mut w);
            if play == Play::Grow || play == Play::Both {
                pair_them_up(&mut w);
            }
            if play == Play::Dike || play == Play::Both {
                man_the_wall(&mut w);
            }
            if run.diary {
                diary(&w, hx, hy);
            }
        }

        // The wall is ordered on the second day, after the farm and the
        // granary, and raised on the third — early enough that the second
        // level is standing before the water comes rather than half-built
        // when it does.
        // "After the farm and the granary" was what the comment said and not
        // what the code did: the wall went down on the second day whatever
        // else was half-built, and the builders it took were the ones that
        // would have finished the farm. Two seeds in three then had no food at
        // the hearth in any age and the city starved before the water came.
        let fed = |w: &World| {
            let up = |k: Kind| {
                w.buildings.iter().any(|b| b.owner == ME && b.kind == k && b.standing_now())
            };
            up(Kind::Farm) && up(Kind::Granary)
        };
        if (play == Play::Dike || play == Play::Both) && !diked && day >= 2 && fed(&w) {
            diked = true;
            report.wall = order_a_wall(&mut w, hx, hy);
        }
        // The same wall, on the same day, for nothing. Nobody is taken off a
        // farm and no stone leaves the store, so what this measures is the
        // wall itself rather than what a city gives up for it.
        if run.wall_by_fiat && !diked && day >= 2 && fed(&w) {
            diked = true;
            report.wall = order_a_wall(&mut w, hx, hy);
            finish_the_wall(&mut w, 2);
        }
        if (play == Play::Dike || play == Play::Both) && diked && day >= 3 {
            // Two, and no higher: `which_dikes_break` says level two is where
            // a wall starts holding, and stone spent on a third level is stone
            // not spent on more wall.
            raise_the_wall(&mut w, 2);
        }

        // Uphill, on the omen. It arrives a day before the water (design §4)
        // and is the only warning there is.
        // On the impact day itself, not on the day's warning: holding people
        // off the fields for a whole extra day costs more in hunger than the
        // water takes.
        if (play == Play::Flee || play == Play::Both)
            && !fled
            && w.day_of_age() == World::IMPACT_DAY
        {
            fled = true;
            let (tx, ty) = high_ground(&w.map, hx, hy);
            let citizens: Vec<CitizenId> =
                w.citizens.iter().filter(|c| c.owner == ME && c.alive()).map(|c| c.id).collect();
            let _ = w.apply(ME, &Command::MoveTo { citizens, x: tx as u8, y: ty as u8 });
        }
        // And back to work once the water has gone. Without this the strategy
        // is not "run uphill", it is "abandon the city": held citizens do not
        // farm, and a city that farms nobody for an age starves — which is a
        // true thing about the game and not the thing being measured here.
        if (play == Play::Flee || play == Play::Both)
            && fled
            && !returned
            // Once the first surge has poured and had a while to drain.
            //
            // The number is a dilemma, not a constant. Waiting until age
            // three's *second* corner has also poured — half a day later —
            // means holding everybody off the fields for most of a day, and
            // three runs of that starve the city in age two before the water
            // ever gets a chance at them. Coming home at this one survives
            // ages one and two and walks into the second surge in age three.
            // A person watching the water would come home when their own
            // street drained; a fixed number cannot, which is the honest limit
            // of measuring this without a player.
            && w.tick % TICKS_PER_DAY > SURGE_TICKS + 500
        {
            returned = true;
            let citizens: Vec<CitizenId> =
                w.citizens.iter().filter(|c| c.owner == ME && c.alive()).map(|c| c.id).collect();
            let _ = w.apply(ME, &Command::Unassign { citizens });
        }

        let age_before = w.age();
        let children_before = w
            .citizens
            .iter()
            .filter(|c| c.owner == ME && c.alive() && c.is_child())
            .count();
        let people_before =
            w.citizens.iter().filter(|c| c.owner == ME && c.alive()).count();
        w.tick(&mut nav, &[]);
        // Births and comings-of-age, counted by watching rather than by
        // instrumenting `sim`. A birth adds a citizen who is a child; a coming
        // of age turns one into an adult without the count moving.
        let children_now = w
            .citizens
            .iter()
            .filter(|c| c.owner == ME && c.alive() && c.is_child())
            .count();
        let people_now = w.citizens.iter().filter(|c| c.owner == ME && c.alive()).count();
        report.born += people_now.saturating_sub(people_before) as u32;
        // Fewer children with no fewer people is somebody who grew up.
        if children_now < children_before && people_now >= people_before {
            report.came_of_age += (children_before - children_now) as u32;
        }
        report.adult_ticks += report.came_of_age;
        report.households = report.households.max(
            w.households.iter().filter(|h| h.owner == ME && h.alive()).count() as u32,
        );
        report.settled = report.settled.max(
            w.households.iter().filter(|h| h.owner == ME && h.alive() && h.settled()).count() as u32,
        );
        report.made = report.made.max(w.households.len() as u32);
        report.toward = report.toward.max(
            w.households
                .iter()
                .filter(|h| h.owner == ME && h.alive())
                .map(|h| h.toward_child)
                .max()
                .unwrap_or(0),
        );
        report.nurseries = report.nurseries.max(
            w.buildings
                .iter()
                .filter(|b| b.owner == ME && b.kind == Kind::Nursery && b.standing_now())
                .count() as u32,
        );
        report.spare_beds = report.spare_beds.max({
            let beds: usize = w
                .buildings
                .iter()
                .filter(|b| b.owner == ME && b.kind == Kind::Cottage && b.standing_now())
                .map(|b| b.beds())
                .sum();
            let homed =
                w.citizens.iter().filter(|c| c.owner == ME && c.alive() && c.home.is_some()).count();
            beds.saturating_sub(homed) as u32
        });
        report.homed = report.homed.max(
            w.citizens.iter().filter(|c| c.owner == ME && c.alive() && c.home.is_some()).count()
                as u32,
        );
        deepest_this_age = deepest_this_age.max(deepest_near(&w, hx, hy));
        at_the_fire = at_the_fire.max(w.water.depth_at(hx, hy));
        // Stone in hand, once an age. The wall, once a run — **on the first
        // flood, which is what this has always claimed and not what it did.**
        // The guard put one figure in the list per age and then overwrote
        // `wall_cost` every time, so "the wall at the age-one flood" was the
        // wall left standing after the age-three one: on seed 31 the `dike`
        // script read sixty stone, meaning one segment, when what it had on the
        // day that mattered was three hundred.
        if w.day_of_age() == World::IMPACT_DAY && report.stone_on_the_day.len() < w.age() as usize {
            let first_flood = report.stone_on_the_day.is_empty();
            report.stone_on_the_day.push(w.treasury(ME).stone);
            if first_flood {
                report.wall_cost = w
                    .buildings
                    .iter()
                    .filter(|b| b.owner == ME && b.kind == Kind::Dike && b.standing_now())
                    .map(|b| b.level as u16 * Kind::Dike.cost().stone)
                    .sum();
            }
        }
        if w.age() != age_before || w.finished().is_some() {
            report.alive.push(w.population(ME));
            report.soaked.push(deepest_this_age);
            report.at_the_fire.push(at_the_fire);
            at_the_fire = 0;
            // **Not counting the wall's own segments.** A walled run has eleven
            // more buildings than a bare one the moment the wall is up, so
            // `standing` was measuring the wall rather than what the wall
            // saved.
            report.standing.push(
                w.buildings
                    .iter()
                    .filter(|b| b.owner == ME && b.standing_now() && b.kind != Kind::Dike)
                    .count(),
            );
            deepest_this_age = 0;
            fled = false;
        }
    }
    report.ages = w.score().ages_survived;
    // Read off the bodies, the way `Input::mind_the_dead` reads them: nothing
    // new in `sim`, and the same answer a player is given on the screen.
    for c in w.citizens.iter().filter(|c| c.owner == ME && !c.alive()) {
        if c.drowning_for > 0 {
            report.drowned += 1;
        } else if c.food == 0 {
            report.starved += 1;
        } else {
            report.otherwise += 1;
        }
    }
    report
}

/// One line a day: what the city has, who is on what, and how the wall is
/// coming along.
///
/// The strategy table says a walling city dies on two seeds in three and says
/// nothing about what it died of. This is that.
fn diary(w: &World, hx: i32, hy: i32) {
    let mine = |f: &dyn Fn(&sim::Citizen) -> bool| {
        w.citizens.iter().filter(|c| c.owner == ME && c.alive() && f(c)).count()
    };
    let dikes: Vec<&sim::Building> =
        w.buildings.iter().filter(|b| b.owner == ME && b.kind == Kind::Dike).collect();
    let standing = dikes.iter().filter(|b| b.standing_now()).count();
    let owed: u16 = dikes.iter().map(|b| b.outstanding().stone).sum();
    let effort: u32 = dikes
        .iter()
        .filter(|b| !b.standing_now())
        .map(|b| b.progress * 100 / Kind::Dike.build_ticks().max(1))
        .sum();
    let sites = dikes.len() - standing;
    let store = w.treasury(ME);
    // **Where the stone went.** Nothing in this plan quarries, so a city has
    // `STARTING_STONE` and that is all the stone there will ever be: whatever
    // is not in a store, in somebody's arms or delivered to a site has left
    // the game. `Citizen::abandon` clears `carrying`, and hunger and
    // exhaustion both call it.
    let hand = w.in_hand(ME);
    let delivered: u16 =
        w.buildings.iter().filter(|b| b.owner == ME).map(|b| b.delivered.stone).sum();
    let lost = STARTING_STONE
        .saturating_sub(store.stone)
        .saturating_sub(hand.stone)
        .saturating_sub(delivered);
    // Where the day goes. The wall was rising at twenty stone a day with five
    // builders on it, which the length of the walk does not explain, so count
    // the errands rather than reason about them.
    let after_food = mine(&|c| {
        matches!(c.errand, Some(sim::citizen::Errand::Collect { good: sim::building::Good::Food, .. }))
            || c.carrying.food > 0
    });
    let after_stone = mine(&|c| {
        matches!(c.errand, Some(sim::citizen::Errand::Collect { good: sim::building::Good::Stone, .. }))
            || c.carrying.stone > 0
    });
    let resting = mine(&|c| {
        matches!(
            c.errand,
            Some(sim::citizen::Errand::ToEat(_)) | Some(sim::citizen::Errand::ToSleep(_))
        ) || c.state == sim::citizen::State::Eating
            || c.state == sim::citizen::State::Sleeping
    });
    println!(
        "  age {} day {}  {} alive  food {:>3}  stone {:>3}+{:<3} {:>3} gone  \
         {} farm {} build {} idle  [{} fetching food, {} fetching stone, \
         {} eating or asleep]  wall {}/{} up, {} owed, {}% done  water {}",
        w.age(),
        w.day_of_age(),
        w.population(ME),
        store.food,
        store.stone,
        hand.stone,
        lost,
        mine(&|c| c.job == Some(sim::citizen::Job::Farmer)),
        mine(&|c| c.job == Some(sim::citizen::Job::Builder)),
        mine(&|c| c.job.is_none()),
        after_food,
        after_stone,
        resting,
        standing,
        dikes.len(),
        owed,
        if sites == 0 { 0 } else { effort as usize / sites },
        deepest_near(w, hx, hy),
    );
}

/// Hand the city a finished wall: every segment it has ordered, raised to
/// `levels` and built, with nobody carrying to it and nothing paid for it.
///
/// The same fiat `dikes.rs` uses in `wall_the_bank`, and for the same reason
/// stated there — mixing "does the water take it down" with "can anybody
/// afford it" is how a threshold gets tuned against a shortage of stone.
fn finish_the_wall(w: &mut World, levels: u8) {
    let ids: Vec<sim::BuildingId> = w
        .buildings
        .iter()
        .filter(|b| b.owner == ME && b.kind == Kind::Dike)
        .map(|b| b.id)
        .collect();
    for id in ids {
        for _ in 0..levels {
            let owed = w.buildings[id.0 as usize].outstanding().stone;
            w.deliver_to(id, sim::building::Good::Stone, owed);
            w.build_at(id, Kind::Dike.build_ticks());
            if w.buildings[id.0 as usize].level < levels {
                let _ = w.raise_dike(ME, id);
            }
        }
    }
}

/// Fill every farm's slots from whoever has no job, which is what a player
/// does after each farm goes up. Nothing else needs assigning: a city finishes
/// its own building sites, and hauling is what an unassigned citizen does.
/// Put half the city on the wall while any of it is unfinished.
///
/// **Ordering a wall is not building one.** The first version of this strategy
/// placed eleven segments and then sent everybody back to the fields, and on
/// the impact day one segment in seven was standing and the rest were heaps of
/// delivered stone — so what the playtest was actually measuring was a city
/// that had paid for a wall and did not have one. A player who has decided to
/// build a wall puts people on it, and half the city is the trade: the other
/// half still has to feed everybody, which is the cost the decision is
/// supposed to have.
fn man_the_wall(w: &mut World) {
    let sites: Vec<sim::BuildingId> = w
        .buildings
        .iter()
        .filter(|b| b.owner == ME && b.kind == Kind::Dike && b.state == BuildState::Site)
        .map(|b| b.id)
        .collect();
    let mut hands = FOUNDING_CITIZENS / 2;
    for site in sites {
        if hands == 0 {
            return;
        }
        let free: Vec<CitizenId> = w
            .citizens
            .iter()
            .filter(|c| c.owner == ME && c.alive() && c.job.is_none())
            .map(|c| c.id)
            .take(hands.min(BUILDER_SLOTS as u32) as usize)
            .collect();
        if free.is_empty() {
            return;
        }
        hands -= free.len() as u32;
        let _ = w.apply(ME, &Command::Assign { citizens: free, building: site });
    }
}

fn assign_to_farms(w: &mut World) {
    let farms: Vec<sim::BuildingId> = w
        .buildings
        .iter()
        .filter(|b| b.owner == ME && b.standing_now() && b.kind == Kind::Farm)
        .map(|b| b.id)
        .collect();
    for building in farms {
        let slots = Kind::Farm.slots_for(sim::citizen::Job::Farmer);
        let held = w.buildings[building.0 as usize].workers.len();
        if held >= slots {
            continue;
        }
        // Two of the eight stay unassigned whatever happens: somebody has to
        // carry the harvest to the granary, and a city where everybody farms
        // starves beside a full farm.
        let free: Vec<CitizenId> = w
            .citizens
            .iter()
            .filter(|c| c.owner == ME && c.alive() && c.job.is_none())
            .map(|c| c.id)
            .collect();
        let spare = free.len().saturating_sub(2);
        let take: Vec<CitizenId> = free.into_iter().take(spare.min(slots - held)).collect();
        if take.is_empty() {
            continue;
        }
        let _ = w.apply(ME, &Command::Assign { citizens: take, building });
    }
}

/// A wall between the city and the river — *ordered*, not conjured.
///
/// **This used to wall a diagonal against a corner**, because the flood used
/// to come out of one. It does not any more: the water comes down the channel
/// and spills over its banks, so what stops it is a bank across the ground
/// between the river and the city, running the same way the river does.
///
/// One straight `DikeLine`, which is what the drag tool draws and what a
/// player would draw looking at the map. Every segment is hauled and built by
/// the same eight people who are farming, out of the same stone the city
/// started with. The first version of this built the wall by fiat, which was
/// the right way to find out whether a wall in the right place changes the
/// outcome and no way at all to find out whether anybody could have one.
fn order_a_wall(w: &mut World, hx: i32, hy: i32) -> usize {
    let dikes = |w: &World| {
        w.buildings.iter().filter(|b| b.owner == ME && b.kind == Kind::Dike).count()
    };
    let before = dikes(w);

    let river: Vec<(i32, i32)> =
        w.map.river.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
    let at = river
        .iter()
        .enumerate()
        .min_by_key(|(_, &(x, y))| (hx - x).abs().max((hy - y).abs()))
        .map(|(i, _)| i as i32)
        .expect("every map has a river");
    let (rx, ry) = river[at as usize];

    // Along the channel here, and out from it toward the city.
    let a = river[(at - 4).clamp(0, river.len() as i32 - 1) as usize];
    let b = river[(at + 4).clamp(0, river.len() as i32 - 1) as usize];
    let (tx, ty) = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    let (ox, oy) = ((hx - rx).signum(), (hy - ry).signum());

    // A third of the way from the bank to the city: close enough to be the
    // bank rather than a fence round the houses, far enough not to be in the
    // water.
    let out = ((hx - rx).abs().max((hy - ry).abs()) / 3).max(3);
    let (cx, cy) = (rx + ox * out, ry + oy * out);
    // **Short, and then raised**, which is not what this used to do.
    //
    // A forty-cell wall at level one is what the first version ordered, and
    // `which_dikes_break` is the reason it stopped: at level one the water
    // takes four segments in five, so a long low wall is a whole city's labour
    // spent on something that will not be there when it is wanted. Half the
    // length costs half the builder-ticks and leaves stone for the second
    // level, which is the one that holds. The wall is the decision; how to
    // spend on it is the decision inside the decision.
    let half = 20;
    let from = (
        (cx - tx * half).clamp(0, MAP_W - 1) as u8,
        (cy - ty * half).clamp(0, MAP_H - 1) as u8,
    );
    let to = (
        (cx + tx * half).clamp(0, MAP_W - 1) as u8,
        (cy + ty * half).clamp(0, MAP_H - 1) as u8,
    );
    let _ = w.apply(ME, &Command::DikeLine { from, to });

    dikes(w) - before
}

/// Raise every finished dike toward `want`, while there is stone for it.
///
/// Called every day rather than once, because raising a dike puts it back to a
/// site and a dike ordered on the second day is not standing on the third. The
/// first version raised once, on the fourth, and measured a wall that was
/// still level one when the water came — which `which_dikes_break` says is a
/// wall the water takes four segments in five of.
fn raise_the_wall(w: &mut World, want: u8) {
    let standing: Vec<sim::BuildingId> = w
        .buildings
        .iter()
        .filter(|b| {
            b.owner == ME && b.kind == Kind::Dike && b.standing_now() && b.level < want
        })
        .map(|b| b.id)
        .collect();
    for dike in standing {
        if w.treasury(ME).stone < Kind::Dike.cost().stone {
            return;
        }
        let _ = w.apply(ME, &Command::RaiseDike { dike });
    }
}

/// The highest cell within thirty of the hearth: where a city runs to.
fn high_ground(map: &Map, hx: i32, hy: i32) -> (i32, i32) {
    let mut best = (hx, hy);
    let mut best_h = map.height_at(hx, hy);
    for dy in -30..=30i32 {
        for dx in -30..=30i32 {
            let (x, y) = (hx + dx, hy + dy);
            if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H {
                continue;
            }
            if map.height_at(x, y) > best_h {
                best_h = map.height_at(x, y);
                best = (x, y);
            }
        }
    }
    best
}

#[test]
#[ignore = "a measurement, not an assertion: run it with --nocapture"]
fn three_full_runs_of_each_strategy() {
    const SEEDS: [u64; 3] = [31, 1_000_003, 0xF100_D11E];
    println!();
    println!(
        "  seed        play   ages  alive by age   at the hearth     in the quarter      wall   stone  out"
    );
    let mut reports = Vec::new();
    for seed in SEEDS {
        for p in Play::ALL {
            let r = play(Run::new(seed, p));
            println!(
                "  {:<11} {:<6} {:>4}   {:<13} {:<17} {:<17} {:>5} {:>6} {:>4}",
                r.seed,
                r.play.name(),
                r.ages,
                format!("{:?}", r.alive),
                format!("{:?}", r.at_the_fire),
                format!("{:?}", r.soaked),
                r.wall,
                r.wall_cost,
                r.from_the_corner,
            );
            reports.push(r);
        }
        println!();
    }

    let total = |p: Play| -> u32 {
        reports.iter().filter(|r| r.play == p).map(|r| r.alive.last().copied().unwrap_or(0)).sum()
    };
    let paid: Vec<u16> = reports.iter().filter(|r| r.wall_cost > 0).map(|r| r.wall_cost).collect();
    if let Some(&most) = paid.iter().max() {
        println!(
            "  the tallest wall anybody got built by the age-one flood cost {most} stone \
             of the {STARTING_STONE} a city starts with, and nothing in the MVP makes more."
        );
        println!();
    }
    println!("  survivors across all seeds:");
    for p in Play::ALL {
        println!("    {:<6} {}", p.name(), total(p));
    }
    println!();
}

/// How deep the water gets, at each distance from the corner it comes out of.
///
/// The number `SITE_RING_OFFSET` is chosen from. A city has to sit where an
/// age-one flood is a threat it can answer and an age-three flood is a threat
/// it might not — and "where" is a distance, which nobody had measured.
///
///     cargo test -p sim --release --test playtest reach -- --ignored --nocapture
/// When the wave reaches each city and when it has gone again.
///
/// The other half of what M4 replaced: the old table said how *deep* the water
/// got at a distance from the corner it came out of, and said nothing about
/// when. A river delivers its flood along its whole length rather than from one
/// point, so the interesting question stopped being "how far" and became "how
/// long have I got, and how long does it last".
#[test]
#[ignore]
fn when_the_water_arrives() {
    println!();
    println!("  seed        city   at the hearth: wades at   peak (tick)   dry again");
    for seed in [31u64, 1_000_003, 4_043_362_590] {
        for height in [12u16, 18] {
            let mut w = World::new(seed, 2);
            w.disaster.height = height;
            w.tick = (World::IMPACT_DAY - 1) * TICKS_PER_DAY;

            let cities: Vec<(i32, i32)> = w.map.hearth_sites.clone();
            let mut wades = vec![None; cities.len()];
            let mut peak = vec![(0u16, 0u32); cities.len()];
            let mut dry = vec![None; cities.len()];

            for t in 0..(SURGE_TICKS + 3 * TICKS_PER_DAY) {
                w.step_water();
                w.tick += 1;
                for (i, &(x, y)) in cities.iter().enumerate() {
                    let d = w.water.depth_at(x, y);
                    if d >= WADE_DEPTH {
                        if wades[i].is_none() {
                            wades[i] = Some(t);
                        }
                        dry[i] = None;
                    } else if wades[i].is_some() && dry[i].is_none() {
                        dry[i] = Some(t);
                    }
                    if d > peak[i].0 {
                        peak[i] = (d, t);
                    }
                }
            }

            for i in 0..cities.len() {
                println!(
                    "  {seed:>10}   h{height:<3} {i}   {:>12}   {:>4} ({:>4})   {:>9}",
                    wades[i].map(|t| t.to_string()).unwrap_or_else(|| "never".into()),
                    peak[i].0,
                    peak[i].1,
                    dry[i].map(|t| t.to_string()).unwrap_or_else(|| "still wet".into()),
                );
            }
        }
        println!();
    }
    println!("  wading starts at {WADE_DEPTH}, swimming at {SWIM_DEPTH}.");
}

#[test]
#[ignore = "a measurement, not an assertion: run it with --nocapture"]
fn how_far_the_water_reaches() {
    const SEEDS: [u64; 2] = [31, 1_000_003];
    // Cells from the river bank, not from a corner. `SHORE_DISTANCE` is set
    // from this table and the cities sit in whichever band it names.
    const BANDS: [i32; 9] = [2, 6, 10, 14, 18, 24, 30, 40, 55];

    for height in [12u16, 18] {
        println!();
        println!("  a surge of {height} (age {})", if height == 12 { "1-2" } else { "3" });
        println!("  distance from the bank   deepest   median deep   wade  swim");
        let mut rows: Vec<(i32, Vec<u16>, Vec<u16>)> =
            BANDS.iter().map(|&d| (d, Vec::new(), Vec::new())).collect();

        for seed in SEEDS {
            let mut w = World::new(seed, 2);
            w.disaster.height = height;
            // The water, and nothing else. `World::tick` is not used: a
            // citizen in a flood re-paths every tick against an effective
            // height the water is changing under it, which makes five days of
            // simulation about a hundred times more expensive for an answer
            // that does not depend on anybody being alive. `step_water` is the
            // stage `tick` would have called.
            //
            // Wound forward to the impact day by hand, because the surge only
            // pours on that day (design section 5) and `tick` is what would
            // otherwise have moved the clock.
            w.tick = (World::IMPACT_DAY - 1) * TICKS_PER_DAY;
            let mut deepest = vec![0u16; sim::map::CELLS];
            for _ in 0..SURGE_TICKS + 900 {
                w.step_water();
                w.tick += 1;
                for i in 0..sim::map::CELLS {
                    deepest[i] = deepest[i].max(w.water.depth[i]);
                }
            }

            let from_water = w.map.distance_to_water();
            for (band, worst, sampled) in rows.iter_mut() {
                let mut here: Vec<u16> = Vec::new();
                for y in 0..MAP_H {
                    for x in 0..MAP_W {
                        let d = from_water[Map::idx(x, y)];
                        if (d - *band).abs() <= 2 {
                            here.push(deepest[Map::idx(x, y)]);
                        }
                    }
                }
                here.sort_unstable();
                worst.push(here.last().copied().unwrap_or(0));
                // The median of the wet cells in the band: "if your city is
                // here, this is what a typical bit of it gets".
                let wet: Vec<u16> = here.into_iter().filter(|&d| d > 0).collect();
                sampled.push(wet.get(wet.len() / 2).copied().unwrap_or(0));
            }
        }

        for (band, worst, sampled) in &rows {
            let avg = |v: &Vec<u16>| v.iter().map(|&x| x as u32).sum::<u32>() / v.len() as u32;
            let (w_, m) = (avg(worst), avg(sampled));
            println!(
                "  {:>4} cells                 {:>7}   {:>11}   {:>4}  {:>4}",
                band,
                w_,
                m,
                if m >= WADE_DEPTH as u32 { "yes" } else { "no" },
                if m >= SWIM_DEPTH as u32 { "yes" } else { "no" },
            );
        }
    }
    println!();
    println!("  wading starts at {WADE_DEPTH}, swimming at {SWIM_DEPTH}, drowning after");
    println!("  {DROWN_TICKS} ticks out of your depth.");
    println!();
}

/// Is growing worth doing inside a three-age run?
///
/// M12.10. `COMING_OF_AGE` is two whole ages, so **only a child born in age
/// one ever works**, and age one is the age with no wood to spare for cottages
/// and a starvation clock on day four. City 0 reasoned its way to this during
/// the M11.9 run and asked whether growth is a fourth-age feature the MVP
/// should stop asking for.
///
/// **What this arranges**: nothing about food - M12.A made a farm feed a city,
/// and the point is to ask the question in the game that ships. It runs the
/// `grow` script, which builds cottages and a nursery, against `idle`, which
/// does not, and counts what the spending bought.
#[test]
#[ignore]
fn whether_growing_pays_inside_three_ages() {
    const SEEDS: [u64; 4] = [31, 1_000_003, 0xF100_D11E, 99];
    println!();
    println!("  seed          homed   households   settled   nur/beds  toward/made   born   ended   without");
    let mut any_worked = 0u32;
    for seed in SEEDS {
        let grown = play(Run::new(seed, Play::Grow));
        let bare = play(Run::new(seed, Play::Idle));
        // A child born at tick t works from t + COMING_OF_AGE. The run is
        // MAX_AGE * DAYS_PER_AGE * TICKS_PER_DAY long, so anybody born after
        // that minus COMING_OF_AGE never lifts anything.
        let run = MAX_AGE * DAYS_PER_AGE * TICKS_PER_DAY;
        let last_useful = run.saturating_sub(COMING_OF_AGE);
        println!(
            "  {:<12}  {:>5}   {:>10}   {:>7}   {:>3}/{:<4}  {:>5}/{:<5}   {:>4}   {:>5}   {:>7}",
            seed,
            grown.homed,
            grown.households,
            grown.settled,
            grown.nurseries,
            grown.spare_beds,
            grown.toward,
            grown.made,
            grown.born,
            grown.alive.last().copied().unwrap_or(0),
            bare.alive.last().copied().unwrap_or(0),
        );
        any_worked += grown.came_of_age;
        let _ = last_useful;
    }
    println!();
    println!(
        "  COMING_OF_AGE is {} days, and a run is {} days.",
        COMING_OF_AGE / TICKS_PER_DAY,
        MAX_AGE * DAYS_PER_AGE
    );
    println!(
        "  So a child has to be born in the first {} days to work at all,",
        (MAX_AGE * DAYS_PER_AGE).saturating_sub(COMING_OF_AGE / TICKS_PER_DAY)
    );
    println!("  and children come of age {any_worked} times across these seeds.");
}

/// Put two adults in every standing cottage that has room, which is what a
/// player who wants a household does. Design §3.2: two adults sharing a fed
/// cottage become a household, and no nursery means no children.
///
/// Nothing did this before M12.10, which is why the `grow` column of the
/// strategy table had never produced a child.
fn pair_them_up(w: &mut World) {
    let cottages: Vec<sim::building::BuildingId> = w
        .buildings
        .iter()
        .filter(|b| b.owner == ME && b.kind == Kind::Cottage && b.standing_now())
        .map(|b| b.id)
        .collect();
    for c in cottages {
        // Two, and only into a cottage that has nobody in it yet. Filling
        // every bed was tried and measured: eight people into two four-bed
        // cottages leaves no spare bed, and `have_children` needs one - so the
        // city that had housed everybody was the city that could not have a
        // child. A household is a *pair* with room to grow, not a full house.
        let already = w
            .citizens
            .iter()
            .filter(|p| p.owner == ME && p.alive() && p.home == Some(c))
            .count();
        if already > 0 {
            continue;
        }
        let homeless: Vec<CitizenId> = w
            .citizens
            .iter()
            .filter(|p| p.owner == ME && p.alive() && !p.is_child() && p.home.is_none())
            .map(|p| p.id)
            .take(2)
            .collect();
        if homeless.len() < 2 {
            return;
        }
        let room = w.will_house(ME, c, &homeless);
        if room >= 2 {
            let _ = w.apply(ME, &Command::SetHome { citizens: homeless, cottage: c });
        }
    }
}

/// **Does a finished wall change the outcome of a flood?**
///
/// Four playtests have asked whether a dike is worth building and not one has
/// answered, because in four runs no player has ever owned a finished wall:
/// M12.11's city 0 put four hundred and forty stone into fifteen segments
/// across two ages and not one reached `level 1 of 4`. The strategy table has
/// the same hole from the other side — the tallest wall its own `dike` script
/// ever got standing by the age-one flood was sixty stone of the seven hundred
/// and twenty a city starts with, and on two seeds in three the walling city
/// is dead before the water arrives.
///
/// So the question was never measured. It was two questions:
///
/// 1. **Is a wall worth having?** Give the city one, free, on the day it would
///    have ordered it, and take nobody off the fields for it.
/// 2. **Can a city of eight get one up?** That is the `dike` column of
///    `three_full_runs_of_each_strategy`, and it is a different answer.
///
/// This measures the first. Four cells to a seed: the two plays that survive
/// (`grow` and `flee`), each with and without a wall it did not have to earn.
/// If the wall changes nothing here it is not worth building at any price, and
/// three milestones of legibility work have been spent on a decision that does
/// not exist. If it changes a great deal, the fault is affordability, and
/// `DIKE_BUILD_TICKS` and the hauling are where to look.
///
///     cargo test -p sim --release --test playtest finished_wall -- --ignored --nocapture
#[test]
#[ignore = "a measurement, not an assertion: run it with --nocapture"]
fn whether_a_finished_wall_changes_a_flood() {
    const SEEDS: [u64; 3] = [31, 1_000_003, 0xF100_D11E];
    println!();
    println!(
        "  seed        play          ages  alive by age   at the hearth     in the quarter    \
         standing  wall  would cost"
    );
    let (mut walled_lived, mut bare_lived) = (0i32, 0i32);
    for seed in SEEDS {
        for how in [Play::Grow, Play::Flee] {
            for walled in [false, true] {
                let run = if walled { Run::new(seed, how).walled() } else { Run::new(seed, how) };
                let (_, lived) = play_and_name(run);
                if walled {
                    walled_lived += lived;
                } else {
                    bare_lived += lived;
                }
            }
        }
        println!();
    }
    println!(
        "  walled runs ended with {walled_lived} people alive, bare ones with \
         {bare_lived}, out of {} who ever lived.",
        FOUNDING_CITIZENS * 2 * SEEDS.len() as u32
    );
    println!();
}

/// One row of the table above, printed, returning the survivors it ended with.
fn play_and_name(run: Run) -> (Report, i32) {
    let r = play(run);
    println!(
        "  {:<11} {:<13} {:>4}   {:<13} {:<17} {:<17} {:>8}  {:>4}  {:>10}",
        r.seed,
        format!("{}{}", r.play.name(), if run.wall_by_fiat { " + wall" } else { "" }),
        r.ages,
        format!("{:?}", r.alive),
        format!("{:?}", r.at_the_fire),
        format!("{:?}", r.soaked),
        format!("{:?}", r.standing),
        r.wall,
        r.wall_cost,
    );
    let survivors = r.alive.last().copied().unwrap_or(0) as i32;
    (r, survivors)
}

/// What a walling city actually spends its days on, and what kills it.
///
/// The strategy table says the `dike` script dies in age one on two seeds in
/// three, with nought stone of wall standing on the impact day, while `grow`
/// and `flee` on the same seeds live two and three ages. It does not say what
/// the city died of, and every previous guess about a wall that never got
/// built has been wrong — the M11.9 handover blamed a footprint offset, the
/// M12.11 handover blamed `take_a_site`, and the answer was an evacuation that
/// holds everybody.
///
///     cargo test -p sim --release --test playtest walling_city -- --ignored --nocapture
#[test]
#[ignore = "a measurement, not an assertion: run it with --nocapture"]
fn what_a_walling_city_spends_its_days_on() {
    // The three the strategy table uses: 31 is the seed where a wall works,
    // and the other two are the ones where ordering one used to kill the city
    // on day four of age one.
    const SEEDS: [u64; 3] = [31, 1_000_003, 0xF100_D11E];
    for seed in SEEDS {
        for how in [Play::Dike, Play::Grow] {
            println!();
            println!("  seed {seed}, playing {}", how.name());
            let r = play(Run::new(seed, how).watched());
            println!(
                "  ended after {} ages with {} alive: {} drowned, {} starved, {} otherwise",
                r.ages,
                r.alive.last().copied().unwrap_or(0),
                r.drowned,
                r.starved,
                r.otherwise,
            );
        }
    }
    println!();
}
