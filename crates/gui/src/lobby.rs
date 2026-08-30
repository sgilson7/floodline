//! Getting two people into the same game.
//!
//! Design §9.4: a room is a short code the host shares, and
//! `?room=brisk-otter-42` opens the lobby with it filled in. Design §9.1: when
//! the relays are unreachable there is a second path that needs nobody's
//! relays, one pasted invitation and one pasted reply. Both end in the same
//! `Session`, so nothing past this file knows which was used.
//!
//! Everything here is drawn in logical pixels through `ui`, which reads the
//! mouse through `screen::Viewport` exactly once a frame. Nothing in this file
//! touches `mouse_position()`.

use crate::game::{Mode, Session};
use crate::ui::{self, Field, Ui};
use crate::{page, palette};
use macroquad::prelude::*;

/// Where the lobby is.
pub enum Lobby {
    /// Host or join, and by which path.
    Start { mode: Mode, seats: u32, room: Field, notice: String },
    /// Hosting, waiting for people. `Session` is live from this point.
    Hosting { mode: Mode, room: String, reply: Field, copied: f64 },
    /// Joining. In `Mode::Code` the field holds the host's invitation until it
    /// is applied, and after that the reply to send back.
    ///
    /// Only ever reached in the browser: a native build has no transport to
    /// join with, by design §7, so `open_join` refuses there instead.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Joining { mode: Mode, room: String, offer: Field, sent: bool, copied: f64 },
}

/// What the frame decided. `main` owns the session, so the lobby asks.
pub enum Act {
    Nothing,
    /// Start a session and move to the given screen.
    Open(Session, Lobby),
    /// The host pressed Start.
    Play,
    /// Back to the beginning, with a line saying why if there is one.
    Cancel(String),
}

impl Lobby {
    /// The first screen, with the room code from the URL already in it.
    pub fn new() -> Lobby {
        Lobby::Start {
            mode: Mode::Relay,
            seats: 2,
            room: Field::with(&page::url_room().unwrap_or_default()),
            notice: String::new(),
        }
    }

    /// Back to the first screen with a line saying what happened — and with
    /// the room code still in the box. Losing it meant retyping `brisk-otter-42`
    /// after every refusal, which is exactly when somebody is already annoyed.
    pub fn with_notice(notice: String, room: Option<String>) -> Lobby {
        match Lobby::new() {
            Lobby::Start { mode, seats, room: from_url, .. } => Lobby::Start {
                mode,
                seats,
                room: match room {
                    Some(r) if !r.is_empty() => Field::with(&r),
                    _ => from_url,
                },
                notice,
            },
            other => other,
        }
    }

    /// The room this screen is about, if it is about one.
    pub fn room(&self) -> Option<String> {
        match self {
            Lobby::Start { room, .. } => Some(room.text.clone()),
            Lobby::Hosting { room, .. } | Lobby::Joining { room, .. } => Some(room.clone()),
        }
    }

    pub fn draw(&mut self, ui: &Ui, session: Option<&mut Session>, build: &str) -> Act {
        card();
        match self {
            Lobby::Start { .. } => self.start_screen(ui, build),
            Lobby::Hosting { .. } => self.hosting_screen(ui, session, build),
            Lobby::Joining { .. } => self.joining_screen(ui, session, build),
        }
    }

    // ---- the first screen ---------------------------------------------------

    fn start_screen(&mut self, ui: &Ui, build: &str) -> Act {
        let Lobby::Start { mode, seats, room, notice } = self else {
            return Act::Nothing;
        };
        ui::centred("FLOODLINE", 210.0, 56.0, palette::INK);
        ui::centred(
            "a city on a river, and the water is coming",
            250.0,
            20.0,
            palette::FAINT,
        );

        let (cx, mut y) = (LOGICAL_W / 2.0, 330.0);

        // The path. Named for what it needs rather than for how it works: a
        // player choosing between these does not care what Nostr is.
        let relay = Rect::new(cx - 330.0, y, 320.0, 46.0);
        let code = Rect::new(cx + 10.0, y, 320.0, 46.0);
        if ui.button(relay, "by room code", true) {
            *mode = Mode::Relay;
        }
        if ui.button(code, "by pasted code", true) {
            *mode = Mode::Code;
        }
        let chosen = if *mode == Mode::Relay { relay } else { code };
        draw_rectangle_lines(chosen.x, chosen.y, chosen.w, chosen.h, 2.0, palette::INK);
        y += 54.0;
        ui::centred(
            match mode {
                Mode::Relay => "found through public relays - nothing of ours runs anywhere",
                Mode::Code => "one code pasted each way - needs nobody's relays at all",
            },
            y + 16.0,
            17.0,
            palette::FAINT,
        );
        y += 48.0;

        // Seats. The map is generated with this many cities and a seat nobody
        // takes is a city standing there with nobody commanding it, so it is
        // the one thing that cannot be decided later.
        draw_text("cities on the map", cx - 330.0, y + 26.0, 19.0, palette::INK);
        if ui.button(Rect::new(cx - 60.0, y, 44.0, 38.0), "-", *seats > 2) {
            *seats -= 1;
        }
        let n = seats.to_string();
        ui::centred_in(Rect::new(cx - 16.0, y, 44.0, 38.0), &n, 22.0, palette::INK);
        if ui.button(Rect::new(cx + 28.0, y, 44.0, 38.0), "+", (*seats as usize) < net::MAX_PLAYERS)
        {
            *seats += 1;
        }
        y += 60.0;

        if *mode == Mode::Relay {
            draw_text("room", cx - 330.0, y + 28.0, 19.0, palette::INK);
            let field = Rect::new(cx - 250.0, y, 340.0, 40.0);
            ui.field(field, &room.text, "leave empty and one is made for you", true);
            room.edit(ui, 40);
            y += 56.0;
        }

        let host = Rect::new(cx - 330.0, y, 320.0, 52.0);
        let join = Rect::new(cx + 10.0, y, 320.0, 52.0);
        let can_join = *mode == Mode::Code || !room.text.trim().is_empty();

        if !notice.is_empty() {
            ui::centred(notice, y + 96.0, 18.0, palette::ALARM);
        }
        ui::centred(
            &format!("build {build}"),
            LOGICAL_H - 40.0,
            15.0,
            palette::FAINT,
        );

        if ui.button(host, "Host a game", true) {
            let name = if room.text.trim().is_empty() {
                room_code()
            } else {
                room.text.trim().to_owned()
            };
            return open_host(name, *mode, *seats, build);
        }
        if ui.button(join, "Join a game", can_join) {
            return open_join(room.text.trim().to_owned(), *mode, build);
        }
        Act::Nothing
    }

    // ---- hosting ------------------------------------------------------------

    fn hosting_screen(&mut self, ui: &Ui, session: Option<&mut Session>, build: &str) -> Act {
        let Lobby::Hosting { mode, room, reply, copied } = self else {
            return Act::Nothing;
        };
        let Some(session) = session else {
            return Act::Nothing;
        };
        ui::centred("HOSTING", 190.0, 40.0, palette::INK);

        let (cx, mut y) = (LOGICAL_W / 2.0, 270.0);
        match mode {
            Mode::Relay => {
                ui::centred("give your friend this room code", y, 19.0, palette::FAINT);
                y += 54.0;
                ui::centred(room, y, 40.0, palette::WARNING);
                y += 40.0;
                let link = page::share_link(room);
                if !link.is_empty() {
                    ui::centred(&link, y, 16.0, palette::FAINT);
                }
                y += 34.0;
                let what = if link.is_empty() { room.clone() } else { link.clone() };
                let button = Rect::new(cx - 110.0, y, 220.0, 42.0);
                arm(ui, button, &what);
                if ui.button(button, "copy the link", true) {
                    page::copy(&what);
                    *copied = get_time();
                }
                y += 46.0;
                copied_note(*copied, y);
                y += 24.0;
            }
            Mode::Code => {
                y = blob_exchange(ui, session, reply, copied, y,
                                  "1. send them this invitation",
                                  "2. paste their reply here");
            }
        }

        let here = session.connected();
        ui::centred(
            &match here {
                1 => "nobody else is here yet".to_owned(),
                2 => "one other player is here".to_owned(),
                n => format!("{} other players are here", n - 1),
            },
            y,
            20.0,
            if here > 1 { palette::INK } else { palette::FAINT },
        );
        y += 40.0;

        if let net::Status::Ended(reason) = session.status() {
            return Act::Cancel(reason.clone());
        }
        if let Some(act) = trouble(ui, session, *mode, true, room, seats(session), build) {
            return act;
        }

        // Start is not gated on a second player. The plan says two to six, and
        // the design says single player is the lockstep with one peer — so
        // refusing to start alone would be refusing a mode the design has,
        // and would also strand anyone whose friend is slow to arrive.
        if ui.button(Rect::new(cx - 110.0, y, 220.0, 52.0), "Start", true) {
            return Act::Play;
        }
        if ui.button(Rect::new(cx - 110.0, y + 66.0, 220.0, 40.0), "back", true) {
            return Act::Cancel(String::new());
        }
        ui::centred(&format!("build {build}"), LOGICAL_H - 40.0, 15.0, palette::FAINT);
        Act::Nothing
    }

    // ---- joining ------------------------------------------------------------

    fn joining_screen(&mut self, ui: &Ui, session: Option<&mut Session>, build: &str) -> Act {
        let Lobby::Joining { mode, room, offer, sent, copied } = self else {
            return Act::Nothing;
        };
        let Some(session) = session else {
            return Act::Nothing;
        };
        ui::centred("JOINING", 190.0, 40.0, palette::INK);

        let (cx, mut y) = (LOGICAL_W / 2.0, 270.0);
        match mode {
            Mode::Relay => {
                ui::centred(&format!("room {room}"), y, 30.0, palette::WARNING);
                y += 50.0;
                // Three different things, which all used to read "looking for
                // the host on the public relays" — including the one where the
                // host had abandoned its lobby and was never going to answer.
                let (text, colour) = if session.welcomed() {
                    ("you are in. waiting for the host to start", palette::INK)
                } else if session.connected() > 0 && !session.roster_empty() {
                    ("found the host, asking for a city...", palette::INK)
                } else {
                    ("looking for the host on the public relays...", palette::FAINT)
                };
                ui::centred(text, y, 19.0, colour);
                y += 60.0;
            }
            Mode::Code => {
                if !*sent {
                    ui::centred("paste the invitation the host gave you", y, 19.0, palette::FAINT);
                    y += 30.0;
                    let field = Rect::new(cx - 420.0, y, 840.0, 44.0);
                    ui.field(field, &shortened(&offer.text), PASTE_HINT, true);
                    let entered = offer.edit(ui, 4000);
                    y += 62.0;
                    let ready = !offer.text.trim().is_empty();
                    if (ui.button(Rect::new(cx - 110.0, y, 220.0, 44.0), "use it", ready) || entered)
                        && ready
                    {
                        session.code_remote(offer.text.trim());
                        *sent = true;
                    }
                    y += 70.0;
                } else {
                    y = blob_exchange(ui, session, offer, copied, y,
                                      "send this reply back to the host", "");
                }
            }
        }

        if let net::Status::Ended(reason) = session.status() {
            return Act::Cancel(reason.clone());
        }
        if let Some(act) = trouble(ui, session, *mode, false, room, 2, build) {
            return act;
        }
        if session.welcomed() {
            let here = session.connected();
            ui::centred(
                &match here {
                    0 | 1 => "you and the host".to_owned(),
                    n => format!("{n} players here, you among them"),
                },
                y,
                20.0,
                palette::INK,
            );
        }
        y += 46.0;
        if ui.button(Rect::new(cx - 110.0, y, 220.0, 40.0), "back", true) {
            return Act::Cancel(String::new());
        }
        ui::centred(&format!("build {build}"), LOGICAL_H - 40.0, 15.0, palette::FAINT);
        Act::Nothing
    }
}

use crate::screen::{LOGICAL_H, LOGICAL_W};

/// How many cities this world was made with, so re-hosting keeps them.
fn seats(session: &Session) -> u32 {
    session.world().players.len() as u32
}

/// What the transport is unhappy about, and the way out of it.
///
/// Phase 6: "if Trystero reports no relay connection within 15 s, the lobby
/// offers *by code* automatically". The plugin does the fifteen seconds and
/// says so; this is the offer. It is a button rather than an automatic switch
/// because switching would abandon a room code the host has already sent
/// somebody, and the relays are often only slow.
fn trouble(
    ui: &Ui,
    session: &mut Session,
    mode: Mode,
    hosting: bool,
    room: &str,
    seats: u32,
    build: &str,
) -> Option<Act> {
    let said = session.trouble()?.clone();
    let lines = ui::wrapped_words(&said.text, 84);
    for (i, line) in lines.iter().enumerate() {
        ui::centred(line, LOGICAL_H - 238.0 + i as f32 * 22.0, 17.0, palette::ALARM);
    }
    // Only when a different introduction would actually help. The button used
    // to be offered for every complaint, including "we found each other and
    // could not connect" — for which the pasted path fails in exactly the same
    // place, because it needs the same direct link. That advice sent a player
    // round a loop.
    if mode == Mode::Relay && said.try_a_code {
        let r = Rect::new(LOGICAL_W / 2.0 - 190.0, LOGICAL_H - 168.0, 380.0, 40.0);
        // And it keeps what the player was doing. Offering a *joiner* a way to
        // become the host of a brand new empty room is not a way out of
        // anything.
        let label = if hosting { "host by pasted code instead" } else { "join by pasted code instead" };
        if ui.button(r, label, true) {
            return Some(if hosting {
                open_host(room.to_owned(), Mode::Code, seats, build)
            } else {
                open_join(String::new(), Mode::Code, build)
            });
        }
    }
    None
}

/// The blob to send and, if `ask` is non-empty, a box for the one coming back.
fn blob_exchange(
    ui: &Ui,
    session: &mut Session,
    reply: &mut Field,
    copied: &mut f64,
    mut y: f32,
    give: &str,
    ask: &str,
) -> f32 {
    let cx = LOGICAL_W / 2.0;
    ui::centred(give, y, 19.0, palette::FAINT);
    y += 26.0;

    let blob = session.code_local().map(|s| s.to_owned());
    let box_rect = Rect::new(cx - 420.0, y, 840.0, 76.0);
    draw_rectangle(box_rect.x, box_rect.y, box_rect.w, box_rect.h, palette::FIELD);
    draw_rectangle_lines(box_rect.x, box_rect.y, box_rect.w, box_rect.h, 1.0, palette::RULE);
    match &blob {
        Some(text) => {
            for (i, line) in ui::wrapped(text, 96).iter().take(4).enumerate() {
                draw_text(line, box_rect.x + 10.0, box_rect.y + 20.0 + i as f32 * 17.0, 14.0,
                          palette::INK);
            }
        }
        None => {
            draw_text(
                "gathering...",
                box_rect.x + 10.0,
                box_rect.y + 44.0,
                19.0,
                palette::FAINT,
            );
        }
    }
    y += 88.0;
    let button = Rect::new(cx - 110.0, y, 220.0, 40.0);
    if let Some(text) = &blob {
        arm(ui, button, text);
    }
    if ui.button(button, "copy", blob.is_some()) {
        if let Some(text) = &blob {
            page::copy(text);
            *copied = get_time();
        }
    }
    y += 44.0;
    copied_note(*copied, y);
    y += 22.0;

    if !ask.is_empty() {
        ui::centred(ask, y, 19.0, palette::FAINT);
        y += 26.0;
        let field = Rect::new(cx - 420.0, y, 840.0, 44.0);
        ui.field(field, &shortened(&reply.text), PASTE_HINT, true);
        let entered = reply.edit(ui, 4000);
        y += 60.0;
        let ready = !reply.text.trim().is_empty();
        if (ui.button(Rect::new(cx - 110.0, y, 220.0, 40.0), "use it", ready) || entered) && ready {
            session.code_remote(reply.text.trim());
            reply.text.clear();
        }
        y += 58.0;
    }
    y
}

/// Hand the page what this button would copy, and where it is.
///
/// Not on the click: a browser only lets a page write to the clipboard while a
/// user gesture is live, and macroquad reads a click in the animation frame
/// *after* the browser delivered it, by which time it is not. So the plugin
/// does the copying in the canvas's own click handler, and needs to know both
/// what to write and which part of the canvas means "copy". Hovering was tried
/// as the signal and is too slow: a click can arrive before a frame has been
/// drawn with the cursor over the button.
fn arm(ui: &Ui, button: Rect, text: &str) {
    page::arm_copy(Some(text), ui.to_page(button));
}

/// "copied", for a couple of seconds after the button was pressed.
///
/// The button used to do nothing visible whether it had worked or not, and for
/// a while it did nothing at all: `navigator.clipboard.writeText` hands back a
/// promise, a rejection is not something `try`/`catch` sees, and the fallback
/// never ran. On the one screen whose entire content is a string you have to
/// get to somebody else, that is the worst place in the game to be silent.
fn copied_note(at: f64, y: f32) {
    let age = get_time() - at;
    if at <= 0.0 || age > 2.5 {
        // The hint stands in for it, because the keyboard always works and
        // the button is at the mercy of the browser's clipboard permissions.
        ui::centred("or select nothing and press ctrl-C", y, 14.0, palette::FAINT);
        return;
    }
    ui::centred("copied", y, 15.0, palette::GOOD);
}

/// Said once rather than in two places that could disagree.
const PASTE_HINT: &str = "press ctrl-V here, or cmd-V on a Mac";

/// A blob is four hundred characters and the box is one line.
fn shortened(text: &str) -> String {
    if text.chars().count() <= 78 {
        text.to_owned()
    } else {
        let head: String = text.chars().take(70).collect();
        format!("{head}... ({} characters)", text.chars().count())
    }
}

fn open_host(room: String, mode: Mode, seats: u32, build: &str) -> Act {
    let seed = fresh_seed();
    #[cfg(target_arch = "wasm32")]
    let session = Session::Web(crate::game::Web::host(&room, mode, seed, seats, build));
    // Design §7: the native build is for development and plays against
    // `Loopback`. Hosting there fills every seat with a peer in this process,
    // which is the two-player game phase 3 was proved on.
    #[cfg(not(target_arch = "wasm32"))]
    let session = Session::local(seed, seats, build);
    Act::Open(session, Lobby::Hosting { mode, room, reply: Field::default(), copied: 0.0 })
}

fn open_join(room: String, mode: Mode, build: &str) -> Act {
    #[cfg(target_arch = "wasm32")]
    {
        let session = Session::Web(crate::game::Web::join(&room, mode, build));
        Act::Open(
            session,
            Lobby::Joining { mode, room, offer: Field::default(), sent: false, copied: 0.0 },
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (room, mode, build);
        Act::Cancel("there is nothing to join from a native build - it has no transport, by design. Host instead.".to_owned())
    }
}

/// The panel the lobby sits on.
fn card() {
    let r = Rect::new(LOGICAL_W / 2.0 - 520.0, 120.0, 1040.0, LOGICAL_H - 240.0);
    draw_rectangle(r.x, r.y, r.w, r.h, palette::PANEL);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, palette::RULE);
}

/// `brisk-otter-42` (design §9.4): two words and a number, short enough to
/// read down a phone and long enough not to collide on a public relay.
fn room_code() -> String {
    const ADJECTIVES: [&str; 16] = [
        "brisk", "slate", "amber", "quiet", "bitter", "salt", "low", "grey", "keen", "hollow",
        "north", "wide", "old", "green", "cold", "hard",
    ];
    const CREATURES: [&str; 16] = [
        "otter", "heron", "pike", "vole", "tern", "eel", "crow", "hare", "marten", "gull",
        "roach", "wren", "adder", "coot", "stoat", "kite",
    ];
    let a = ADJECTIVES[macroquad::rand::gen_range(0, ADJECTIVES.len())];
    let c = CREATURES[macroquad::rand::gen_range(0, CREATURES.len())];
    format!("{a}-{c}-{}", macroquad::rand::gen_range(10, 100))
}

/// A different map every run. `sim` may not read a clock and does not: this is
/// the one place a seed is chosen, and it is handed in from outside.
fn fresh_seed() -> u64 {
    macroquad::rand::gen_range(1u64, u64::MAX)
}
