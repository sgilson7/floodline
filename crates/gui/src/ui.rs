//! Buttons, fields and hit-testing, in logical coordinates only.
//!
//! Immediate mode: there is no widget tree, a button is a rectangle and a
//! label, and whether it was pressed is the return value. That suits a game
//! whose interface is a lobby and a side panel, and it means no state can get
//! out of step with what is drawn.
//!
//! **Everything here is in logical pixels**, the 1600 × 980 the game draws in.
//! `Ui::frame` is the one place the real mouse is read, and it converts once
//! through `screen::Viewport::mouse`. Nothing below this line may call
//! `mouse_position()` — see the note on `Viewport`, and the deployed bug that
//! note exists because of.

use crate::palette;
use crate::screen::Viewport;
use macroquad::prelude::*;

/// One frame's worth of input, already in logical coordinates.
pub struct Ui {
    pub mouse: Vec2,
    /// The left button went down this frame.
    pub clicked: bool,
    /// The right button went down this frame. Phase 5's assign and move-to.
    #[allow(dead_code)]
    pub right_clicked: bool,
    /// The left button is held.
    pub held: bool,
    /// The left button came up this frame. Phase 5's drag-select.
    #[allow(dead_code)]
    pub released: bool,
    /// Modifier for the shortcuts a lobby needs: ctrl-V, ctrl-C.
    pub ctrl: bool,
}

impl Ui {
    pub fn frame(view: &Viewport) -> Ui {
        let (mx, my) = view.mouse();
        Ui {
            mouse: vec2(mx, my),
            clicked: is_mouse_button_pressed(MouseButton::Left),
            right_clicked: is_mouse_button_pressed(MouseButton::Right),
            held: is_mouse_button_down(MouseButton::Left),
            released: is_mouse_button_released(MouseButton::Left),
            ctrl: is_key_down(KeyCode::LeftControl)
                || is_key_down(KeyCode::RightControl)
                || is_key_down(KeyCode::LeftSuper)
                || is_key_down(KeyCode::RightSuper),
        }
    }

    pub fn over(&self, r: Rect) -> bool {
        r.contains(self.mouse)
    }

    /// Draw a button and say whether it was just pressed.
    pub fn button(&self, r: Rect, label: &str, enabled: bool) -> bool {
        let over = enabled && self.over(r);
        let fill = if !enabled {
            palette::PANEL
        } else if over && self.held {
            palette::RULE
        } else {
            palette::BUTTON
        };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            1.0,
            if over { palette::INK } else { palette::RULE },
        );
        let size = 19.0;
        let m = measure_text(label, None, size as u16, 1.0);
        draw_text(
            label,
            r.x + (r.w - m.width) / 2.0,
            r.y + (r.h + m.offset_y) / 2.0 - 1.0,
            size,
            if enabled { palette::INK } else { palette::FAINT },
        );
        enabled && over && self.clicked
    }

    /// A one-line text box. Editing is the caller's job; this draws it.
    pub fn field(&self, r: Rect, text: &str, hint: &str, focused: bool) {
        draw_rectangle(r.x, r.y, r.w, r.h, palette::FIELD);
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            1.0,
            if focused { palette::INK } else { palette::RULE },
        );
        let (shown, colour) = if text.is_empty() {
            (hint, palette::FAINT)
        } else {
            (text, palette::INK)
        };
        draw_text(shown, r.x + 10.0, r.y + r.h / 2.0 + 6.0, 19.0, colour);
        if focused && (get_time() * 2.0) as i64 % 2 == 0 {
            let m = measure_text(text, None, 19, 1.0);
            let x = r.x + 11.0 + if text.is_empty() { 0.0 } else { m.width };
            draw_line(x, r.y + 9.0, x, r.y + r.h - 9.0, 1.0, palette::INK);
        }
    }
}

/// A string being typed into, with the two keys that actually matter.
#[derive(Default)]
pub struct Field {
    pub text: String,
    /// The last thing the clipboard was seen holding.
    ///
    /// Pasting is not a keystroke, and treating it as one does not work. In
    /// the browser a paste is an event the page receives, which miniquad
    /// stashes; the ctrl-V that caused it arrives as a *separate* keydown, and
    /// which of the two the frame loop sees first is not defined — so reading
    /// the clipboard "when ctrl-V is pressed" reads it either a frame early or
    /// not at all. Worse, the shortcut is ⌘V on a Mac and ctrl-V everywhere
    /// else, and the browser only raises the event for the right one. So the
    /// field watches what the clipboard holds instead and takes it when it
    /// changes, which is true of every platform and every shortcut.
    seen: Option<String>,
}

impl Field {
    pub fn with(text: &str) -> Field {
        Field { text: text.to_owned(), seen: None }
    }

    /// Take this frame's keystrokes and any paste. Returns true on Enter.
    pub fn edit(&mut self, ui: &Ui, limit: usize) -> bool {
        // Drained whether focused or not: `get_char_pressed` is a queue, and a
        // character left in it turns up in whatever field is focused next.
        while let Some(c) = get_char_pressed() {
            if ui.ctrl {
                continue;
            }
            if !c.is_control() && self.text.chars().count() <= limit {
                self.text.push(c);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.text.pop();
        }
        let clipboard = crate::page::paste();
        if clipboard.is_some() && clipboard != self.seen {
            self.seen = clipboard.clone();
            self.text = clipboard.unwrap().trim().to_owned();
        }
        is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter)
    }
}

/// Rasterise every character the game draws, at every size it draws them, once.
///
/// macroquad grows its font atlas lazily: the first time a glyph is asked for
/// at a size, the atlas may be reallocated, and anything already batched
/// against the old texture then draws with a deleted one — which the browser
/// reports as `glBindTexture called with an already deleted texture ID`, an
/// error a frame that looks fine. It showed up on the pasted-code screen,
/// which is the one place the game puts three hundred characters of base64 on
/// screen at a size nothing else uses. Growing the atlas to its final size
/// before the first frame costs a few milliseconds at start-up and means the
/// console stays worth reading.
pub fn warm_the_font_atlas() {
    const PRINTABLE: &str =
        " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\
abcdefghijklmnopqrstuvwxyz{|}~\u{2026}\u{2318}\u{2014}";
    for size in [14, 15, 16, 17, 18, 19, 20, 22, 26, 28, 30, 32, 40, 56] {
        measure_text(PRINTABLE, None, size, 1.0);
    }
}

/// A heading, centred on the canvas.
pub fn centred(text: &str, y: f32, size: f32, colour: Color) {
    let m = measure_text(text, None, size as u16, 1.0);
    draw_text(text, (crate::screen::LOGICAL_W - m.width) / 2.0, y, size, colour);
}

/// Wrap prose on spaces. For sentences a person reads, unlike `wrapped`,
/// which chops a base64 blob wherever it likes because nobody reads one.
pub fn wrapped_words(text: &str, cols: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= cols => {
                line.push(' ');
                line.push_str(word);
            }
            _ => out.push(word.to_owned()),
        }
    }
    out
}

/// Wrap a long string into lines of at most `cols` characters, for the blobs.
pub fn wrapped(text: &str, cols: usize) -> Vec<String> {
    text.as_bytes()
        .chunks(cols)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect()
}

/// A string centred in a rectangle, for the little counters.
pub fn centred_in(r: Rect, text: &str, size: f32, colour: Color) {
    let m = measure_text(text, None, size as u16, 1.0);
    draw_text(
        text,
        r.x + (r.w - m.width) / 2.0,
        r.y + (r.h + m.offset_y) / 2.0 - 1.0,
        size,
        colour,
    );
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Nothing the game draws may leave ASCII.
    ///
    /// macroquad's built-in font has no em dash, no ellipsis and no curly
    /// quotes, and it does not fall back — it draws a hollow box. Three of
    /// them shipped in the lobby before a screenshot was looked at closely
    /// enough, including one in the middle of the sentence that tells a player
    /// what to do when the relays are down. The rule is only about strings the
    /// game puts on screen, so comments and doc comments are skipped; this is
    /// a lint with a heuristic in it, not a Rust parser, and it errs towards
    /// complaining.
    #[test]
    fn nothing_the_game_draws_is_outside_ascii() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut bad: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("gui has a src directory") {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || code.starts_with('*') {
                    continue;
                }
                for literal in string_literals(line) {
                    if !literal.is_ascii() {
                        bad.push(format!(
                            "{}:{}: {literal}",
                            path.file_name().unwrap().to_string_lossy(),
                            n + 1
                        ));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "macroquad's font draws a hollow box for these:\n  {}",
            bad.join("\n  ")
        );
    }

    /// Every `"…"` on a line, escapes respected and nothing else understood.
    fn string_literals(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '"' {
                continue;
            }
            let mut literal = String::new();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        chars.next();
                    }
                    '"' => break,
                    _ => literal.push(c),
                }
            }
            out.push(literal);
        }
        out
    }
}
