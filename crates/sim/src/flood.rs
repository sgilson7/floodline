//! What moving water does to people and buildings.
//!
//! Design §3.4's second and third paragraphs: bodies are kinematic points that
//! pick up the water's movement, and buildings take damage from the flow
//! across their footprint minus what their material shrugs off. Both are
//! deliberately small — the automaton in `water` is where the interesting
//! behaviour comes from, and this is only what reads it.
//!
//! The two things that save a citizen are the two design §5 names: high
//! ground, and a roof. Everything else is arithmetic.

use crate::balance::*;
use crate::building::{BuildState, BuildingId, Kind};
use crate::citizen::{CitizenId, State};
use crate::fx::{Fx, V2};
use crate::map::Map;
use crate::nav;
use crate::world::World;

impl World {
    /// One tick of the flood acting on everything in it.
    ///
    /// Public, with `step_water`, so a test can drive the water without a
    /// whole tick of hunger and errands around it. `tick` calls both in order
    /// and nothing else should need to.
    pub fn flood_bodies(&mut self) {
        if self.water.volume() == 0 {
            return;
        }
        self.sweep_citizens();
        self.batter_buildings();
    }

    /// How deep the water is where a citizen is standing, after whatever it is
    /// standing on.
    ///
    /// A standing building is a roof. A construction site is not — it is a
    /// hole with some planks in it — and rubble is certainly not.
    pub fn depth_over(&self, x: i32, y: i32) -> u16 {
        let d = self.water.depth_at(x, y);
        match self.building_at(x, y) {
            Some(b) if b.standing_now() => d.saturating_sub(depth(b.kind.shelter())),
            _ => d,
        }
    }

    fn sweep_citizens(&mut self) {
        for i in 0..self.citizens.len() {
            if !self.citizens[i].alive() {
                continue;
            }
            let (cx, cy) = self.citizens[i].pos.cell();
            let over = self.depth_over(cx, cy);

            if over < WADE_DEPTH {
                // Dry, or near enough to keep your feet.
                self.citizens[i].swept = false;
                self.citizens[i].drowning_for = 0;
                continue;
            }

            let (fx, fy) = self.water.flow_at(cx, cy);
            let push = V2::new(
                Fx(fx * WATER_DRAG / DEPTH_SCALE as i32),
                Fx(fy * WATER_DRAG / DEPTH_SCALE as i32),
            );

            if over >= SWIM_DEPTH {
                // Out of your depth: you go where the water goes. Orders are
                // no longer something you can follow, which is why "get
                // uphill" has to be given *before* the water arrives — that is
                // the whole tension design §5 step 3 is describing.
                self.citizens[i].swept = true;
                self.citizens[i].dest = None;
                self.citizens[i].errand = None;
                if self.citizens[i].state != State::Dead {
                    self.citizens[i].state = State::Idle;
                }
                self.citizens[i].drowning_for += 1;
                if self.citizens[i].drowning_for >= DROWN_TICKS {
                    self.citizens[i].die();
                    continue;
                }
            } else {
                // Wading: you keep your feet and your orders, and the water
                // still shoves you about.
                self.citizens[i].swept = false;
                self.citizens[i].drowning_for = 0;
            }

            self.citizens[i].vel += push;
            let want = self.citizens[i].pos + push;
            self.citizens[i].pos = self.carried_to(self.citizens[i].pos, want);
        }
    }

    /// Move a body from `from` toward `to`, refusing to put it anywhere it
    /// could not be.
    ///
    /// Design §3.4 resolves a collision "by pushing the point out along the
    /// shallowest axis". This does the same job the cheap way round: try the
    /// whole move, and if that lands somewhere solid, try each axis on its own.
    /// A body sliding along a wall is the visible result either way, and there
    /// is no penetration to push out of because none is ever allowed.
    fn carried_to(&self, from: V2, to: V2) -> V2 {
        let solid = |p: V2| {
            let (x, y) = p.cell();
            if !Map::contains(x, y) {
                return true;
            }
            // Water carries you into places you would not walk — over a road,
            // against a wall — so the test is what is solid, not what is
            // walkable. Rock and a standing building are solid; deep water is
            // exactly where you are.
            match self.building_at(x, y) {
                Some(b) if b.blocks_movement() => true,
                _ => self.map.ground_at(x, y) == crate::map::Ground::Rock,
            }
        };

        if !solid(to) {
            return to;
        }
        let slide_x = V2::new(to.x, from.y);
        if !solid(slide_x) {
            return slide_x;
        }
        let slide_y = V2::new(from.x, to.y);
        if !solid(slide_y) {
            return slide_y;
        }
        from
    }

    /// Buildings take damage from the water going past them.
    fn batter_buildings(&mut self) {
        let mut ruined: Vec<BuildingId> = Vec::new();

        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            if building.state == BuildState::Rubble {
                continue;
            }
            let resist = building.kind.resist();

            // The worst any one cell of the footprint is taking, not the sum:
            // a farm is not nine times as fragile as a dike for being nine
            // times as wide.
            let worst = building
                .cells()
                .map(|(x, y)| self.water.speed_at(x, y))
                .max()
                .unwrap_or(0);

            if worst <= resist {
                continue;
            }
            let hurt = ((worst - resist) as u32 * FLOW_DAMAGE as u32 / DEPTH_SCALE as u32)
                .max(1)
                .min(u16::MAX as u32) as u16;
            if self.damage_building(BuildingId(b as u16), hurt) {
                ruined.push(BuildingId(b as u16));
            }
        }

        // Anybody who worked there is out of a job, and a road that has gone
        // is a road nobody should still be walking to.
        for id in ruined {
            for i in 0..self.citizens.len() {
                if self.citizens[i].workplace == Some(id) {
                    self.citizens[i].workplace = None;
                    self.citizens[i].job = None;
                    self.citizens[i].abandon();
                }
                if self.citizens[i].dest == Some(nav::Dest::Building(id)) {
                    self.citizens[i].halt();
                }
                if self.citizens[i].home == Some(id) {
                    self.citizens[i].home = None;
                }
            }
        }
        let _ = (CitizenId(0), Kind::Road);
    }
}
