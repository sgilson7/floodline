//! What everybody does with their day.
//!
//! One function per stage, run in a fixed order from `World::tick`, and every
//! loop over citizens or buildings goes in index order. That is the whole
//! determinism story for this module: two peers pick the same farm, the same
//! granary and the same load, because they walk the same lists the same way.
//!
//! The order of priorities is the design in one list: eat, sleep, then work.
//! A citizen too hungry to stand does not finish the delivery first — it goes,
//! and comes back to the load afterwards. It used to *destroy* the load, which
//! this note called "the real cost of having left it too late" until a
//! measurement put a number on it: seven hundred and ten stone of seven
//! hundred and twenty, in one day. See `Citizen::pause`.

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
                    self.citizens[i].pause();
                    self.citizens[i].errand = Some(Errand::ToEat(g));
                    self.citizens[i].walk_to(Dest::Building(g));
                    continue;
                }
                // **Nowhere to eat, and food standing in a field.**
                //
                // A farm fills a small buffer and stops, a citizen can only eat
                // at a granary, and an assigned farmer or builder never hauls —
                // so a city that employed everybody starved beside its own
                // working farm. Both M12.11 players hit it and so did the
                // `dike` script: three farmers, five builders on the wall,
                // nought in the granary, and five hundred units of food sitting
                // in two farms while eight people died.
                //
                // M12 answered it with a sentence in the panel, which is right
                // and is not enough. Hunger outranks the job — that is the list
                // at the top of this function and design §3.2's order — and it
                // cannot outrank it only when somebody else has already done the
                // carrying. A hungry citizen with food in sight goes and gets
                // it, whatever its job, and eats when it arrives.
                if !self.heading_for_food(i) {
                    let owner = self.citizens[i].owner;
                    let (x, y) = self.citizens[i].pos.cell();
                    if let Some((from, good, to)) =
                        self.next_collection(owner, x, y, Some(Good::Food))
                    {
                        self.citizens[i].pause();
                        self.citizens[i].errand = Some(Errand::Collect { from, good, to });
                        self.citizens[i].walk_to(Dest::Building(from));
                        continue;
                    }
                }
            }
            // **And not while it is already on its way to eat.**
            //
            // Without that second guard the two branches take it in turns. A
            // citizen that is hungry *and* tired sets `ToEat` on one tick; on
            // the next, hunger is skipped because `heading_to_eat` is true,
            // and this fires because its errand is `ToEat` rather than
            // `ToSleep` - so it abandons the meal and turns toward a bed. On
            // the tick after that hunger fires again for the same reason. It
            // flips every tick, walks a fraction of a cell each way, and
            // arrives nowhere. Reported from a played game as people
            // "gyrating in place"; measured at 235 changes of mind in 600
            // ticks, in `somebody_both_hungry_and_tired_goes_to_one_of_them`.
            //
            // Worse than the standing still: `abandon` puts down whatever is
            // being carried, so a hauler caught in this drops its load on the
            // floor once a tick.
            //
            // The comment above says hunger outranks tiredness when there is
            // somewhere to eat. This is the line that makes that true. When
            // there is nowhere - `nearest_food` gives `None` - no errand is
            // set, `heading_to_eat` stays false, and sleep still wins, which
            // is the case that comment exists for.
            if self.citizens[i].tired()
                && !self.heading_to_bed(i)
                && !self.heading_to_eat(i)
            {
                if let Some(bed) = self.bed_for(i) {
                    self.citizens[i].pause();
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
                // A cookery is the only building that eats a good to make one,
                // and it stops when the raw food runs out rather than making
                // meals from nothing. The work already put in is kept: cooks
                // waiting on a hauler have not wasted the morning.
                if let Some((from, n)) = b.kind.consumes() {
                    if b.store.get(from) < n {
                        break;
                    }
                    b.store.take(from, n);
                }
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

    /// Already on its way to fetch or deliver food.
    ///
    /// The guard on the hunger branch's second half. Without it a hungry
    /// citizen re-decides to fetch food every tick, and `pause` puts it back to
    /// the start of the walk each time — the gyrating fault of
    /// `somebody_both_hungry_and_tired_goes_to_one_of_them`, in a third place.
    fn heading_for_food(&self, i: usize) -> bool {
        match self.citizens[i].errand {
            Some(Errand::Collect { good: Good::Food, .. }) => true,
            Some(Errand::Carry { .. }) => self.citizens[i].carrying.food > 0,
            _ => false,
        }
    }

    fn heading_to_bed(&self, i: usize) -> bool {
        matches!(self.citizens[i].errand, Some(Errand::ToSleep(_)))
            || self.citizens[i].state == State::Sleeping
    }

    /// The nearest granary of this citizen's own city that has anything in it.
    ///
    /// Either thing: a granary holding only meals is a granary with food in
    /// it. Asking for `Good::Food` alone was how a city with a working cookery
    /// could starve in front of a full larder.
    fn nearest_food(&self, i: usize) -> Option<BuildingId> {
        let c = &self.citizens[i];
        let (x, y) = c.pos.cell();
        self.stores_for(c.owner, Good::Food, x, y)
            .into_iter()
            .find(|id| {
                let s = &self.buildings[id.0 as usize].store;
                s.food > 0 || s.meal > 0
            })
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
            if taken < self.buildings[id.0 as usize].beds() {
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
        // A child does not haul, farm or build. It stays at the nursery it was
        // born into, which is what a nursery is for, and everything else in
        // this file applies to it: it eats, it sleeps, and the flood does not
        // care how old anybody is.
        if self.citizens[i].is_child() {
            if let Some(n) = self.citizens[i].nursery {
                if self.buildings[n.0 as usize].standing_now() {
                    self.citizens[i].errand = Some(Errand::ToWork(n));
                    self.citizens[i].walk_to(Dest::Building(n));
                }
            }
            return;
        }
        // **What is in the arms goes somewhere it is wanted first, whatever
        // the job is.** A farmer or a builder never reaches `find_haul`, so a
        // citizen holding twenty stone when it was given a job would hold it
        // for the rest of the game — which is why this is here rather than
        // inside the `None` arm below, and why `unassign_one` can afford to
        // keep the load.
        if !self.citizens[i].carrying.is_empty() && self.deliver_what_you_hold(i) {
            return;
        }
        match self.citizens[i].job {
            // Everything that is not hauling: a producer stands at its
            // building, a builder walks to sites.
            // A trader stands at its post like a producer does; what leaves
            // the post is the mule, and a mule is not a citizen.
            Some(Job::Farmer)
            | Some(Job::Forester)
            | Some(Job::Quarrier)
            | Some(Job::Trader)
            | Some(Job::Cook)
            | Some(Job::Builder) => {
                if let Some(b) = self.citizens[i].workplace {
                    let there = &self.buildings[b.0 as usize];
                    if there.kind == Kind::BuildersHut && there.state != BuildState::Rubble {
                        // A builder's hut is a roster, not a bench. Walking to
                        // it and standing in it is exactly the thing it must
                        // not do: the point of the hut is that these people go
                        // where the work is. The assignment has already set
                        // the *job*, which is what persists, so let go of the
                        // hut and fall through to the work below.
                        self.citizens[i].workplace = None;
                    } else if there.state != BuildState::Rubble {
                        self.citizens[i].errand = Some(Errand::ToWork(b));
                        self.citizens[i].walk_to(Dest::Building(b));
                        return;
                    } else {
                        // Its workplace is gone; it is a hauler again.
                        self.citizens[i].workplace = None;
                        self.citizens[i].job = None;
                    }
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

    /// Take whatever is in this citizen's arms somewhere that wants it.
    ///
    /// The first branch of `find_haul`, on its own, so that a citizen with a
    /// job can use it: a site that still needs the load, or failing that a
    /// store with room for it.
    fn deliver_what_you_hold(&mut self, i: usize) -> bool {
        let owner = self.citizens[i].owner;
        let (x, y) = self.citizens[i].pos.cell();
        let load = self.citizens[i].carrying;
        match self.somewhere_for(owner, &load, x, y) {
            Some(to) => {
                self.citizens[i].errand = Some(Errand::Carry { to });
                self.citizens[i].walk_to(Dest::Building(to));
                true
            }
            None => false,
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
    /// **Construction first, then clearing producers, and that order is a
    /// hazard.** A city that hauls food while its granary is a hole in the
    /// ground has its priorities wrong, so construction has to come first — but
    /// a construction site is unlimited demand. Order the fifteen dike segments
    /// the drag tool draws and every free pair of hands in the city loops
    /// between the store and the wall for as long as any segment wants a stone,
    /// and the farms back up behind them.
    ///
    /// This comment used to claim a full farm was "construction's equal", which
    /// the code has never done. What keeps a walling city alive is not a
    /// priority here but the hunger branch of `assign_errands`: a citizen with
    /// nothing to eat fetches food itself, whatever it was doing and whatever
    /// its job. Nobody starves any more, and the wall goes up out of what is
    /// left of the day — which is the trade a wall is supposed to be.
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
        // A cookery with an empty larder is two people standing still, which
        // is the same fault as a farm that has backed up and is answered in
        // the same place.
        if let Some((from, good, to)) = self.next_feed_run(owner, x, y) {
            self.citizens[i].errand = Some(Errand::Collect { from, good, to });
            self.citizens[i].walk_to(Dest::Building(from));
            return true;
        }
        if let Some((from, good, to)) = self.next_collection(owner, x, y, None) {
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

    /// A building that eats a good to make one, with room for more of it, and
    /// somewhere to fetch it from.
    ///
    /// Only a cookery, and only one that has somebody in it: sending a hauler
    /// across the city to stock an unmanned kitchen is a walk, not work.
    fn next_feed_run(
        &self,
        owner: PlayerId,
        x: i32,
        y: i32,
    ) -> Option<(BuildingId, Good, BuildingId)> {
        for dst in &self.buildings {
            if dst.owner != owner || !dst.standing_now() || dst.workers.is_empty() {
                continue;
            }
            let Some((good, _)) = dst.kind.consumes() else {
                continue;
            };
            if !dst.kind.has_room_for(good, &dst.store) {
                continue;
            }
            let (dx, dy) = dst.centre();
            let from = self
                .stores_for(owner, good, dx, dy)
                .into_iter()
                .find(|id| self.buildings[id.0 as usize].store.get(good) > 0);
            if let Some(from) = from {
                let _ = (x, y);
                return Some((from, good, dst.id));
            }
        }
        None
    }

    /// Output waiting at a producer, and somewhere to put it.
    ///
    /// `only` narrows it to one good, which is how the starving city above
    /// asks for the harvest and nothing else: a city under a day of food does
    /// not want its haulers fetching wood.
    fn next_collection(
        &self,
        owner: PlayerId,
        x: i32,
        y: i32,
        only: Option<Good>,
    ) -> Option<(BuildingId, Good, BuildingId)> {
        for src in &self.buildings {
            if src.owner != owner || !src.standing_now() {
                continue;
            }
            let Some(good) = src.kind.produces() else {
                continue;
            };
            if only.is_some_and(|want| want != good) {
                continue;
            }
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
        // Meals first. A meal is worth two of what a raw unit is worth, and a
        // granary holding both should spend the better one — a city that ate
        // its raw food while the meals went stale would have built the cookery
        // for nothing. It is also the only ordering a player can predict.
        let on_offer = if b.store.meal > 0 { Good::Meal } else { Good::Food };
        if !b.standing_now() || b.store.get(on_offer) == 0 || self.citizens[i].food >= FED_ENOUGH
        {
            self.citizens[i].errand = None;
            if self.citizens[i].state == State::Eating {
                self.citizens[i].state = State::Idle;
            }
            return;
        }
        let taken = b.store.take(on_offer, EAT_RATE);
        self.citizens[i].eat(taken * FOOD_PER_UNIT * on_offer.feeds());
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
                BuildState::Standing if dest.kind.takes(g) => {
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
                // **A site that has not been supplied is not work, and standing
                // at one is why nobody has ever finished a wall.**
                //
                // `Building::build` returns false while anything is
                // outstanding, and this arm did not look: the citizen was set
                // to `Working`, its errand stayed `ToWork`, `busy()` stayed
                // true, and `find_work` never ran again. An assigned builder at
                // an unsupplied site therefore worked at nothing for the rest
                // of the game — and the hands it took were the city's haulers,
                // so nothing ever brought the stone that would have released
                // it. Measured in `what_a_walling_city_spends_its_days_on`:
                // five of eight people stood at seven dike sites for four days
                // at `0% done` with ninety stone owed and six hundred in the
                // store, and the city starved around them.
                //
                // It lets go of the site rather than the job. `find_work` sends
                // a Builder to the next site that *is* supplied, and failing
                // that to `find_haul`, which fetches exactly what this site was
                // waiting for — so a builder now supplies its own wall, which
                // is what a player watching one would expect it to do.
                if !self.buildings[at.0 as usize].ready_to_build() {
                    self.citizens[i].errand = None;
                    self.citizens[i].workplace = None;
                    self.citizens[i].state = State::Idle;
                    return;
                }
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
            (Some(job), BuildState::Standing) if job.stationed() => {
                let b = &mut self.buildings[at.0 as usize];
                if !b.workers.contains(&CitizenId(i as u16)) {
                    if b.workers.len() < b.slots_for(job) {
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

    /// Everything a city's people are carrying, right now.
    ///
    /// `treasury` counts only what is at rest in a standing store, which is the
    /// right answer for "what can I spend" and a bewildering one to watch: both
    /// players in the M10.6 run saw wood drop from 200 to 40 and back to 150
    /// after siting one 50-wood granary — eight people each picking up a load —
    /// and one of them was sure it had been overcharged.
    ///
    /// Mules are not counted. What a mule carries is on its way to another city
    /// and is not this one's to spend.
    pub fn in_hand(&self, owner: PlayerId) -> Goods {
        let mut total = Goods::NONE;
        for c in self.citizens.iter().filter(|c| c.owner == owner && c.alive()) {
            for g in Good::ALL {
                total.add(g, c.carrying.get(g));
            }
        }
        total
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
