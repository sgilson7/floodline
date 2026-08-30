//! `Peer` over `web/quad_rtc.js`.
//!
//! Thin on purpose. Everything that could be wrong about a handshake is in the
//! plugin, where it can be driven from `web/echo.html` with no wasm in the
//! way; everything that could be wrong about a lockstep is in `net`, where it
//! is driven by `Loopback` with no browser in the way. This file is the seam,
//! and the less it decides the better.

use crate::roster::Roster;
use net::{Event, Peer, PeerId};
use sapp_jsutils::JsObject;

/// How two peers are introduced. The connection they end up with is the same
/// either way, which is why `Peer` above this cannot tell them apart.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Public relays do the introduction (design 9.1). Needs a room code.
    Relay = 0,
    /// One pasted invitation and one pasted reply. Needs nobody's relays.
    Code = 1,
}

extern "C" {
    fn rtc_host(room: JsObject, mode: u32) -> u32;
    fn rtc_join(room: JsObject, mode: u32) -> u32;
    fn rtc_close(session: u32);
    fn rtc_poll() -> JsObject;
    fn rtc_send(peer: u32, reliable: u32, bytes: JsObject);
    fn rtc_code_local() -> JsObject;
    fn rtc_code_remote(blob: JsObject) -> u32;
}

/// One browser's connection to the others.
pub struct WebPeer {
    roster: Roster,
    host: bool,
    /// Which session in the plugin this is.
    ///
    /// Carried so that dropping a finished `WebPeer` cannot close the one that
    /// replaced it. Rust builds the new session before it drops the old, so
    /// without this, hosting a second game opened a room and then tore it down
    /// half a frame later.
    session: u32,
}

impl WebPeer {
    /// Be the hub of the star. `room` is ignored in `Mode::Code`, where the
    /// invitation is the room.
    pub fn host(room: &str, mode: Mode) -> WebPeer {
        let session = unsafe { rtc_host(JsObject::string(room), mode as u32) };
        WebPeer { roster: Roster::new(), host: true, session }
    }

    pub fn join(room: &str, mode: Mode) -> WebPeer {
        let session = unsafe { rtc_join(JsObject::string(room), mode as u32) };
        WebPeer { roster: Roster::new(), host: false, session }
    }

    /// `Mode::Code`: the blob for this player to send to the other one — an
    /// invitation if hosting, a reply if joining — or `None` while ICE is
    /// still gathering. Polled from the lobby; there is no callback to wait
    /// on because there is no callback into wasm.
    pub fn code_local(&self) -> Option<String> {
        let obj = unsafe { rtc_code_local() };
        if obj.is_nil() || obj.is_undefined() {
            // `JsObject`'s `Drop` frees the id, and -1 and -2 are the shared
            // sentinels for null and undefined rather than allocations of
            // ours. Freeing one deletes it from the plugin's table for
            // everybody.
            std::mem::forget(obj);
            return None;
        }
        let mut out = String::new();
        obj.to_string(&mut out);
        Some(out)
    }

    /// `Mode::Code`: what the other player pasted back. Whether it made sense
    /// arrives later as an `Event::Error`, because the work is asynchronous —
    /// ICE has to gather before there is anything to say.
    pub fn code_remote(&mut self, blob: &str) -> bool {
        unsafe { rtc_code_remote(JsObject::string(blob)) == 1 }
    }

    pub fn close(&mut self) {
        unsafe { rtc_close(self.session) };
    }
}

/// Leaving a game leaves the room.
///
/// Without this the plugin stayed in the Trystero room after the game above it
/// was gone: pressing *back* in the lobby dropped the Rust session and nothing
/// told the browser, so the tab went on answering strangers at the transport
/// level for as long as it was open. Joiners connected to it, said `Hello`,
/// and waited on a game that did not exist — while their screen said "looking
/// for the host", the same thing it says when there is no room at all. It cost
/// an evening of play and it is four lines.
impl Drop for WebPeer {
    fn drop(&mut self) {
        self.close();
    }
}

impl Peer for WebPeer {
    fn poll(&mut self) -> Option<Event> {
        let ev = unsafe { rtc_poll() };
        if ev.is_nil() || ev.is_undefined() {
            std::mem::forget(ev);
            return None;
        }
        // The plugin's four event kinds, in the order DECISIONS.md lists them.
        match ev.field_u32("k") {
            0 => {
                let id = PeerId(ev.field_u32("id"));
                self.roster.joined(id);
                Some(Event::Peer(id))
            }
            1 => {
                let id = PeerId(ev.field_u32("id"));
                self.roster.left(id);
                Some(Event::Left(id))
            }
            2 => {
                let mut bytes = Vec::new();
                ev.field("bytes").to_byte_buffer(&mut bytes);
                Some(Event::Msg {
                    from: PeerId(ev.field_u32("id")),
                    reliable: ev.field_u32("reliable") != 0,
                    bytes,
                })
            }
            _ => {
                let mut text = String::new();
                ev.field("text").to_string(&mut text);
                Some(Event::Error { text, try_a_code: ev.field_u32("code") != 0 })
            }
        }
    }

    fn send(&mut self, to: PeerId, bytes: &[u8], reliable: bool) {
        unsafe { rtc_send(to.0, reliable as u32, JsObject::buffer(bytes)) };
    }

    fn peers(&self) -> Vec<PeerId> {
        self.roster.ids().to_vec()
    }

    fn is_host(&self) -> bool {
        self.host
    }
}
