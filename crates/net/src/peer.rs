//! The transport, as the rest of the game sees it.
//!
//! Four methods, and nothing in them knows whether the bytes are going through
//! a data channel in a browser or a queue in the same process. That is the
//! point of design §9.6: the lockstep, the desync banner, dropping a silent
//! player and late-joining from a snapshot are all tested against `Loopback`
//! in `cargo test`, so a networking regression and a lockstep regression can
//! never be mistaken for one another.

use serde::{Deserialize, Serialize};

/// A connection, numbered by the transport.
///
/// Deliberately not a `Uuid` and deliberately not a `PlayerId`. The transport
/// hands out these and knows nothing about the game; `sim::PlayerId` is the
/// game's own numbering and is assigned by the host in `Welcome`. Keeping them
/// separate is what lets a player keep their city across a reconnect that gives
/// them a new connection.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PeerId(pub u32);

/// Something that happened on the wire.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Event {
    /// Somebody connected.
    Peer(PeerId),
    /// Somebody went away — closed the tab, lost their connection, or was
    /// dropped. The lockstep cannot tell those apart and does not need to.
    Left(PeerId),
    Msg { from: PeerId, reliable: bool, bytes: Vec<u8> },
    /// The transport could not do what was asked. Carries something a person
    /// can read, because design §9.3 and phase 6 want failures that say what
    /// to do about them.
    ///
    /// `try_a_code` is whether a *different introduction* would get round it.
    /// Relays that never answered: yes, that is what design §9.1's pasted path
    /// is for. A connection that could not be opened once the two ends had
    /// already found each other: no — the pasted path needs the same direct
    /// link and fails in the same place. Saying "try Join by code" to that one
    /// sends a player round a loop, which is what it did.
    Error { text: String, try_a_code: bool },
}

/// A connection to the other players.
///
/// In the star (design §8) a joiner has exactly one peer — the host — and the
/// host has one per joiner. Nothing above this trait relies on that; the
/// lockstep addresses peers by id and lets the host do the relaying.
pub trait Peer {
    /// The next thing that happened, or `None`. Called once a frame until it
    /// gives `None`.
    fn poll(&mut self) -> Option<Event>;

    /// Send bytes to one peer. `reliable` picks the ordered channel; the
    /// unreliable one is for cursors and chat, which may be dropped.
    fn send(&mut self, to: PeerId, bytes: &[u8], reliable: bool);

    /// Everyone currently connected, in a fixed order.
    fn peers(&self) -> Vec<PeerId>;

    /// Whether this is the host — the hub of the star, which relays and owns
    /// the seed.
    fn is_host(&self) -> bool;

    /// Send to everybody. A default because every implementation would write
    /// the same loop.
    fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
        for p in self.peers() {
            self.send(p, bytes, reliable);
        }
    }
}
