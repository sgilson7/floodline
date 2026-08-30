//! What everybody does with their day.
//!
//! One function per stage, run in a fixed order from `World::tick`, and every
//! loop over citizens or buildings goes in index order. That is the whole
//! determinism story for this module: two peers pick the same farm, the same
//! granary and the same load, because they walk the same lists the same way.
//!
//! The order of priorities is the design in one list: eat, sleep, then work.
//! A citizen too hungry to stand does not finish the delivery first — it drops
//! what it is carrying and goes, and losing that load is the real cost of
//! having left it too late.

use crate::balance::*;
use crate::building::{BuildState, BuildingId, Good, Goods, Kind};
use crate::citizen::{CitizenId, Errand, Job, PlayerId, State};
use crate::nav::Dest;
use crate::world::World;

impl World {
    /// Decide what each idle citizen should be doing, and interrupt those
    /// whose bodies have overruled them.
    pub(crate) fn assign_errands(&mut self) {
        for i in 0..self.citizens.len() {
            if !self.citizens[i].alive() {
                continue;
            }

            // Hunger and exhaustion outrank everything, including a half-done
            // delivery. Checked before `busy`, so they can interrupt.
            //
            // Hunger outranks tiredness, but only when there is somewhere to
            // eat. Ordering them the other way round — hunger first, and
            // exhaustion considered only if not hungry — meant that a city
            // which ran out of food had citizens who were permanently hungry
            // and therefore never slept again, so nobody ever claimed a bed
            // and everybody worked at half speed until they starved. A hunger
            // that cannot be answered must not veto the sleep that can.
            if self.citizens[i].hungry() && !self.heading_to_eat(i) {
                if let Some(g) = self.nearest_food(i) {
                    self.citizens[i].abandon();
                    self.citizens[i].errand = Some(Errand::ToEat(g));
                    self.citizens[i].walk_to(Dest::Building(g));
                    continue;
                }
            }
            if self.citizens[i].tired() && !self.heading_to_bed(i) {
                if let Some(bed) = self.bed_for(i) {
                    self.citizens[i].abandon();
                    self.citizens[i].errand = Some(Errand::ToSleep(bed));
                    self.citizens[i].walk_to(Dest::Building(bed));
                    continue;
                }
            }

            if self.citizens[i].busy() {
                continue;
            }
            // Told to stand somewhere and not yet told otherwise. See
            // `Citizen::held`.
            if self.citizens[i].held {
                continue;
            }
            self.find_work(i);
        }
    }

    /// Everyone who has arrived somewhere does the thing they went there for.
    pub(crate) fn resolve_arrivals(&mut self) {
        for i in 0..self.citizens.len() {
            if !self.citizens[i].alive() || self.citizens[i].state == State::Walking {
                continue;
            }
            let Some(errand) = self.citizens[i].errand else {
                continue;
            };
            match errand {
                Errand::ToEat(g) => self.eat_at(i, g),
                Errand::ToSleep(b) => self.sleep_at(i, b),
                Errand::Collect { from, good, to } => self.collect_at(i, from, good, to),
                Errand::Carry { to } => self.deliver_at(i, to),
                Errand::ToWork(b) => self.work_at(i, b),
            }
        }
    }

    /// Producers turn worker-ticks into goods: a farm into food, a forester's
    /// hut into wood, a quarry into stone.
    pub(crate) fn produce(&mut self) {
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            let Some(good) = building.kind.produces() else {
                continue;
            };
            if !building.standing_now() {
                continue;
            }
            let hands = building.workers.len() as u32;
            if hands == 0 || !building.kind.has_room_for(good, &building.store) {
                continue;
            }
            let b = &mut self.buildings[b];
            let per = b.kind.ticks_per_unit();
            b.work += hands;
            while b.work >= per && b.kind.has_room_for(good, &b.store) {
                b.work -= per;
                b.store.add(good, 1);
            }
        }
    }

    // ---- deciding ----------------------------------------------------------

    fn heading_to_eat(&self, i: usize) -> bool {
        matches!(self.citizens[i].errand, Some(Errand::ToEat(_)))
            || self.citizens[i].state == State::Eating
    }

    fn heading_to_bed(&self, i: usize) -> bool {
        matches!(self.citizens[i].errand, Some(Errand::ToSleep(_)))
            || self.citizens[i].state == State::Sleeping
    }

    /// The nearest granary of this citizen's own city that has anything in it.
    fn nearest_food(&self, i: usize) -> Option<BuildingId> {
        let c = &self.citizens[i];
        let (x, y) = c.pos.cell();
        self.stores_for(c.owner, Good::Food, x, y)
            .into_iter()
            .find(|id| self.buildings[id.0 as usize].store.food > 0)
    }

    /// This citizen's cottage, claiming one if it has none.
    fn bed_for(&mut self, i: usize) -> Option<BuildingId> {
        if let Some(home) = self.citizens[i].home {
            if self.buildings[home.0 as usize].standing_now() {
                return Some(home);
            }
            // The cottage is gone; look for another.
            self.citizens[i].home = None;
        }

        let owner = self.citizens[i].owner;
        let (x, y) = self.citizens[i].pos.cell();
        let mut candidates: Vec<(i32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|b| b.owner == owner && b.standing_now() && b.kind == Kind::Cottage)
            .map(|b| {
                let (bx, by) = b.centre();
                ((bx - x).abs() + (by - y).abs(), b.id)
            })
            .collect();
        candidates.sort_unstable();

        for (_, id) in candidates {
            let taken = self.citizens.iter().filter(|c| c.home == Some(id)).count();
            if taken < Kind::Cottage.beds() {
                self.citizens[i].home = Some(id);
                return Some(id);
            }
        }
        None
    }

    /// Give citizen `i` something to do.
    ///
    /// The order is: your own job if you have one, then carrying, then
    /// building. Carrying before building because a site with nothing
    /// delivered cannot be built and a builder standing at one is a citizen
    /// doing nothing.
    fn find_work(&mut self, i: usize) {
        match self.citizens[i].job {
            // Everything that is not hauling: a producer stands at its
            // building, a builder walks to sites.
            Some(Job::Farmer)
            | Some(Job::Forester)
            | Some(Job::Quarrier)
            | Some(Job::Builder) => {
                if let Some(b) = self.citizens[i].workplace {
                    if self.buildings[b.0 as usize].state != BuildState::Rubble {
                        self.citizens[i].errand = Some(Errand::ToWork(b));
                        self.citizens[i].walk_to(Dest::Building(b));
                        return;
                    }
                    // Its workplace is gone; it is a hauler again.
                    self.citizens[i].workplace = None;
                    self.citizens[i].job = None;
                }
                // Design §3.2 says a Builder "walks to construction sites",
                // plural. One whose site is finished looks for the next before
                // it falls back to carrying, or every building in the city
                // would cost the player another round of clicks.
                if self.citizens[i].job == Some(Job::Builder) && self.take_a_site(i) {
                    return;
                }
                if !self.find_haul(i) {
                    self.take_a_site(i);
                }
            }
            // Design §3.2: hauling is what an unassigned citizen does — and
            // when there is nothing to carry, so is building.
            //
            // The second half of that is not in the design and is here because
            // of what a measurement showed. A city that places a farm sees its
            // haulers deliver the wood and stone and then stop: the site sits
            // full and finished-looking, nothing ever builds it, the granary
            // never exists, and since a citizen can only eat at a granary the
            // whole city starves on day four with the materials on the ground.
            // The fix by the book is "assign builders to the site", the gesture
            // exists, and nothing whatsoever tells a player that it is needed.
            // A trap that silent, that early and that fatal is not difficulty.
            //
            // Assignment still matters and still does the same thing: an
            // assigned builder goes to *its* site and stays until it is done,
            // and `BUILDER_SLOTS` caps how many can work on one at once. What
            // this buys is that an unattended city builds slowly instead of
            // dying, which is the difference between a game a stranger can
            // learn and one they have to be told.
            None | Some(Job::Hauler) => {
                if !self.find_haul(i) {
                    self.take_a_site(i);
                }
            }
        }
    }

    /// The nearest of this owner's sites with a builder's slot free and
    /// something to do, taken for one building's worth of work.
    ///
    /// Not an `Assign`: the citizen's `job` is untouched, so a hauler that
    /// lends a hand goes back to hauling the moment the site is finished, and
    /// nothing about the player's roster changes behind their back.
    fn take_a_site(&mut self, i: usize) -> bool {
        let owner = self.citizens[i].owner;
        let (x, y) = self.citizens[i].pos.cell();
        let mut sites: Vec<(i32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|b| b.owner == owner && b.state == BuildState::Site)
            // Materials first: a site still waiting for wood cannot be built,
            // and standing at one is worse than carrying the wood to it.
            .filter(|b| b.outstanding().is_empty())
            .filter(|b| {
                self.citizens
                    .iter()
                    .filter(|c| c.alive() && c.workplace == Some(b.id))
                    .count()
                    < BUILDER_SLOTS
            })
            .map(|b| {
                let (bx, by) = b.centre();
                ((bx - x).abs() + (by - y).abs(), b.id)
            })
            .collect();
        sites.sort_unstable();
        let Some(&(_, id)) = sites.first() else {
            return false;
        };
        self.citizens[i].workplace = Some(id);
        self.citizens[i].errand = Some(Errand::ToWork(id));
        self.citizens[i].walk_to(Dest::Building(id));
        true
    }

    /// The next load worth moving, if there is one.
    ///
    /// Construction first, then clearing producers. A city that hauls food
    /// while its granary is a hole in the ground has its priorities wrong, and
    /// so would a city whose farms back up because every hauler is on a
    /// building site — which is why a full farm counts as construction's equal
    /// rather than being checked only when nothing is being built.
    fn find_haul(&mut self, i: usize) -> bool {
        let owner = self.citizens[i].owner;
        let (x, y) = self.citizens[i].pos.cell();

        // Anything already in your arms comes first.
        //
        // A hauler that carries twenty to a site wanting ten delivers ten and
        // keeps the rest, and until this was here it then forgot it had them:
        // it looked for new work, found every store empty *because the wood
        // was in its own arms*, and stood still for the rest of the game. Three
        // of them ended up holding sixty wood in front of a granary that needed
        // ten, and the city starved beside a farm it had built.
        if !self.citizens[i].carrying.is_empty() {
            if let Some(to) = self.somewhere_for(owner, &self.citizens[i].carrying.clone(), x, y) {
                self.citizens[i].errand = Some(Errand::Carry { to });
                self.citizens[i].walk_to(Dest::Building(to));
                return true;
            }
        }

        if let Some((from, good, to)) = self.next_supply_run(owner, x, y) {
            self.citizens[i].errand = Some(Errand::Collect { from, good, to });
            self.citizens[i].walk_to(Dest::Building(from));
            return true;
        }
        if let Some((from, good, to)) = self.next_collection(owner, x, y) {
            self.citizens[i].errand = Some(Errand::Collect { from, good, to });
            self.citizens[i].walk_to(Dest::Building(from));
            return true;
        }
        false
    }

    /// Somewhere that will take what this citizen is holding: a site that
    /// still wants some of it, or failing that a store with room.
    fn somewhere_for(&self, owner: PlayerId, load: &Goods, x: i32, y: i32) -> Option<BuildingId> {
        // A site that needs it, nearest first — the load is more useful in a
        // wall than in a pile.
        let mut wanted: Vec<(i32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|b| b.owner == owner && b.state == BuildState::Site)
            .filter(|b| {
                let want = b.outstanding();
                Good::ALL.iter().any(|&g| want.get(g) > 0 && load.get(g) > 0)
            })
            .map(|b| {
                let (bx, by) = b.centre();
                ((bx - x).abs() + (by - y).abs(), b.id)
            })
            .collect();
        wanted.sort_unstable();
        if let Some(&(_, id)) = wanted.first() {
            return Some(id);
        }

        // Otherwise put it down somewhere it will keep.
        for g in Good::ALL {
            if load.get(g) == 0 {
                continue;
            }
            let store = self.stores_for(owner, g, x, y).into_iter().find(|id| {
                let b = &self.buildings[id.0 as usize];
                b.kind.has_room_for(g, &b.store)
            });
            if store.is_some() {
                return store;
            }
        }
        None
    }

    /// Materials a construction site is waiting for, and a store that has
    /// them. Sites in id order, so the oldest is finished first rather than
    /// every site being fed a little.
    fn next_supply_run(
        &self,
        owner: PlayerId,
        x: i32,
        y: i32,
    ) -> Option<(BuildingId, Good, BuildingId)> {
        for site in &self.buildings {
            if site.owner != owner || site.state != BuildState::Site {
                continue;
            }
            let want = site.outstanding();
            if want.is_empty() {
                continue;
            }
            for g in Good::ALL {
                if want.get(g) == 0 {
                    continue;
                }
                let source = self
                    .stores_for(owner, g, x, y)
                    .into_iter()
                    .find(|id| self.buildings[id.0 as usize].store.get(g) > 0);
                if let Some(from) = source {
                    return Some((from, g, site.id));
                }
            }
        }
        None
    }

    /// Output waiting at a producer, and somewhere to put it.
    fn next_collection(
        &self,
        owner: PlayerId,
        x: i32,
        y: i32,
    ) -> Option<(BuildingId, Good, BuildingId)> {
        for src in &self.buildings {
            if src.owner != owner || !src.standing_now() {
                continue;
            }
            let Some(good) = src.kind.produces() else {
                continue;
            };
            if src.store.get(good) == 0 {
                continue;
            }
            let (sx, sy) = src.centre();
            let to = self
                .stores_for(owner, good, sx, sy)
                .into_iter()
                .find(|id| {
                    let b = &self.buildings[id.0 as usize];
                    b.kind.has_room_for(good, &b.store)
                })?;
            let _ = (x, y);
            return Some((src.id, good, to));
        }
        None
    }

    // ---- arriving ----------------------------------------------------------

    fn eat_at(&mut self, i: usize, granary: BuildingId) {
        let b = &mut self.buildings[granary.0 as usize];
        if !b.standing_now() || b.store.food == 0 || self.citizens[i].food >= FED_ENOUGH {
            self.citizens[i].errand = None;
            if self.citizens[i].state == State::Eating {
                self.citizens[i].state = State::Idle;
            }
            return;
        }
        let taken = b.store.take(Good::Food, EAT_RATE);
        self.citizens[i].eat(taken * FOOD_PER_UNIT);
        self.citizens[i].state = State::Eating;
    }

    fn sleep_at(&mut self, i: usize, cottage: BuildingId) {
        if !self.buildings[cottage.0 as usize].standing_now()
            || self.citizens[i].rest >= RESTED_ENOUGH
        {
            self.citizens[i].errand = None;
            if self.citizens[i].state == State::Sleeping {
                self.citizens[i].state = State::Idle;
            }
            return;
        }
        self.citizens[i].sleep(SLEEP_RATE);
        self.citizens[i].state = State::Sleeping;
    }

    fn collect_at(&mut self, i: usize, from: BuildingId, good: Good, to: BuildingId) {
        let src = &mut self.buildings[from.0 as usize];
        let got = src.store.take(good, CARRY_CAPACITY);
        if got == 0 {
            // Somebody beat this citizen to it.
            self.citizens[i].errand = None;
            return;
        }
        self.citizens[i].carrying.add(good, got);
        self.citizens[i].errand = Some(Errand::Carry { to });
        self.citizens[i].walk_to(Dest::Building(to));
    }

    fn deliver_at(&mut self, i: usize, to: BuildingId) {
        let carried = self.citizens[i].carrying;
        for g in Good::ALL {
            let amount = carried.get(g);
            if amount == 0 {
                continue;
            }
            let dest = &mut self.buildings[to.0 as usize];
            let accepted = match dest.state {
                // A site takes exactly what it still needs.
                BuildState::Site => dest.deliver(g, amount),
                BuildState::Standing if dest.kind.stores(g) => {
                    let room = dest.kind.capacity().get(g).saturating_sub(dest.store.get(g));
                    let put = room.min(amount);
                    dest.store.add(g, put);
                    put
                }
                _ => 0,
            };
            self.citizens[i].carrying.take(g, accepted);
        }
        // Anything that would not fit is still in this citizen's arms, and the
        // next errand it takes will carry it along. It is not thrown away.
        self.citizens[i].errand = None;
        self.citizens[i].state = State::Idle;
    }

    fn work_at(&mut self, i: usize, at: BuildingId) {
        let b = &self.buildings[at.0 as usize];
        let job = self.citizens[i].job;

        match (job, b.state) {
            // Any job at all, at a site. A Builder was assigned here; a hauler
            // walked over because there was nothing to carry and `take_a_site`
            // sent it. The work is the same work, and `build_at` is the only
            // thing that turns delivered materials into a building.
            (_, BuildState::Site) => {
                // Half effort when exhausted, like walking.
                let effort =
                    if self.citizens[i].tired() { BUILDER_EFFORT.max(2) / 2 } else { BUILDER_EFFORT };
                self.citizens[i].state = State::Working;
                if self.build_at(at, effort) {
                    // Finished. Look for the next thing next tick.
                    self.citizens[i].errand = None;
                    self.citizens[i].workplace = None;
                    self.citizens[i].state = State::Idle;
                }
            }
            (Some(job), BuildState::Standing) if job.produces() => {
                let b = &mut self.buildings[at.0 as usize];
                if !b.workers.contains(&CitizenId(i as u16)) {
                    if b.workers.len() < b.kind.slots_for(job) {
                        b.workers.push(CitizenId(i as u16));
                    } else {
                        // Somebody took the last slot while this one walked.
                        self.citizens[i].errand = None;
                        self.citizens[i].workplace = None;
                        self.citizens[i].state = State::Idle;
                        return;
                    }
                }
                self.citizens[i].state = State::Working;
            }
            _ => {
                self.citizens[i].errand = None;
                self.citizens[i].workplace = None;
                self.citizens[i].state = State::Idle;
            }
        }
    }

    /// Take a citizen off whatever building's roster it is on. Called when a
    /// job changes or a citizen dies, so a farm does not go on producing from
    /// the labour of the departed.
    pub(crate) fn clear_from_rosters(&mut self, who: CitizenId) {
        for b in &mut self.buildings {
            b.workers.retain(|&c| c != who);
        }
    }

    /// Everything a city holds, across all its standing stores. For the panel
    /// and for the tests.
    pub fn treasury(&self, owner: PlayerId) -> Goods {
        let mut total = Goods::NONE;
        for b in &self.buildings {
            if b.owner == owner && b.standing_now() && b.kind.is_store() {
                for g in Good::ALL {
                    total.add(g, b.store.get(g));
                }
            }
        }
        total
    }
}
