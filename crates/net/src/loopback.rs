//! N players in one process, wired as a star.
//!
//! Design §9.6: everything above the `Peer` trait is tested here, with no
//! browser and no network. It is a real transport, not a stub — it has one-way
//! latency, it drops unreliable messages, and it does both deterministically
//! from its own seeded generator, so a test that fails once fails every time.
//!
//! The star is enforced rather than assumed: a joiner sending to another
//! joiner is a bug in the lockstep, and here it is an error rather than a
//! message that quietly arrives.

use crate::peer::{Event, Peer, PeerId};
use sim::Rng;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

/// The host is always peer zero.
pub const HOST: PeerId = PeerId(0);

/// How the wire behaves.
#[derive(Copy, Clone, Debug)]
pub struct Conditions {
    /// One-way delay, in ticks of the caller's own clock.
    pub latency: u32,
    /// Chance in a hundred that an unreliable message is thrown away.
    pub loss_percent: u32,
    /// Whether every peer can see every other one, as in a real room.
    ///
    /// **Off by default, and that default hid a bug for the whole of the
    /// project.** This transport gives a joiner exactly one peer — the host —
    /// which *enforces* the star rather than testing that the lockstep keeps
    /// it. `WebPeer::peers()` returns the whole roster, because a Trystero
    /// room is a mesh: everybody is connected to everybody. So
    /// `peer.broadcast` from a joiner meant "to the host" in every test here
    /// and "to every other player, every tick" in a browser, and `send_turns`
    /// did exactly that until M12 went looking.
    ///
    /// With this on, a joiner is handed every peer and `broken_star` records
    /// anything it sends where the design says it may not.
    /// `the_star_holds_even_when_the_wire_does_not` is the guard.
    pub mesh: bool,
}

impl Default for Conditions {
    fn default() -> Self {
        Conditions { latency: 0, loss_percent: 0, mesh: false }
    }
}

impl Conditions {
    /// A room rather than a star: everybody sees everybody, which is what the
    /// browser actually gives us. See `mesh`.
    pub fn room() -> Self {
        Conditions { mesh: true, ..Conditions::realistic() }
    }

    /// About what two people on home connections see.
    pub fn realistic() -> Self {
        Conditions { latency: 2, loss_percent: 2, mesh: false }
    }
}

struct Wire {
    /// Messages waiting to be delivered, keyed by the step they arrive on.
    queued: BTreeMap<PeerId, VecDeque<(u32, Event)>>,
    /// Delivered and waiting to be polled.
    inbox: BTreeMap<PeerId, VecDeque<Event>>,
    connected: Vec<PeerId>,
    conditions: Conditions,
    rng: Rng,
    step: u32,
    /// Set when somebody sends where the star says they may not, with what
    /// they sent — because one message is allowed to go that way and the rest
    /// are not, and only the caller can tell them apart. See `broken_star`.
    pub broken_star: Vec<(PeerId, PeerId, Vec<u8>)>,
}

/// A shared in-process network.
#[derive(Clone)]
pub struct Loopback {
    wire: Rc<RefCell<Wire>>,
}

impl Loopback {
    /// A network of `players` peers: peer 0 hosts, the rest join.
    pub fn new(players: u32, conditions: Conditions) -> Loopback {
        let ids: Vec<PeerId> = (0..players).map(PeerId).collect();
        let wire = Wire {
            queued: ids.iter().map(|&p| (p, VecDeque::new())).collect(),
            inbox: ids.iter().map(|&p| (p, VecDeque::new())).collect(),
            connected: ids.clone(),
            conditions,
            rng: Rng::new(0xB0A7),
            step: 0,
            broken_star: Vec::new(),
        };
        let net = Loopback { wire: Rc::new(RefCell::new(wire)) };

        // Everybody learns about the peers the star gives them: the host sees
        // every joiner, and each joiner sees only the host.
        for &id in &ids {
            for &other in &ids {
                if id == other {
                    continue;
                }
                if conditions.mesh || id == HOST || other == HOST {
                    net.wire.borrow_mut().inbox.get_mut(&id).unwrap().push_back(Event::Peer(other));
                }
            }
        }
        net
    }

    /// A handle for one of the peers.
    pub fn peer(&self, id: PeerId) -> LoopbackPeer {
        LoopbackPeer { id, net: self.clone() }
    }

    /// Advance the wire's clock by one step, delivering whatever is due.
    ///
    /// Called once per tick by whoever is driving the test. Latency is
    /// measured in these steps, which makes "two hundred milliseconds" mean
    /// two ticks and keeps the whole thing free of real time — `sim` may not
    /// look at a clock and neither may its tests.
    pub fn step(&self) {
        let mut w = self.wire.borrow_mut();
        w.step += 1;
        let now = w.step;
        let ids: Vec<PeerId> = w.queued.keys().copied().collect();
        for id in ids {
            loop {
                let due = matches!(w.queued[&id].front(), Some(&(at, _)) if at <= now);
                if !due {
                    break;
                }
                let (_, ev) = w.queued.get_mut(&id).unwrap().pop_front().unwrap();
                w.inbox.get_mut(&id).unwrap().push_back(ev);
            }
        }
    }

    /// Take a peer off the network, as a closed tab would.
    pub fn disconnect(&self, id: PeerId) {
        let mut w = self.wire.borrow_mut();
        w.connected.retain(|&p| p != id);
        let others: Vec<PeerId> = w.connected.clone();
        for other in others {
            // Only peers that could see them are told.
            if other == HOST || id == HOST {
                w.inbox.get_mut(&other).unwrap().push_back(Event::Left(id));
            }
        }
        w.queued.get_mut(&id).map(|q| q.clear());
        w.inbox.get_mut(&id).map(|q| q.clear());
    }

    /// Whether anybody sent where the star forbids it.
    /// Everything sent from a joiner to a joiner, with the bytes.
    ///
    /// **A `Hello` belongs in here and is not a fault.** Discovery in a room is
    /// necessarily mesh-shaped: a joiner cannot tell a host from a bystander
    /// until one of them answers, so it greets everybody it meets (M12.2). It
    /// is the *game* traffic — turns, bundles, rosters — that must go through
    /// the hub, and a caller that wants that question asks it by decoding.
    pub fn broken_star(&self) -> Vec<(PeerId, PeerId, Vec<u8>)> {
        self.wire.borrow().broken_star.clone()
    }
}

/// One participant's view of the network.
pub struct LoopbackPeer {
    id: PeerId,
    net: Loopback,
}

impl LoopbackPeer {
    pub fn id(&self) -> PeerId {
        self.id
    }
}

impl Peer for LoopbackPeer {
    fn poll(&mut self) -> Option<Event> {
        self.net.wire.borrow_mut().inbox.get_mut(&self.id)?.pop_front()
    }

    fn send(&mut self, to: PeerId, bytes: &[u8], reliable: bool) {
        let mut w = self.net.wire.borrow_mut();
        if !w.connected.contains(&to) || !w.connected.contains(&self.id) {
            return;
        }
        // The star: everything goes through the host.
        if self.id != HOST && to != HOST {
            w.broken_star.push((self.id, to, bytes.to_vec()));
            return;
        }
        if !reliable {
            let roll = w.rng.below(100);
            if roll < w.conditions.loss_percent {
                return;
            }
        }
        let at = w.step + w.conditions.latency;
        let ev = Event::Msg { from: self.id, reliable, bytes: bytes.to_vec() };
        w.queued.get_mut(&to).unwrap().push_back((at, ev));
    }

    fn peers(&self) -> Vec<PeerId> {
        let w = self.net.wire.borrow();
        // In `mesh` this is every peer, as a browser gives it. Otherwise the
        // star: the host sees everybody and a joiner sees the host. See
        // `Conditions::mesh` for why the difference matters.
        w.connected
            .iter()
            .copied()
            .filter(|&p| p != self.id && (w.conditions.mesh || self.id == HOST || p == HOST))
            .collect()
    }

    fn is_host(&self) -> bool {
        self.id == HOST
    }
}
