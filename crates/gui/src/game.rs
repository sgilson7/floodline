//! The game this window is playing.
//!
//! Natively that is a `net::Loopback` star in one process — design §7: "native
//! builds are for development: the GUI runs against `net::Loopback`". Single
//! player is the same lockstep with one peer, so there is no separate path
//! through the code for it and no way for one to rot while the other is used.

use net::loopback::{Conditions, Loopback, LoopbackPeer};
use net::{Lockstep, PeerId, Status};
use sim::{Command, PlayerId, World};

/// A local game: every player in this process, wired as a star.
pub struct Local {
    net: Loopback,
    peers: Vec<LoopbackPeer>,
    steps: Vec<Lockstep>,
    /// Which of them this window is looking through.
    pub me: usize,
}

impl Local {
    pub fn new(seed: u64, players: u32, build: &str) -> Local {
        let net = Loopback::new(players, Conditions::default());
        let peers: Vec<LoopbackPeer> = (0..players).map(|i| net.peer(PeerId(i))).collect();
        let mut steps = vec![Lockstep::host(seed, players, build)];
        for _ in 1..players {
            steps.push(Lockstep::join(build));
        }
        Local { net, peers, steps, me: 0 }
    }

    pub fn world(&self) -> &World {
        &self.steps[self.me].world
    }

    pub fn status(&self) -> &Status {
        &self.steps[self.me].status
    }

    pub fn me(&self) -> PlayerId {
        self.steps[self.me].me
    }

    pub fn in_lobby(&self) -> bool {
        self.steps[0].in_lobby()
    }

    pub fn start(&mut self) {
        self.steps[0].start();
    }

    /// Issue a command as the player this window belongs to. Phase 5's input
    /// is the only caller; it is here now so the plumbing is proven.
    #[allow(dead_code)]
    pub fn issue(&mut self, cmd: Command) -> Result<(), sim::world::RuleError> {
        let me = self.me;
        self.steps[me].issue(cmd)
    }

    /// One step of every peer, then the wire.
    pub fn advance(&mut self) {
        for (i, ls) in self.steps.iter_mut().enumerate() {
            ls.advance(&mut self.peers[i]);
        }
        self.net.step();
    }

    /// Every peer's tick, for the panel. They will not all be the same, and
    /// that is not a fault — see the lockstep tests.
    pub fn ticks(&self) -> Vec<u32> {
        self.steps.iter().map(|s| s.tick()).collect()
    }
}
