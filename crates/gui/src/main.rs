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
mod ui;

use macroquad::prelude::*;

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
        if let Some(s) = session.as_mut() {
            s.advance();
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
            draw::world(s.world(), me, input::selected(&input));
            let panel_ends = draw::panel(s.world(), me, s.status(), &build, &s.ticks());
            let over = s.world().finished().is_some();
            if over {
                // Nothing left to command, and a build menu over a score
                // screen would only invite clicks that fail.
                draw::score(s.world());
            } else {
                input.frame(&ui, s, panel_ends);
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
