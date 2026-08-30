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
use sim::building::{Good, Kind};
use sim::citizen::{CitizenId, PlayerId};
use sim::command::Command;
use sim::map::{Corner, Map, MAP_H, MAP_W};
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
    /// How many dike cells were actually built.
    wall: usize,
    /// Water at the hearth itself, deepest in each age.
    at_the_fire: Vec<u16>,
    /// Stone in hand on the day the water came.
    stone_on_the_day: Vec<u16>,
    ages: u32,
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
                if w.can_place(ME, kind, hx + dx, hy + dy).is_ok() {
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

/// A step from the hearth towards the corner the water comes out of.
fn towards(corner: Corner, hx: i32, hy: i32) -> (i32, i32) {
    let (cx, cy) = corner.cell();
    ((cx - hx).signum(), (cy - hy).signum())
}

fn play(seed: u64, play: Play) -> Report {
    let mut w = World::new(seed, 2);
    let mut nav = Nav::new();
    let (hx, hy) = w.map.hearth_sites[0];
    let (dx, dy) = towards(w.map.low_corner, hx, hy);

    let mut report = Report {
        seed,
        play,
        alive: Vec::new(),
        soaked: Vec::new(),
        from_the_corner: (hx - w.map.low_corner.cell().0).abs()
            + (hy - w.map.low_corner.cell().1).abs(),
        standing: Vec::new(),
        wall: 0,
        at_the_fire: Vec::new(),
        stone_on_the_day: Vec::new(),
        ages: 0,
    };

    // What the strategy intends to put down, in order, one per day. Spread out
    // rather than all at once because materials are hauled and built by the
    // same eight people, and a city that orders six buildings on day one has
    // six half-built sites when the water arrives.
    // Food first, and a granary the same day: a citizen can only eat at a
    // granary, and the founding party's own food empties inside one day.
    let plan: Vec<Kind> = if play.builds() {
        vec![Kind::Farm, Kind::Granary, Kind::Cottage, Kind::Cottage, Kind::Farm]
    } else {
        Vec::new()
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
                    if w.apply(ME, &Command::Place { kind, x: x as u8, y: y as u8 }).is_ok() {
                        placed += 1;
                    }
                }
            }
            assign_to_farms(&mut w);
        }

        // The dike goes up once, early, and by hand rather than by hauling —
        // this file is measuring whether a wall in the right place changes the
        // outcome, and "could eight people have built one in six days" is the
        // separate question `build_ticks` answers.
        if (play == Play::Dike || play == Play::Both) && !diked && day >= 1 {
            diked = true;
            build_a_wall(&mut w, hx, hy, dx, dy);
            report.wall = w
                .buildings
                .iter()
                .filter(|b| b.owner == ME && b.kind == Kind::Dike)
                .count();
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
        w.tick(&mut nav, &[]);
        deepest_this_age = deepest_this_age.max(deepest_near(&w, hx, hy));
        at_the_fire = at_the_fire.max(w.water.depth_at(hx, hy));
        if w.day_of_age() == World::IMPACT_DAY && report.stone_on_the_day.len() < w.age() as usize {
            report.stone_on_the_day.push(w.treasury(ME).stone);
        }
        if w.age() != age_before || w.finished().is_some() {
            report.alive.push(w.population(ME));
            report.soaked.push(deepest_this_age);
            report.at_the_fire.push(at_the_fire);
            at_the_fire = 0;
            report.standing.push(
                w.buildings.iter().filter(|b| b.owner == ME && b.standing_now()).count(),
            );
            deepest_this_age = 0;
            fled = false;
        }
    }
    report.ages = w.score().ages_survived;
    report
}

/// Fill every farm's slots from whoever has no job, which is what a player
/// does after each farm goes up. Nothing else needs assigning: a city finishes
/// its own building sites, and hauling is what an unassigned citizen does.
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

/// A wall along the shore, between the city and the water, at full height.
///
/// Parallel to the line the water arrives on rather than an L around the
/// hearth: the flood comes off one corner and spreads along the low ground, so
/// what stops it is a bank across its path, and what a player would build is a
/// bank across its path. An L of fourteen cells was the first attempt and the
/// water simply went round it, which is a fact about that wall and not about
/// dikes.
fn build_a_wall(w: &mut World, hx: i32, hy: i32, _dx: i32, _dy: i32) {
    let (cx, cy) = w.map.low_corner.cell();
    let (sx, sy) = ((MAP_W / 2 - cx).signum(), (MAP_H / 2 - cy).signum());
    let out = (hx - cx).abs() + (hy - cy).abs() - 10;
    let mid = (hx - cx).abs();
    let mut wall = |x: i32, y: i32| {
        if w.can_place(ME, Kind::Dike, x, y).is_err() {
            return;
        }
        let Ok(id) = w.place(ME, Kind::Dike, x, y) else { return };
        for _ in 0..DIKE_MAX_LEVEL {
            let want = w.buildings[id.0 as usize].outstanding().stone;
            w.deliver_to(id, Good::Stone, want);
            w.build_at(id, Kind::Dike.build_ticks());
            if w.buildings[id.0 as usize].level < DIKE_MAX_LEVEL {
                let _ = w.raise_dike(ME, id);
            }
        }
    };
    // Every cell ten Manhattan cells nearer the water than the hearth is, for
    // twenty-five cells either side of it along the shore.
    for k in -25..=25 {
        let along = mid + k;
        if along < 0 || along > out {
            continue;
        }
        wall(cx + sx * along, cy + sy * (out - along));
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
        "  seed        play   ages  alive by age   at the hearth     in the quarter     wall  out"
    );
    let mut reports = Vec::new();
    for seed in SEEDS {
        for p in Play::ALL {
            let r = play(seed, p);
            println!(
                "  {:<11} {:<6} {:>4}   {:<13} {:<17} {:<17} {:>5} {:>4}",
                r.seed,
                r.play.name(),
                r.ages,
                format!("{:?}", r.alive),
                format!("{:?}", r.at_the_fire),
                format!("{:?}", r.soaked),
                r.wall,
                r.from_the_corner,
            );
            reports.push(r);
        }
        println!();
    }

    let total = |p: Play| -> u32 {
        reports.iter().filter(|r| r.play == p).map(|r| r.alive.last().copied().unwrap_or(0)).sum()
    };
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
#[test]
#[ignore = "a measurement, not an assertion: run it with --nocapture"]
fn how_far_the_water_reaches() {
    const SEEDS: [u64; 2] = [31, 1_000_003];
    const BANDS: [i32; 9] = [40, 55, 70, 85, 100, 115, 130, 145, 160];

    for height in [12u16, 18] {
        println!();
        println!("  a surge of {height} (age {})", if height == 12 { "1-2" } else { "3" });
        println!("  distance from the corner   deepest   median deep   wade  swim");
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

            let (cx, cy) = w.map.low_corner.cell();
            for (band, worst, sampled) in rows.iter_mut() {
                let mut here: Vec<u16> = Vec::new();
                for y in 0..MAP_H {
                    for x in 0..MAP_W {
                        let d = (x - cx).abs() + (y - cy).abs();
                        if (d - *band).abs() <= 7 {
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
