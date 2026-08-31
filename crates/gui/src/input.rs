//! Everything a player does with the mouse, and what it turns into.
//!
//! Immediate mode, like `ui`: the tools draw themselves and act in the same
//! pass, so there is no state that can disagree with what is on screen. What
//! leaves this file is always a `Command` handed to `Session::issue`, which is
//! `World::apply` on this machine first — so an illegal placement is refused
//! *now*, under the cursor, instead of being silently dropped three ticks
//! later on every machine at once.
//!
//! **Nothing here reads `mouse_position()`, and nothing here converts.**
//! `Ui::frame` has already put the cursor into logical coordinates and
//! `screen::MapView` is the only thing that turns those into a cell or into
//! map space. Between them those two types are the whole of the coordinate
//! system; this file asks them and never does the arithmetic itself.

use crate::game::Session;
use crate::screen::{MapView, CELL, LOGICAL_H, LOGICAL_W, PANEL_W};
use crate::ui::Ui;
use crate::{palette, ui};
use macroquad::prelude::*;
use sim::building::{Facing, Good, Kind};
use sim::{CitizenId, Command, PlayerId, World};

/// What the next click on the map means.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Tool {
    /// Select citizens; right-click orders them about.
    Select,
    /// Put one of these down. A dike is also raised with this tool, by
    /// clicking one that is already there.
    Build(Kind),
    /// Carrying a building to somewhere else. Press `M` with one selected.
    Moving { building: sim::BuildingId },
    /// Draw a wall. Press where it starts, drag, release where it ends.
    ///
    /// A drag rather than the road tool's two clicks, and its own variant
    /// rather than `Build(Kind::Dike)`, because a wall is the one thing in the
    /// game you draw a length of: two clicks would hide the run and the price
    /// of it behind a second gesture, and the whole point of the ghost is that
    /// you see both before you commit. Pressing and releasing on the same
    /// dike still raises it, which is design §3.3's "dikes grow".
    Wall { from: Option<(u8, u8)> },
    /// Two clicks: where from, where to.
    Road { from: Option<(u8, u8)> },
    /// Point at something, for the other player to see.
    Ping,
}

/// The buildings the MVP lets a player put down, in the order the panel shows
/// them and the order the number keys pick them. `Hearth` is not among them:
/// the run starts with the only one a city gets. `Road` and `Bridge` are laid
/// by the road tool, which routes and bridges by itself (design §6).
const BUILDABLE: [(Kind, &str, KeyCode); 9] = [
    (Kind::Cottage, "cottage", KeyCode::Key1),
    (Kind::Farm, "farm", KeyCode::Key2),
    (Kind::Granary, "granary", KeyCode::Key3),
    (Kind::Forester, "forester", KeyCode::Key4),
    (Kind::Quarry, "quarry", KeyCode::Key5),
    (Kind::Stockpile, "stockpile", KeyCode::Key6),
    (Kind::Dike, "dike", KeyCode::Key7),
    (Kind::TradingPost, "post", KeyCode::Key8),
    (Kind::Nursery, "nursery", KeyCode::Key9),
];

/// A trade being composed. Design §6: a standing daily exchange, proposed by
/// one city and accepted by the other.
struct Draft {
    open: bool,
    with: usize,
    give: (Good, u16),
    take: (Good, u16),
}

impl Default for Draft {
    fn default() -> Draft {
        Draft { open: false, with: 0, give: (Good::Food, 10), take: (Good::Wood, 10) }
    }
}

pub struct Input {
    pub tool: Tool,
    pub selected: Vec<CitizenId>,
    /// Where a selection drag began, in logical coordinates.
    drag: Option<Vec2>,
    trade: Draft,
    /// The last thing the rules said no to, and when, so it fades.
    notice: Option<(String, f64)>,
    /// The building the player last clicked with the select tool. What the
    /// level button and `M` act on.
    chosen: Option<sim::BuildingId>,
    /// Which half of the panel is showing.
    tab: Tab,
    /// Whoever the cursor is over in the households list, rung on the map.
    /// The first thing in this game that connects a list to the world.
    ringed: Vec<CitizenId>,
    /// How many segments the wall under the cursor would be, and what they
    /// would cost. Worked out with the ghost, drawn with the panel.
    wall_hint: Option<(usize, u16)>,
}

/// The two halves of the panel: what you can do, and who is doing it.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Tab {
    Tools,
    Households,
}

/// Which tool puts a kind down. Everything is placed with a click except the
/// dike, which is drawn.
fn tool_for(kind: Kind) -> Tool {
    match kind {
        Kind::Dike => Tool::Wall { from: None },
        _ => Tool::Build(kind),
    }
}

impl Default for Input {
    fn default() -> Input {
        Input { tool: Tool::Select, selected: Vec::new(), drag: None, trade: Draft::default(),
                notice: None, wall_hint: None, chosen: None,
                tab: Tab::Tools, ringed: Vec::new() }
    }
}

/// How long a refusal stays on screen.
const NOTICE_SECONDS: f64 = 4.5;

impl Input {
    /// Everything drawn in map space, under the map camera: the selection
    /// rectangle, the ghost of what is about to be built, the road being laid.
    ///
    /// Split from the panel because the two are drawn through different
    /// cameras — the map is clipped to its window and scaled by the zoom, the
    /// panel is not — and mixing them was the one way this could have gone
    /// wrong quietly.
    pub fn map_layer(&mut self, ui: &Ui, session: &mut Session, view: &MapView) {
        let me = session.me();
        self.keys();
        self.forget_the_dead(session.world());
        if self.trade.open {
            return;
        }
        self.map(ui, session, me, view);
    }

    /// Everything drawn on the logical canvas: the tools, the dialog, and the
    /// line under the map that says why the last order was refused.
    pub fn panel_layer(
        &mut self,
        ui: &Ui,
        session: &mut Session,
        panel_top: f32,
        view: &MapView,
    ) {
        let me = session.me();
        if self.trade.open {
            self.trade_dialog(ui, session, me);
        }
        let top = self.tabs(ui, panel_top);
        match self.tab {
            Tab::Tools => self.tools(ui, session, me, top, view),
            Tab::Households => self.households(ui, session, me, top),
        }
        self.wall_cost(ui);
        self.notice();
    }

    fn keys(&mut self) {
        for (kind, _, key) in BUILDABLE {
            if is_key_pressed(key) {
                self.tool = tool_for(kind);
            }
        }
        if is_key_pressed(KeyCode::R) {
            self.tool = Tool::Road { from: None };
        }
        if is_key_pressed(KeyCode::P) {
            self.tool = Tool::Ping;
        }
        if is_key_pressed(KeyCode::M) {
            if let Some(building) = self.chosen {
                self.tool = Tool::Moving { building };
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            self.tool = Tool::Select;
            self.trade.open = false;
        }
    }

    /// A citizen who has died stops being selected.
    ///
    /// Not tidiness: `World::apply` refuses a command naming a dead citizen,
    /// and it refuses the *whole* command (see DECISIONS.md, "Commands are
    /// all-or-nothing), so one drowned villager in the selection would
    /// silently cancel every order given to the other seven — during a flood,
    /// which is when it matters most.
    fn forget_the_dead(&mut self, w: &World) {
        self.selected.retain(|id| {
            w.citizens.get(id.0 as usize).is_some_and(|c| c.alive())
        });
    }

    fn say(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), get_time()));
    }

    fn issue(&mut self, session: &mut Session, cmd: Command) {
        if let Err(e) = session.issue(cmd) {
            self.say(e.to_message());
        }
    }

    /// Send the ones that fit, and say what happened to the rest.
    ///
    /// A command is all-or-nothing (DECISIONS.md), which is right on the wire
    /// and wrong under a mouse: choosing a whole city of eight and
    /// right-clicking a farm asks to put eight people in three slots, and the
    /// rules answer `Full` — so *nobody* is assigned, the farm stands empty,
    /// and the city starves on day four. That is the most natural gesture in
    /// the game and it did nothing at all, with a red line under the map that
    /// fades in three seconds as the only sign.
    ///
    /// So the mouse asks first, and only sends what will be taken. The rule in
    /// `sim` does not move: `will_take` and `will_house` are the same
    /// arithmetic `assign` and `SetHome` use, next to them, so this cannot
    /// drift out of step with what the rules will accept.
    fn send_as_many_as_fit(
        &mut self,
        session: &mut Session,
        citizens: Vec<CitizenId>,
        room: usize,
        what: &str,
        cmd: impl Fn(Vec<CitizenId>) -> Command,
    ) {
        if room == 0 {
            self.say(format!("no {what} left there"));
            return;
        }
        let wanted = citizens.len();
        let taken: Vec<CitizenId> = citizens.into_iter().take(room).collect();
        let sent = taken.len();
        self.issue(session, cmd(taken));
        if sent < wanted {
            self.say(format!("{sent} of {wanted} - that is all the {what} there is"));
        }
    }

    // ---- the map ------------------------------------------------------------

    fn map(&mut self, ui: &Ui, session: &mut Session, me: PlayerId, view: &MapView) {
        let cell = view.cell_at(ui.mouse);
        self.hover(session.world(), me, cell);

        match self.tool {
            Tool::Select => self.select(ui, session, me, cell, view),
            Tool::Build(kind) => {
                if let (true, Some((x, y))) = (ui.clicked, cell) {
                    // Everything this tool places is square, so east-west is
                    // the only answer that means anything here. `tool_for`
                    // sends the one kind that is not down the wall tool.
                    self.issue(session, Command::Place {
                        kind,
                        facing: Facing::EastWest,
                        x: x as u8,
                        y: y as u8,
                    });
                }
                if ui.right_clicked {
                    self.tool = Tool::Select;
                }
            }
            Tool::Moving { building } => {
                if let (true, Some((x, y))) = (ui.clicked, cell) {
                    self.issue(session, Command::Move {
                        building,
                        x: x as u8,
                        y: y as u8,
                    });
                    self.tool = Tool::Select;
                }
                if ui.right_clicked {
                    self.tool = Tool::Select;
                }
            }
            Tool::Wall { from } => {
                // The anchor is tracked through this frame rather than read
                // back out of `self.tool`, because a click fast enough to go
                // down and up inside one frame arrives with `clicked` and
                // `released` both set — and a wall tool that ignored quick
                // clicks would also have stopped raising dikes.
                let mut start = from;
                if let (true, Some((x, y))) = (ui.clicked, cell) {
                    start = Some((x as u8, y as u8));
                    self.tool = Tool::Wall { from: start };
                }
                if ui.released {
                    if let (Some(start), Some((x, y))) = (start, cell) {
                        let end = (x as u8, y as u8);
                        // A press and a release on the same dike is a click,
                        // and a click on a dike raises it.
                        let raise = session
                            .world()
                            .building_at(x, y)
                            .filter(|b| b.owner == me && b.kind == Kind::Dike && start == end)
                            .map(|b| b.id);
                        match raise {
                            Some(dike) => self.issue(session, Command::RaiseDike { dike }),
                            None => {
                                self.issue(session, Command::DikeLine { from: start, to: end })
                            }
                        }
                    }
                    self.tool = Tool::Wall { from: None };
                }
                if ui.right_clicked {
                    self.tool = Tool::Select;
                }
            }
            Tool::Road { from } => {
                if let (true, Some((x, y))) = (ui.clicked, cell) {
                    match from {
                        None => self.tool = Tool::Road { from: Some((x as u8, y as u8)) },
                        Some(start) => {
                            self.issue(session, Command::Road {
                                from: start,
                                to: (x as u8, y as u8),
                            });
                            self.tool = Tool::Road { from: None };
                        }
                    }
                }
                if ui.right_clicked {
                    self.tool = Tool::Select;
                }
            }
            Tool::Ping => {
                if let (true, Some((x, y))) = (ui.clicked, cell) {
                    self.issue(session, Command::Ping { x: x as u8, y: y as u8 });
                    self.tool = Tool::Select;
                }
            }
        }
    }

    /// Drag to select, right-click to order.
    fn select(
        &mut self,
        ui: &Ui,
        session: &mut Session,
        me: PlayerId,
        cell: Option<(i32, i32)>,
        view: &MapView,
    ) {
        // In map space, so a rectangle dragged at one zoom means the same
        // cells at any other.
        let here = view.to_map(ui.mouse);
        if ui.clicked && cell.is_some() {
            self.drag = Some(here);
        }
        if let Some(start) = self.drag {
            let r = rect_between(start, here);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0 / view.zoom, palette::INK);
            if ui.released {
                self.drag = None;
                // A click is a drag of no size, and picking the one citizen
                // under the cursor wants a little tolerance rather than an
                // exact hit on a body a cell wide.
                let r = if r.w < CELL / 2.0 && r.h < CELL / 2.0 {
                    Rect::new(start.x - CELL, start.y - CELL, CELL * 2.0, CELL * 2.0)
                } else {
                    r
                };
                self.selected = session
                    .world()
                    .citizens
                    .iter()
                    .filter(|c| c.owner == me && c.alive())
                    .filter(|c| r.contains(citizen_at(c)))
                    .map(|c| c.id)
                    .collect();
                // And whichever of your own buildings was under the click.
                // Clicking people and clicking a building are the same gesture
                // — a click on a farm with three farmers standing in it should
                // not make you choose which one you meant.
                self.chosen = cell
                    .and_then(|(x, y)| session.world().building_at(x, y))
                    .filter(|b| b.owner == me)
                    .map(|b| b.id);
            }
        }

        if ui.right_clicked && !self.selected.is_empty() {
            let Some((x, y)) = cell else { return };
            let citizens = self.selected.clone();
            let target = session.world().building_at(x, y).map(|b| (b.id, b.owner, b.kind));
            match target {
                // Somebody else's building is not a place to work.
                Some((id, owner, kind)) if owner == me => match kind {
                    Kind::Cottage => {
                        let room = session.world().will_house(me, id, &citizens);
                        self.send_as_many_as_fit(session, citizens, room, "beds", |c| {
                            Command::SetHome { citizens: c, cottage: id }
                        });
                    }
                    _ => {
                        let room = session.world().will_take(me, id, &citizens);
                        self.send_as_many_as_fit(session, citizens, room, "room", |c| {
                            Command::Assign { citizens: c, building: id }
                        });
                    }
                },
                _ => self.issue(session, Command::MoveTo {
                    citizens,
                    x: x as u8,
                    y: y as u8,
                }),
            }
        }
    }

    /// What the cursor is over: a ghost of what would be built, or the name of
    /// what is already there.
    fn hover(&mut self, w: &World, me: PlayerId, cell: Option<(i32, i32)>) {
        self.wall_hint = None;
        let Some((x, y)) = cell else { return };
        match self.tool {
            Tool::Moving { building } => {
                // Where it would land, and whether it may. A move is validated
                // ignoring the building's own cells, so shuffling one step
                // shows green rather than red.
                let Some(b) = w.buildings.get(building.0 as usize) else { return };
                let (bw, bh) = b.size();
                let ok = w.can_move(me, building, x, y).is_ok();
                draw_rectangle(
                    x as f32 * CELL,
                    y as f32 * CELL,
                    bw as f32 * CELL,
                    bh as f32 * CELL,
                    Color { a: 0.35, ..if ok { palette::GOOD } else { palette::ALARM } },
                );
            }
            Tool::Wall { from } => {
                // The ghost is the run `sim` would actually lay, asked of the
                // same function that will lay it, so a player cannot be shown
                // one wall and sold another.
                let start = from.unwrap_or((x as u8, y as u8));
                let end = (x as u8, y as u8);
                let plan = w.plan_dike_line(me, start, end);
                for (sx, sy) in &plan {
                    let (bw, bh) = Kind::Dike.size(Facing::of_run(
                        (start.0 as i32, start.1 as i32),
                        (x, y),
                    ));
                    draw_rectangle(
                        *sx as f32 * CELL,
                        *sy as f32 * CELL,
                        bw as f32 * CELL,
                        bh as f32 * CELL,
                        Color { a: 0.35, ..palette::GOOD },
                    );
                }
                if plan.is_empty() {
                    draw_rectangle(
                        x as f32 * CELL,
                        y as f32 * CELL,
                        CELL,
                        CELL,
                        Color { a: 0.35, ..palette::ALARM },
                    );
                }
                self.wall_hint = Some((
                    plan.len(),
                    Kind::Dike.cost().stone.saturating_mul(plan.len() as u16),
                ));
            }
            Tool::Build(kind) => {
                let (bw, bh) = kind.size(Facing::EastWest);
                let ok = w.can_place(me, kind, Facing::EastWest, x, y).is_ok();
                draw_rectangle(
                    x as f32 * CELL,
                    y as f32 * CELL,
                    bw as f32 * CELL,
                    bh as f32 * CELL,
                    Color { a: 0.35, ..if ok { palette::GOOD } else { palette::ALARM } },
                );
            }
            Tool::Road { from: Some((fx, fy)) } => {
                draw_line(
                    fx as f32 * CELL + CELL / 2.0,
                    fy as f32 * CELL + CELL / 2.0,
                    x as f32 * CELL + CELL / 2.0,
                    y as f32 * CELL + CELL / 2.0,
                    2.0,
                    palette::WARNING,
                );
            }
            _ => {}
        }
    }

    // ---- the panel ----------------------------------------------------------

    /// The two tab buttons, and where the panel body starts under them.
    fn tabs(&mut self, ui: &Ui, top: f32) -> f32 {
        let left = LOGICAL_W - PANEL_W + 18.0;
        let wide = PANEL_W - 36.0;
        let half = (wide - 8.0) / 2.0;
        let y = top + 10.0;
        for (i, (tab, label)) in
            [(Tab::Tools, "build"), (Tab::Households, "households")].into_iter().enumerate()
        {
            let r = Rect::new(left + i as f32 * (half + 8.0), y, half, 28.0);
            if ui.button(r, label, true) {
                self.tab = tab;
                self.ringed.clear();
            }
            if self.tab == tab {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, palette::INK);
            }
        }
        y + 30.0
    }

    /// One chip per household: who, where, how many children, and how close
    /// the next one is.
    ///
    /// Hovering a chip rings its people on the map, which is only useful
    /// because there is a camera to see them at — and is the first thing in
    /// this game that connects a list to the world.
    fn households(&mut self, ui: &Ui, session: &mut Session, me: PlayerId, top: f32) {
        let left = LOGICAL_W - PANEL_W + 18.0;
        let wide = PANEL_W - 36.0;
        let mut y = top + 8.0;
        self.ringed.clear();

        let w = session.world();
        let mine: Vec<&sim::Household> =
            w.households.iter().filter(|h| h.owner == me && h.alive()).collect();
        if mine.is_empty() {
            draw_text(
                "nobody shares a cottage yet",
                left,
                y + 16.0,
                15.0,
                palette::FAINT,
            );
            draw_text(
                "put two people in one and give them a day",
                left,
                y + 34.0,
                15.0,
                palette::FAINT,
            );
            return;
        }

        for h in mine {
            let chip = Rect::new(left, y, wide, 40.0);
            let over = chip.contains(ui.mouse);
            draw_rectangle(chip.x, chip.y, chip.w, chip.h, palette::BUTTON);
            draw_rectangle_lines(
                chip.x,
                chip.y,
                chip.w,
                chip.h,
                1.0,
                if over { palette::INK } else { palette::RULE },
            );
            let names: Vec<&str> = h
                .members
                .iter()
                .filter_map(|id| w.citizens.get(id.0 as usize))
                .map(|c| sim::names::NAMES[c.name as usize])
                .collect();
            draw_text(&names.join(" and "), chip.x + 8.0, chip.y + 17.0, 16.0, palette::INK);
            let line = if !h.settled() {
                "settling in".to_owned()
            } else {
                format!("{} children - next {}%", h.children.len(), h.expecting())
            };
            draw_text(&line, chip.x + 8.0, chip.y + 33.0, 14.0, palette::FAINT);

            if over {
                self.ringed = h.members.to_vec();
                self.ringed.extend(h.children.iter().copied());
            }
            y += 46.0;
        }
    }

    fn tools(&mut self, ui: &Ui, session: &mut Session, me: PlayerId, top: f32, view: &MapView) {
        let left = LOGICAL_W - PANEL_W + 18.0;
        let wide = PANEL_W - 36.0;
        let half = (wide - 8.0) / 2.0;
        let mut y = top + 8.0;

        draw_line(left, y, left + wide, y, 1.0, palette::RULE);
        y += 22.0;
        draw_text("BUILD", left, y, 15.0, palette::FAINT);
        y += 14.0;

        let goods = session.world().treasury(me);
        for (i, (kind, name, key)) in BUILDABLE.iter().enumerate() {
            let r = Rect::new(
                left + (i % 2) as f32 * (half + 8.0),
                y + (i / 2) as f32 * 42.0,
                half,
                36.0,
            );
            let cost = kind.cost();
            let afford = goods.food >= cost.food && goods.wood >= cost.wood
                && goods.stone >= cost.stone;
            let label = format!("{} {}", i + 1, name);
            if ui.button(r, &label, true) {
                self.tool = tool_for(*kind);
            }
            if self.tool == tool_for(*kind) {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, palette::INK);
            }
            // The cost, under the name, greyed when it is out of reach. Not a
            // refusal — the materials are hauled to a site over time and a
            // player may well want to start one they cannot yet pay for.
            draw_text(
                &cost_line(kind.cost()),
                r.x + 6.0,
                r.y + r.h - 4.0,
                13.0,
                if afford { palette::FAINT } else { palette::ALARM },
            );
            let _ = key;
        }
        y += 42.0 * ((BUILDABLE.len() as f32 + 1.0) / 2.0).floor() + 8.0;

        // The dike is picked from the build menu above like anything else; it
        // is the gesture that differs, not the shopping.
        let road = Rect::new(left, y, half, 36.0);
        let ping = Rect::new(left + half + 8.0, y, half, 36.0);
        if ui.button(road, "r road", true) {
            self.tool = Tool::Road { from: None };
        }
        if matches!(self.tool, Tool::Road { .. }) {
            draw_rectangle_lines(road.x, road.y, road.w, road.h, 2.0, palette::INK);
        }
        if ui.button(ping, "p point", true) {
            self.tool = Tool::Ping;
        }
        if self.tool == Tool::Ping {
            draw_rectangle_lines(ping.x, ping.y, ping.w, ping.h, 2.0, palette::INK);
        }
        y += 48.0;

        draw_text(
            match self.tool {
                Tool::Select => "drag to choose. right-click to send them",
                Tool::Build(_) => "click the ground. right-click to stop",
                Tool::Moving { .. } => "click where it should stand. right-click to stop",
                Tool::Wall { from: None } => "drag to draw a wall. click one to raise it",
                Tool::Wall { from: Some(_) } => "let go where the wall should end",
                Tool::Road { from: None } => "click where the road starts",
                Tool::Road { from: Some(_) } => "click where it ends",
                Tool::Ping => "click what you want them to look at",
            },
            left,
            y,
            14.0,
            palette::FAINT,
        );
        y += 22.0;
        // What you have chosen, and the two things you can do to it. Only
        // drawn when there is something to do: a row of dead buttons is a row
        // a player learns to stop reading.
        //
        // **Below everything fixed, and that is not a layout preference.**
        // This row appears and disappears as a player clicks about, so
        // anything under it would move under the cursor — and every browser
        // check clicks the panel at a written-down coordinate. The panel has
        // shifted five times now and each time it silently broke two of them.
        if let Some(id) = self.chosen {
            let w = session.world();
            if let Some(b) = w.buildings.get(id.0 as usize).filter(|b| b.owner == me) {
                let (kind, level) = (b.kind, b.level);
                let can_level = kind.upgradable() && b.standing_now() && level < sim::balance::MAX_LEVEL;
                if can_level || kind.movable() {
                    let up = Rect::new(left, y, half, 36.0);
                    let mv = Rect::new(left + half + 8.0, y, half, 36.0);
                    if can_level {
                        let cost = kind.upgrade_cost(level);
                        if ui.button(up, &format!("level {}", level + 1), true) {
                            self.issue(session, Command::Upgrade { building: id });
                        }
                        draw_text(
                            &cost_line(cost),
                            up.x + 6.0,
                            up.y + up.h - 4.0,
                            13.0,
                            if goods.covers(&cost) { palette::FAINT } else { palette::ALARM },
                        );
                    }
                    if kind.movable() && ui.button(mv, "m move", true) {
                        self.tool = Tool::Moving { building: id };
                    }
                    if matches!(self.tool, Tool::Moving { .. }) {
                        draw_rectangle_lines(mv.x, mv.y, mv.w, mv.h, 2.0, palette::INK);
                    }
                    y += 48.0;
                }
            } else {
                self.chosen = None;
            }
        }

        // What the cursor is over, so a farm's three slots are visible before
        // the click rather than as a refusal after it. Its own line, kept
        // clear whether or not there is anything under the mouse, so nothing
        // below it moves as the cursor crosses a building.
        if let Some(line) = self.under_the_cursor(session.world(), me, ui, view) {
            draw_text(&line, left, y, 15.0, palette::INK);
        }
        y += 22.0;

        // Who is chosen, and the one order that has no gesture of its own.
        draw_line(left, y, left + wide, y, 1.0, palette::RULE);
        y += 22.0;
        draw_text(
            &match self.selected.len() {
                0 => "nobody chosen".to_owned(),
                1 => "1 chosen".to_owned(),
                n => format!("{n} chosen"),
            },
            left,
            y,
            17.0,
            if self.selected.is_empty() { palette::FAINT } else { palette::INK },
        );
        y += 12.0;
        let chosen = !self.selected.is_empty();
        if ui.button(Rect::new(left, y, half, 34.0), "back to hauling", chosen) {
            let citizens = self.selected.clone();
            self.issue(session, Command::Unassign { citizens });
        }
        if ui.button(Rect::new(left + half + 8.0, y, half, 34.0), "choose all", true) {
            self.selected = session
                .world()
                .citizens
                .iter()
                .filter(|c| c.owner == me && c.alive())
                .map(|c| c.id)
                .collect();
        }
        y += 48.0;

        // Trade, and anything waiting for an answer.
        draw_line(left, y, left + wide, y, 1.0, palette::RULE);
        y += 22.0;
        let others = session.world().players.len() > 1;
        if ui.button(Rect::new(left, y, wide, 34.0), "propose a trade", others) {
            self.trade.open = true;
        }
        y += 42.0;
        y = self.offers(ui, session, me, left, wide, y);
        let _ = y;
    }

    /// The building under the mouse, and how full it is.
    fn under_the_cursor(&self, w: &World, me: PlayerId, ui: &Ui, view: &MapView) -> Option<String> {
        let (x, y) = view.cell_at(ui.mouse)?;
        let b = w.building_at(x, y)?;
        let name = kind_name(b.kind);
        if b.owner != me {
            return Some(format!("city {}'s {name}", b.owner.0));
        }
        if !b.standing_now() {
            let want = b.outstanding();
            return Some(if want.is_empty() {
                format!("{name}: being built")
            } else {
                format!("{name}: waiting for {}", cost_line(want))
            });
        }
        let here = |f: fn(&sim::Citizen, sim::BuildingId) -> bool| {
            w.citizens.iter().filter(|c| c.alive() && f(c, b.id)).count()
        };
        Some(match b.kind {
            k if sim::citizen::Job::at(k).is_some() => format!(
                "{name}: {} of {} working{}",
                here(|c, id| c.workplace == Some(id)),
                b.slots_for(sim::citizen::Job::at(k).expect("just matched")),
                level_note(b),
            ),
            Kind::Cottage => format!(
                "{name}: {} of {} beds taken{}",
                here(|c, id| c.home == Some(id)),
                b.beds(),
                level_note(b),
            ),
            Kind::Dike => format!("{name}: level {} of {}", b.level, sim::balance::DIKE_MAX_LEVEL),
            _ => format!("{name}: {}", goods_line(b.store)),
        })
    }

    /// Roads and trades that are waiting on this player to say yes.
    fn offers(
        &mut self,
        ui: &Ui,
        session: &mut Session,
        me: PlayerId,
        left: f32,
        wide: f32,
        mut y: f32,
    ) -> f32 {
        let roads: Vec<_> = session
            .world()
            .roads
            .iter()
            .filter(|r| r.reaches == Some(me) && !r.joined)
            .map(|r| (r.id, r.by))
            .collect();
        for (id, by) in roads {
            if ui.button(
                Rect::new(left, y, wide, 32.0),
                &format!("join city {}'s road", by.0),
                true,
            ) {
                self.issue(session, Command::AcceptRoad { road: id });
            }
            y += 38.0;
        }

        let trades: Vec<_> = session
            .world()
            .trades
            .iter()
            .filter(|t| t.with == me && !t.accepted)
            .map(|t| (t.id, t.from, t.give, t.take))
            .collect();
        for (id, from, give, take) in trades {
            // `give` and `take` are named from the proposer's side, so this
            // player receives what the other gives. Saying it the wrong way
            // round would be a trap in the one screen where a mistake costs
            // real food.
            if ui.button(
                Rect::new(left, y, wide, 32.0),
                &format!(
                    "city {}: {} {} for your {} {}",
                    from.0, give.1, good_name(give.0), take.1, good_name(take.0)
                ),
                true,
            ) {
                self.issue(session, Command::AcceptTrade { trade: id });
            }
            y += 38.0;
        }
        y
    }

    // ---- the trade dialog ---------------------------------------------------

    fn trade_dialog(&mut self, ui: &Ui, session: &mut Session, me: PlayerId) {
        let others: Vec<PlayerId> = session
            .world()
            .players
            .iter()
            .copied()
            .filter(|&p| p != me)
            .collect();
        if others.is_empty() {
            self.trade.open = false;
            return;
        }
        self.trade.with %= others.len();
        let with = others[self.trade.with];

        let card = Rect::new(340.0, 260.0, 620.0, 420.0);
        draw_rectangle(card.x, card.y, card.w, card.h, Color { a: 0.97, ..palette::PANEL });
        draw_rectangle_lines(card.x, card.y, card.w, card.h, 2.0, palette::RULE);
        let cx = card.x + card.w / 2.0;
        let mut y = card.y + 54.0;
        let m = measure_text("A STANDING TRADE", None, 26, 1.0);
        draw_text("A STANDING TRADE", cx - m.width / 2.0, y, 26.0, palette::INK);
        y += 20.0;
        let m = measure_text("every day, until one of you stops it", None, 16, 1.0);
        draw_text(
            "every day, until one of you stops it",
            cx - m.width / 2.0,
            y,
            16.0,
            palette::FAINT,
        );
        y += 44.0;

        draw_text("with", card.x + 40.0, y + 24.0, 18.0, palette::INK);
        if ui.button(Rect::new(card.x + 130.0, y, 200.0, 34.0), &format!("city {}", with.0), true)
        {
            self.trade.with = (self.trade.with + 1) % others.len();
        }
        y += 56.0;

        y = self.trade_row(ui, card, y, "you give", true);
        y = self.trade_row(ui, card, y, "you get", false);
        y += 14.0;

        let propose = Rect::new(cx - 220.0, y, 210.0, 44.0);
        let close = Rect::new(cx + 10.0, y, 210.0, 44.0);
        if ui.button(propose, "propose it", true) {
            let (give, take) = (self.trade.give, self.trade.take);
            self.issue(session, Command::Trade { with, give, take });
            self.trade.open = false;
        }
        if ui.button(close, "never mind", true) {
            self.trade.open = false;
        }
    }

    fn trade_row(&mut self, ui: &Ui, card: Rect, y: f32, label: &str, giving: bool) -> f32 {
        draw_text(label, card.x + 40.0, y + 24.0, 18.0, palette::INK);
        let (good, amount) = if giving { self.trade.give } else { self.trade.take };
        if ui.button(Rect::new(card.x + 130.0, y, 150.0, 34.0), good_name(good), true) {
            // Round the three things a hauler can carry. Gold is not one of
            // them (`Good::hauled`): barter is design §6's standing exchange
            // walked by people, and coins are what the mules bring back.
            let next = match good {
                Good::Food => Good::Wood,
                Good::Wood => Good::Stone,
                _ => Good::Food,
            };
            if giving {
                self.trade.give.0 = next;
            } else {
                self.trade.take.0 = next;
            }
        }
        if ui.button(Rect::new(card.x + 300.0, y, 40.0, 34.0), "-", amount > 5) {
            let slot = if giving { &mut self.trade.give.1 } else { &mut self.trade.take.1 };
            *slot -= 5;
        }
        ui::centred_in(
            Rect::new(card.x + 340.0, y, 70.0, 34.0),
            &amount.to_string(),
            20.0,
            palette::INK,
        );
        if ui.button(Rect::new(card.x + 410.0, y, 40.0, 34.0), "+", amount < 200) {
            let slot = if giving { &mut self.trade.give.1 } else { &mut self.trade.take.1 };
            *slot += 5;
        }
        y + 50.0
    }

    /// The last refusal, under the map, fading.
    /// The length and the price of the wall under the cursor.
    ///
    /// Drawn here rather than with the ghost because the ghost is in map
    /// space, where the camera would blow this up or shrink it away; a label
    /// is the same size at every zoom.
    fn wall_cost(&self, ui: &Ui) {
        let Some((segments, stone)) = self.wall_hint else { return };
        if segments == 0 {
            return;
        }
        let text = format!("{segments} x dike - {stone} stone");
        let m = measure_text(&text, None, 16, 1.0);
        let x = (ui.mouse.x + 14.0).min(LOGICAL_W - PANEL_W - m.width - 8.0);
        let y = (ui.mouse.y - 10.0).max(m.height + 4.0);
        draw_rectangle(x - 5.0, y - m.height - 3.0, m.width + 10.0, m.height + 8.0,
                       Color { a: 0.82, ..palette::PANEL });
        draw_text(&text, x, y, 16.0, palette::INK);
    }

    fn notice(&mut self) {
        let Some((text, at)) = &self.notice else { return };
        let age = get_time() - at;
        if age > NOTICE_SECONDS {
            self.notice = None;
            return;
        }
        // Held at full strength for most of its life and then faded, on a
        // dark plate. It used to fade from the first frame straight into the
        // terrain, which made the one piece of feedback the game gives when a
        // command is refused into something you could look straight past —
        // and did.
        let fade = (1.0 - (age / NOTICE_SECONDS) as f32).min(0.35) / 0.35;
        let m = measure_text(text, None, 22, 1.0);
        let x = (LOGICAL_W - PANEL_W) / 2.0 - m.width / 2.0;
        let plate = Rect::new(x - 18.0, LOGICAL_H - 52.0, m.width + 36.0, 36.0);
        draw_rectangle(plate.x, plate.y, plate.w, plate.h, Color { a: 0.82 * fade, ..palette::PANEL });
        draw_rectangle_lines(plate.x, plate.y, plate.w, plate.h, 1.0,
                             Color { a: fade, ..palette::ALARM });
        draw_text(text, x, LOGICAL_H - 26.0, 22.0, Color { a: fade, ..palette::ALARM });
    }
}

/// A citizen's position in map space, which is where the selection rectangle
/// is too, so the two need no conversion between them.
fn citizen_at(c: &sim::Citizen) -> Vec2 {
    vec2(c.pos.x.raw() as f32 / 256.0 * CELL, c.pos.y.raw() as f32 / 256.0 * CELL)
}

fn rect_between(a: Vec2, b: Vec2) -> Rect {
    Rect::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs())
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Hearth => "hearth",
        Kind::Cottage => "cottage",
        Kind::Farm => "farm",
        Kind::Forester => "forester",
        Kind::Quarry => "quarry",
        Kind::Granary => "granary",
        Kind::Stockpile => "stockpile",
        Kind::TradingPost => "trading post",
        Kind::Nursery => "nursery",
        Kind::Dike => "dike",
        Kind::Road => "road",
        Kind::Bridge => "bridge",
    }
}

/// What a store is holding, or that it is empty.
fn goods_line(g: sim::building::Goods) -> String {
    let line = cost_line(g);
    if line == "free" {
        "empty".to_owned()
    } else {
        line
    }
}

/// " (level 2)", or nothing at a building nobody has paid to grow.
fn level_note(b: &sim::Building) -> String {
    if b.level > 1 {
        format!(" (level {})", b.level)
    } else {
        String::new()
    }
}

fn good_name(g: Good) -> &'static str {
    match g {
        Good::Food => "food",
        Good::Wood => "wood",
        Good::Stone => "stone",
        Good::Gold => "gold",
    }
}

fn cost_line(c: sim::building::Goods) -> String {
    // Every good, in one loop rather than a line each, so a fifth one cannot
    // be added and silently left off a price tag.
    let mut parts: Vec<String> = Vec::new();
    for g in Good::ALL {
        if c.get(g) > 0 {
            parts.push(format!("{} {}", c.get(g), good_name(g)));
        }
    }
    if parts.is_empty() {
        "free".to_owned()
    } else {
        parts.join("  ")
    }
}

/// Selected citizens, for the renderer's owner ring.
pub fn selected(input: &Input) -> &[CitizenId] {
    &input.selected
}

/// Whoever the households list is pointing at.
pub fn ringed(input: &Input) -> &[CitizenId] {
    &input.ringed
}

