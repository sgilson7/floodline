//! Who this peer can see, in the order they arrived.
//!
//! Small enough to look pointless and worth having anyway: it is the only part
//! of `net-web` that can be tested without a browser, and `Peer::peers()` is
//! `&self` while `Peer::poll()` is `&mut self`, so the list has to be kept as
//! events are drained rather than asked for on demand.

use net::PeerId;

#[derive(Default)]
pub struct Roster {
    ids: Vec<PeerId>,
}

impl Roster {
    pub fn new() -> Roster {
        Roster { ids: Vec::new() }
    }

    /// Idempotent. The plugin reports a peer once, but a transport that ever
    /// repeated itself would otherwise give the host two seats for one player.
    pub fn joined(&mut self, id: PeerId) {
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
    }

    pub fn left(&mut self, id: PeerId) {
        self.ids.retain(|&p| p != id);
    }

    /// Arrival order, which is a fixed order — the trait asks for one so that
    /// `broadcast` sends to everybody in the same sequence on every peer.
    pub fn ids(&self) -> &[PeerId] {
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrivals_are_kept_in_order_and_only_once() {
        let mut r = Roster::new();
        r.joined(PeerId(3));
        r.joined(PeerId(1));
        r.joined(PeerId(3));
        assert_eq!(r.ids(), &[PeerId(3), PeerId(1)]);
    }

    #[test]
    fn leaving_removes_exactly_one_and_keeps_the_rest_in_order() {
        let mut r = Roster::new();
        for id in [1, 2, 3] {
            r.joined(PeerId(id));
        }
        r.left(PeerId(2));
        assert_eq!(r.ids(), &[PeerId(1), PeerId(3)]);
        // A departure the transport reports twice is not an error.
        r.left(PeerId(2));
        assert_eq!(r.ids(), &[PeerId(1), PeerId(3)]);
    }

    #[test]
    fn a_joiner_sees_exactly_one_peer() {
        // The star, as the plugin enforces it: a joiner is only ever told
        // about the host, so the roster it builds has one entry in it.
        let mut r = Roster::new();
        r.joined(PeerId(1));
        assert_eq!(r.ids().len(), 1);
    }
}
