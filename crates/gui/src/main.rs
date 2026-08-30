//! The client.
//!
//! Kept as several small modules from the first commit rather than one file
//! that grows: `screen` owns the letterbox, `buildid` owns the one fact that
//! has to come back out of the page. Phase 5 adds the map, the panel and the
//! input handling as their own modules beside them.

mod buildid;
mod draw;
mod game;
mod palette;
mod screen;

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

    let build = buildid::build_hash();

    // Two players in one process, wired as a star through `net::Loopback`.
    // Design §7: native builds are for development. Single player is the same
    // lockstep with one peer, so there is no second path through any of this.
    let mut game = game::Local::new(0x_F100_D11E, 2, &build);
    let selected: Vec<sim::CitizenId> = Vec::new();

    loop {
        // Clear the whole window, letterbox bars included, then move into
        // logical space for everything after.
        set_default_camera();
        clear_background(palette::BACKDROP);
        let view = screen::Viewport::current();
        set_camera(&view.camera());
        clear_background(palette::BACKDROP);

        if game.in_lobby() && is_key_pressed(KeyCode::Space) {
            game.start();
        }
        if !game.in_lobby() {
            game.advance();
        }

        let me = game.me();
        draw::world(game.world(), me, &selected);
        draw::panel(game.world(), me, game.status(), &build, &game.ticks());
        if game.world().finished().is_some() {
            draw::score(game.world());
        }

        next_frame().await
    }
}
