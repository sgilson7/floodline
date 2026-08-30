//! The client.
//!
//! Kept as several small modules from the first commit rather than one file
//! that grows: `screen` owns the letterbox, `buildid` owns the one fact that
//! has to come back out of the page. Phase 5 adds the map, the panel and the
//! input handling as their own modules beside them.

mod buildid;
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

    loop {
        // Clear the whole window, letterbox bars included, then move into
        // logical space for everything after.
        set_default_camera();
        clear_background(Color::from_rgba(8, 8, 12, 255));
        let view = screen::Viewport::current();
        set_camera(&view.camera());
        clear_background(BLACK);

        let title = "FLOODLINE";
        let size = 72;
        let m = measure_text(title, None, size, 1.0);
        draw_text(
            title,
            (screen::LOGICAL_W - m.width) / 2.0,
            screen::LOGICAL_H / 2.0,
            size as f32,
            Color::from_rgba(120, 170, 220, 255),
        );

        draw_text(
            &format!("build {build}"),
            16.0,
            screen::LOGICAL_H - 16.0,
            20.0,
            Color::from_rgba(90, 90, 110, 255),
        );

        next_frame().await
    }
}
