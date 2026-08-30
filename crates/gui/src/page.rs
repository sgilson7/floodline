//! The three things about the page that Rust cannot know without asking.
//!
//! Kept apart from `buildid` because that one answers a question about the
//! *build* and these are about the *page* — the URL a room code arrived in,
//! the link to share it, and the clipboard. Natively there is no page, so the
//! answers are the honest ones: no room, no link, and miniquad's own
//! clipboard, which on a desktop is the real one.

#[cfg(target_arch = "wasm32")]
mod imp {
    use sapp_jsutils::JsObject;

    extern "C" {
        fn fl_url_room() -> JsObject;
        fn fl_share_link(room: JsObject) -> JsObject;
        fn fl_arm_copy(text: JsObject, x: u32, y: u32, w: u32, h: u32);
    }

    fn read(obj: JsObject) -> String {
        let mut out = String::new();
        obj.to_string(&mut out);
        out
    }

    /// `?room=` from the address bar, which is how design §9.4 says an
    /// invitation travels: the host sends a link, not instructions.
    pub fn url_room() -> Option<String> {
        let room = read(unsafe { fl_url_room() });
        if room.is_empty() {
            None
        } else {
            Some(room)
        }
    }

    pub fn share_link(room: &str) -> String {
        read(unsafe { fl_share_link(JsObject::string(room)) })
    }

    /// Tell the page what the Copy button under the cursor would copy, or
    /// nothing when the cursor is elsewhere.
    ///
    /// Armed on hover rather than done on the click, because a browser only
    /// lets a page write to the clipboard while a user gesture is live and
    /// macroquad reads a click in the *next* animation frame, by which time it
    /// is not. The plugin does the copying inside the canvas's own click
    /// handler; this is how it knows what to write.
    pub fn arm_copy(text: Option<&str>, rect: (f32, f32, f32, f32)) {
        let (x, y, w, h) = rect;
        unsafe {
            fl_arm_copy(
                JsObject::string(text.unwrap_or("")),
                x.max(0.0) as u32,
                y.max(0.0) as u32,
                w.max(0.0) as u32,
                h.max(0.0) as u32,
            )
        };
    }

    /// What ctrl-C will produce. miniquad serves this from the page's own
    /// `copy` event, which is always a real gesture — so the keyboard works
    /// even where the button is refused.
    pub fn copy(text: &str) {
        macroquad::miniquad::window::clipboard_set(text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub fn url_room() -> Option<String> {
        None
    }

    pub fn share_link(_room: &str) -> String {
        String::new()
    }

    pub fn copy(text: &str) {
        macroquad::miniquad::window::clipboard_set(text);
    }

    /// Nothing to arm: a desktop clipboard has no rule about gestures.
    pub fn arm_copy(_text: Option<&str>, _rect: (f32, f32, f32, f32)) {}
}

pub use imp::{arm_copy, copy, share_link, url_room};

/// What the player last pasted, or `None`.
///
/// In the browser this is whatever a paste event delivered, which miniquad
/// keeps for us; natively it is the system clipboard. Either way the lobby
/// reads it on ctrl-V rather than polling, because on a desktop reading the
/// clipboard every frame is a syscall every frame.
pub fn paste() -> Option<String> {
    macroquad::miniquad::window::clipboard_get().filter(|s| !s.trim().is_empty())
}
