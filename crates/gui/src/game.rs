//! The game this window is playing, whatever is carrying it.
//!
//! Two of them. Natively a `net::Loopback` star in one process — design §7:
//! "native builds are for development: the GUI runs against `net::Loopback`" —
//! which is what makes `make` useful with no browser and no other player. In
//! the browser, one `net_web::WebPeer` and one `Lockstep`, which is the same
//! lockstep the loopback runs and the same one `cargo test -p net` proves.
//!
//! Single player is the lockstep with one peer, so there is no third path.

#[cfg(not(target_arch = "wasm32"))]
use net::loopback::{Conditions, Loopback, LoopbackPeer};
#[cfg(not(target_arch = "wasm32"))]
use net::PeerId;
use net::{Lockstep, Status};
use sim::{Command, PlayerId, World};

/// How two browsers are introduced. Mirrors `net_web::Mode` so the lobby can
/// name it on a build that has no `net-web` in it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    Relay,
    Code,
}

/// Every player in this process, wired as a star.
///
/// Native only. Design §7: "native builds are for development: the GUI runs
/// against `net::Loopback`". A browser has a real transport and no reason to
/// pretend, and single player there is the same lockstep hosting one seat.
#[cfg(not(target_arch = "wasm32"))]
pub struct Local {
    net: Loopback,
    peers: Vec<LoopbackPeer>,
    steps: Vec<Lockstep>,
    /// Which of them this window is looking through.
    pub me: usize,
}

#[cfg(not(target_arch = "wasm32"))]
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

    fn advance(&mut self) {
        for (i, ls) in self.steps.iter_mut().enumerate() {
            ls.advance(&mut self.peers[i]);
        }
        self.net.step();
    }
}

#[cfg(target_arch = "wasm32")]
pub struct Web {
    peer: net_web::WebPeer,
    step: Lockstep,
    /// The blob the plugin last offered. Cached because the lobby draws it
    /// every frame and it crosses the JS boundary as a string.
    blob: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl Web {
    fn mode(mode: Mode) -> net_web::Mode {
        match mode {
            Mode::Relay => net_web::Mode::Relay,
            Mode::Code => net_web::Mode::Code,
        }
    }

    pub fn host(room: &str, mode: Mode, seed: u64, seats: u32, build: &str) -> Web {
        Web {
            peer: net_web::WebPeer::host(room, Web::mode(mode)),
            step: Lockstep::host(seed, seats, build),
            blob: None,
        }
    }

    pub fn join(room: &str, mode: Mode, build: &str) -> Web {
        Web {
            peer: net_web::WebPeer::join(room, Web::mode(mode)),
            step: Lockstep::join(build),
            blob: None,
        }
    }
}

pub enum Session {
    #[cfg(not(target_arch = "wasm32"))]
    Local(Local),
    #[cfg(target_arch = "wasm32")]
    Web(Web),
}

impl Session {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn local(seed: u64, players: u32, build: &str) -> Session {
        Session::Local(Local::new(seed, players, build))
    }

    /// The lockstep this window is looking through.
    fn step(&self) -> &Lockstep {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => &l.steps[l.me],
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => &w.step,
        }
    }

    #[allow(dead_code)]
    fn step_mut(&mut self) -> &mut Lockstep {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => {
                let me = l.me;
                &mut l.steps[me]
            }
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => &mut w.step,
        }
    }

    pub fn world(&self) -> &World {
        &self.step().world
    }

    pub fn status(&self) -> &Status {
        &self.step().status
    }

    pub fn me(&self) -> PlayerId {
        self.step().me
    }

    /// What the transport has complained about while still in the lobby.
    /// Advice, not a verdict — see `Lockstep::trouble`.
    pub fn trouble(&self) -> Option<&str> {
        self.step().trouble.as_deref()
    }

    /// Whether this peer is still waiting for the host to press Start.
    pub fn in_lobby(&self) -> bool {
        self.step().in_lobby()
    }

    /// Only the host may Start, and only the host is shown a room code.
    #[allow(dead_code)]
    pub fn is_host(&self) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(_) => true,
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => w.step.is_host(),
        }
    }

    /// How many players have connected, host included.
    pub fn connected(&self) -> usize {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => l.steps.len(),
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => w.step.connected(),
        }
    }

    /// Host: begin the run. On the loopback every peer is its own lockstep, so
    /// only the host's Start matters and the rest follow the first bundle.
    pub fn start(&mut self) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => l.steps[0].start(),
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => w.step.start(),
        }
    }

    /// Phase 5's input is the only caller.
    #[allow(dead_code)]
    pub fn issue(&mut self, cmd: Command) -> Result<(), sim::world::RuleError> {
        self.step_mut().issue(cmd)
    }

    pub fn advance(&mut self) {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => l.advance(),
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => {
                w.step.advance(&mut w.peer);
                // Mirrored, not accumulated: the host clears its invitation
                // the moment a reply is applied and gathers a fresh one, and
                // showing the old blob in between would be showing a code that
                // no longer connects to anything.
                w.blob = w.peer.code_local();
            }
        }
    }

    /// Every peer's tick, for the panel. They will not all be the same, and
    /// that is not a fault — see the lockstep tests.
    pub fn ticks(&self) -> Vec<u32> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(l) => l.steps.iter().map(|s| s.tick()).collect(),
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => vec![w.step.tick()],
        }
    }

    /// `Mode::Code`: the blob this player has to send the other one, once ICE
    /// has finished gathering.
    pub fn code_local(&self) -> Option<&str> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(_) => None,
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => w.blob.as_deref(),
        }
    }

    /// `Mode::Code`: what the other player pasted back.
    pub fn code_remote(&mut self, blob: &str) {
        let _ = blob;
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Session::Local(_) => {}
            #[cfg(target_arch = "wasm32")]
            Session::Web(w) => {
                w.blob = None;
                w.peer.code_remote(blob);
            }
        }
    }
}
