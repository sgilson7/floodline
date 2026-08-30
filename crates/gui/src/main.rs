//! The client.
//!
//! Kept as several small modules from the first commit rather than one file
//! that grows: `screen` owns the letterbox, `buildid` owns the one fact that
//! has to come back out of the page. Phase 5 adds the map, the panel and the
//! input handling as their own modules beside them.

mod buildid;
mod draw;
mod game;
mod input;
mod lobby;
mod page;
mod palette;
mod screen;
mod tutorial;
mod ui;

use macroquad::prelude::*;

/// Zoom and pan.
///
/// Not in `input`, because it changes what the player is looking at rather
/// than what the world is doing: nothing here can issue a command, and a wheel
/// turn during a flood is not an order anybody has to agree about.
fn camera_controls(ui: &ui::Ui, map: &mut screen::MapView) {
    let (_, wheel) = mouse_wheel();
    if wheel != 0.0 && screen::map_window().contains(ui.mouse) {
        // A fixed step per notch: browsers report wildly different magnitudes
        // for the same flick of the same wheel, so the sign is the only part
        // of it worth trusting.
        map.zoom_about(ui.mouse, if wheel > 0.0 { 1.18 } else { 1.0 / 1.18 });
    }

    // Keys, for anybody who would rather not drag.
    let mut by = Vec2::ZERO;
    let speed = 640.0 * get_frame_time();
    if is_key_down(KeyCode::Left) { by.x -= speed; }
    if is_key_down(KeyCode::Right) { by.x += speed; }
    if is_key_down(KeyCode::Up) { by.y -= speed; }
    if is_key_down(KeyCode::Down) { by.y += speed; }
    if by != Vec2::ZERO {
        map.pan(by);
    }

    // Middle-drag. Not right-drag: right-click is an order.
    if is_mouse_button_down(MouseButton::Middle) {
        map.pan(-ui.moved);
    }
    if is_key_pressed(KeyCode::Key0) {
        map.frame_the_map();
    }
}

/// The fixed timestep.
///
/// The simulation used to advance one tick per rendered frame, which made a
/// day twenty seconds on one machine and fifty on another: measured at 24
/// ticks a second in a headless browser against design §3.1's ten, and about
/// sixty on an ordinary display. Everything counted in ticks went with it —
/// design §8's "thirty seconds of silence before a player is dropped" was
/// really five, because it counts the host's own ticks.
///
/// It also decided the pace of a whole game by the host's frame rate, since
/// the host emits one bundle per call and nobody advances without one.
#[derive(Default)]
struct Clock {
    /// Seconds owed to the simulation but not yet spent.
    owed: f32,
}

impl Clock {
    /// The longest catch-up a single frame may do.
    ///
    /// A tab that was backgrounded for a minute comes back owing thousands of
    /// ticks, and simulating them all in one frame freezes the page for
    /// seconds and then does it again. Dropping the backlog is the right
    /// answer for the peer that fell behind: lockstep will not let it get
    /// ahead of anybody, and the host is waiting for its turns either way.
    const MOST_PER_FRAME: u32 = 8;

    fn reset(&mut self) {
        self.owed = 0.0;
    }

    fn ticks_due(&mut self, elapsed: f32) -> u32 {
        // A frame that took a second is a frame the machine stalled on, not a
        // second of game to make up.
        self.owed += elapsed.min(0.25);
        let step = 1.0 / sim::balance::TICKS_PER_SECOND as f32;
        let mut due = 0;
        while self.owed >= step && due < Self::MOST_PER_FRAME {
            self.owed -= step;
            due += 1;
        }
        if due == Self::MOST_PER_FRAME {
            self.owed = 0.0;
        }
        due
    }
}

fn window() -> Conf {
    Conf {
        window_title: "FLOODLINE".to_owned(),
        window_width: screen::LOGICAL_W as i32,
        window_height: screen::LOGICAL_H as i32,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window)]
async fn main() {
    // A wasm panic is otherwise an opaque "unreachable executed" in the
    // console, which is the least useful thing to be told about a desync.
    // Taken from the workbench, for the same reason it was written there.
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(|info| {
        buildid::log(&format!("floodline panic: {info}"));
    }));

    // Every random choice the client makes — the room code and the seed — and
    // nothing the simulation ever sees. `sim` has its own `Rng` and one seed,
    // handed to it from here.
    macroquad::rand::srand(macroquad::miniquad::date::now() as u64);

    ui::warm_the_font_atlas();

    let build = buildid::build_hash();
    let mut lobby = Some(lobby::Lobby::new());
    let mut session: Option<game::Session> = None;
    let mut input = input::Input::default();
    let mut clock = Clock::default();
    let mut welcome = tutorial::Welcome::default();
    let mut map = screen::MapView::default();

    loop {
        // Clear the whole window, letterbox bars included, then move into
        // logical space for everything after.
        set_default_camera();
        clear_background(palette::BACKDROP);
        let view = screen::Viewport::current();
        set_camera(&view.camera());
        clear_background(palette::BACKDROP);
        let ui = ui::Ui::frame(&view);

        // The session runs whether or not the lobby is on top of it: a joiner
        // sitting in the lobby still has to answer the host, and the host has
        // to hear it arrive.
        //
        // In the lobby that is once a frame, because nothing is being
        // simulated and a handshake should not wait on a clock. In a game it
        // is on the clock, because `advance` is a *tick* — and until this was
        // here the game ran at whatever the display did.
        if let Some(s) = session.as_mut() {
            if s.in_lobby() {
                clock.reset();
                s.advance();
            } else {
                for _ in 0..clock.ticks_due(get_frame_time()) {
                    s.advance();
                }
            }
        }

        if let Some(screen) = lobby.as_mut() {
            match screen.draw(&ui, session.as_mut(), &build) {
                lobby::Act::Nothing => {}
                lobby::Act::Open(new_session, next) => {
                    session = Some(new_session);
                    lobby = Some(next);
                }
                lobby::Act::Play => {
                    if let Some(s) = session.as_mut() {
                        s.start();
                    }
                    lobby = None;
                }
                lobby::Act::Cancel(why) => {
                    let room = screen.room();
                    // Dropping the session closes the room. Before `WebPeer`
                    // had a `Drop` this left the tab squatting it for ever.
                    session = None;
                    lobby = Some(lobby::Lobby::with_notice(why, room));
                }
            }
            // A joiner has no Start button. It leaves the lobby when the
            // host's first bundle arrives, which is the same tick on every
            // peer — see `Lockstep`'s note on `Welcome`.
            if session.as_ref().is_some_and(|s| !s.in_lobby()) {
                lobby = None;
            }
        } else if let Some(s) = session.as_mut() {
            let me = s.me();
            let over = s.world().finished().is_some();
            let busy = over || welcome.showing();

            // The map, through its own camera: clipped to its window, scaled
            // by the zoom, and drawn in map space. Everything the player draws
            // *on* the map goes here too, or the two would disagree the moment
            // anybody zoomed.
            set_camera(&map.camera(&view));
            draw::world(s.world(), me, input::selected(&input), &map);
            if !busy {
                input.map_layer(&ui, s, &map);
            }

            // And back to the canvas for the panel and everything over it.
            set_camera(&view.camera());
            let panel_ends = draw::panel(s.world(), me, s.status(), &build, &s.ticks());
            if over {
                // Nothing left to command, and a build menu over a score
                // screen would only invite clicks that fail.
                draw::score(s.world());
            } else if welcome.showing() {
                // The card is modal: a click that dismisses it must not also
                // put a building down behind it.
                welcome.draw(&ui);
            } else {
                input.panel_layer(&ui, s, panel_ends, &map);
                camera_controls(&ui, &mut map);
            }
            let stopped = matches!(s.status(), net::Status::Ended(_));
            let back = (over || stopped)
                && (is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter));
            if back {
                let why = match s.status() {
                    net::Status::Ended(reason) => reason.clone(),
                    _ => String::new(),
                };
                session = None;
                input = input::Input::default();
                lobby = Some(lobby::Lobby::with_notice(why, None));
            }
        }

        next_frame().await
    }
}
