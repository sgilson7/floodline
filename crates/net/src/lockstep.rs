//! Everybody simulating the same world, one tick at a time.
//!
//! Design §8's star: a joiner sends its own `Turn` to the host and waits; the
//! host collects one `Turn` per live player, and when it has them all it sends
//! the `Bundle` for that tick to everyone. Every peer, the host included,
//! advances only on a bundle. Nobody simulates ahead and nobody rolls back.
//!
//! The host is a relay with a clock, not an authority. It runs the same `sim`
//! as everyone else and its own world has no privileges — which is what makes
//! a desync a disagreement to be reported rather than an argument to be won.

use crate::peer::{Event, Peer, PeerId};
use crate::wire::{decode, encode, Message, Refusal, PROTO_VERSION};
use sim::nav::Nav;
use sim::{Command, PlayerId, World};
use std::collections::{BTreeMap, BTreeSet};

/// Ticks between issuing a command and it taking effect (design §8): three, or
/// three hundred milliseconds. Long enough to cover a round trip on a home
/// connection, short enough that a click still feels like it did something.
pub const DELAY: u32 = 3;

/// A player this far behind is shown as "waiting on …" (design §8: five
/// seconds).
pub const WAIT_WARN_TICKS: u32 = 5 * sim::balance::TICKS_PER_SECOND;

/// And this far behind is dropped (thirty seconds).
pub const DROP_AFTER_TICKS: u32 = 30 * sim::balance::TICKS_PER_SECOND;

/// The most players design §6 allows.
pub const MAX_PLAYERS: usize = 6;

/// What the game is doing, for the panel to show.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Status {
    /// Connected, waiting for the host to start.
    Lobby,
    Playing,
    /// Everyone is held up on these players.
    WaitingOn(Vec<PlayerId>),
    /// Two peers disagree about the world. The game stops here, on every peer,
    /// at the same tick.
    Desync { with: PlayerId, tick: u32 },
    /// Over, for a reason worth showing.
    Ended(String),
}

impl Status {
    pub fn is_stopped(&self) -> bool {
        matches!(self, Status::Desync { .. } | Status::Ended(_))
    }
}

/// One peer's whole game.
pub struct Lockstep {
    pub world: World,
    pub nav: Nav,
    pub me: PlayerId,
    pub status: Status,
    build_hash: String,
    host: bool,

    /// The tick this peer will simulate next.
    next_tick: u32,
    /// The next tick this peer owes a `Turn` for.
    turn_tick: u32,
    /// Commands the player has issued since the last turn was sent.
    pending: Vec<Command>,

    /// Bundles received (or, on the host, made) and not yet applied.
    bundles: BTreeMap<u32, Vec<(PlayerId, Vec<Command>)>>,

    // ---- host only --------------------------------------------------------
    /// Turns collected from every player, by tick.
    collected: BTreeMap<u32, BTreeMap<PlayerId, Vec<Command>>>,
    /// The checksums each player reported for the tick before.
    reported: BTreeMap<u32, BTreeMap<PlayerId, u64>>,
    /// Who is on which connection.
    player_of: BTreeMap<PeerId, PlayerId>,
    peer_of: BTreeMap<PlayerId, PeerId>,
    /// How many of our own ticks each player has been holding us up for.
    waited: BTreeMap<PlayerId, u32>,
    /// The first tick each player owes a turn for.
    ///
    /// A joiner welcomed at tick N cannot have sent a turn for N — the host
    /// was already there. It is given until N + DELAY + 1 before the game
    /// starts waiting on it, which is exactly the pipeline depth its first
    /// turns will fill.
    active_from: BTreeMap<PlayerId, u32>,
    /// Players the host has decided to give up on but whose `Drop` has not
    /// been simulated yet. They stop being waited for the moment the decision
    /// is made, or the host would deadlock waiting for the very player it is
    /// trying to drop.
    dropping: BTreeSet<PlayerId>,
    next_player: u8,

    /// Joiner: a `Hello` is still owed to whichever peer turns up first.
    greet: bool,
}

impl Lockstep {
    /// Start a game as the host.
    pub fn host(seed: u64, players: u32, build_hash: &str) -> Lockstep {
        let world = World::new(seed, players);
        Lockstep {
            me: PlayerId(0),
            nav: Nav::new(),
            status: Status::Lobby,
            build_hash: build_hash.to_owned(),
            host: true,
            next_tick: 0,
            turn_tick: 0,
            pending: Vec::new(),
            bundles: BTreeMap::new(),
            collected: BTreeMap::new(),
            reported: BTreeMap::new(),
            player_of: BTreeMap::new(),
            peer_of: BTreeMap::new(),
            waited: BTreeMap::new(),
            dropping: BTreeSet::new(),
            active_from: BTreeMap::new(),
            next_player: 1,
            greet: false,
            world,
        }
    }

    /// Join a game. The world is a placeholder until `Welcome` arrives.
    ///
    /// Nothing is sent here, and that is the whole point. The first version
    /// said `Hello` to `peer.peers()` at this moment, which works on
    /// `Loopback` — where every peer exists before the first poll — and cannot
    /// work on a real one, where a browser has no peers at all until a
    /// connection completes seconds later. The `Hello` goes out on
    /// `Event::Peer` instead, which is true of both.
    pub fn join(build_hash: &str) -> Lockstep {
        let mut ls = Lockstep::host(0, 2, build_hash);
        ls.host = false;
        ls.me = PlayerId(u8::MAX); // not ours until the host says so
        ls.status = Status::Lobby;
        ls.greet = true;
        ls
    }

    /// Joiner: say `Hello` to the host now that there is a host to say it to.
    fn greet(&mut self, to: PeerId, peer: &mut impl Peer) {
        if !self.greet {
            return;
        }
        self.greet = false;
        let hello = encode(&Message::Hello {
            proto_version: PROTO_VERSION,
            build_hash: self.build_hash.clone(),
            name: String::new(),
        });
        peer.send(to, &hello, true);
    }

    /// Host: begin the run.
    ///
    /// Design §5 puts a *Start* button in the lobby, host only, once two to
    /// six players are present, and the button is not decoration. Without it
    /// the host simulates alone from the moment it is created and is fifty
    /// ticks ahead by the time anyone finishes connecting — so joiners arrive
    /// into a game already in progress, are handed a snapshot, and spend the
    /// rest of the run catching up on bundles they were never sent. Nothing
    /// happens until everybody is here.
    pub fn start(&mut self) {
        if self.host && self.status == Status::Lobby {
            self.status = Status::Playing;
        }
    }

    /// Whether this peer is the hub of the star — the one with the Start
    /// button and the seed.
    pub fn is_host(&self) -> bool {
        self.host
    }

    /// Whether the game is waiting in the lobby.
    pub fn in_lobby(&self) -> bool {
        self.status == Status::Lobby
    }

    /// Host: how many players have connected, including the host.
    pub fn connected(&self) -> usize {
        self.peer_of.len() + 1
    }

    /// Queue a command. It takes effect `DELAY` ticks from now.
    ///
    /// Checked locally first so the player is told *now* that a placement is
    /// illegal, rather than watching it be silently dropped three ticks later
    /// on five machines at once.
    pub fn issue(&mut self, cmd: Command) -> Result<(), sim::world::RuleError> {
        let mut trial = self.world.clone();
        trial.apply(self.me, &cmd)?;
        self.pending.push(cmd);
        Ok(())
    }

    /// The tick the world is on.
    pub fn tick(&self) -> u32 {
        self.world.tick
    }

    /// Everybody in the game, host first.
    pub fn players(&self) -> Vec<PlayerId> {
        self.world.players.clone()
    }

    /// One step: read the wire, send what is owed, and simulate if the next
    /// bundle has arrived.
    pub fn advance(&mut self, peer: &mut impl Peer) {
        self.drain(peer);
        if self.status.is_stopped() || self.status == Status::Lobby {
            return;
        }
        self.send_turns(peer);
        if self.host {
            self.bundle_up(peer);
        }
        self.apply_bundle();
    }

    // ---- reading the wire --------------------------------------------------

    fn drain(&mut self, peer: &mut impl Peer) {
        while let Some(ev) = peer.poll() {
            match ev {
                Event::Peer(who) => self.greet(who, peer),
                Event::Left(who) => self.peer_left(who, peer),
                Event::Error(text) => self.status = Status::Ended(text),
                Event::Msg { from, bytes, .. } => match decode(&bytes) {
                    Some(m) => self.handle(from, m, peer),
                    // Undecodable bytes are a different build talking. Saying
                    // so beats a silence nobody can debug.
                    None => {
                        self.status =
                            Status::Ended("a peer sent something this build cannot read".into())
                    }
                },
            }
        }
    }

    fn handle(&mut self, from: PeerId, m: Message, peer: &mut impl Peer) {
        match m {
            Message::Hello { proto_version, build_hash, .. } if self.host => {
                let refusal = if proto_version != PROTO_VERSION {
                    Some(Refusal::ProtoMismatch { theirs: proto_version, ours: PROTO_VERSION })
                } else if build_hash != self.build_hash {
                    Some(Refusal::BuildMismatch {
                        theirs: build_hash.clone(),
                        ours: self.build_hash.clone(),
                    })
                } else if self.next_player as usize >= self.world.players.len()
                    || self.player_of.len() + 1 >= MAX_PLAYERS
                {
                    Some(Refusal::GameFull)
                } else if self.world.age() > 1 || self.world.day_of_age() > 1 {
                    // MVP: joining is only allowed before the run gets going.
                    Some(Refusal::TooLate)
                } else {
                    None
                };

                if let Some(r) = refusal {
                    peer.send(from, &encode(&Message::Bye { reason: r.to_message() }), true);
                    return;
                }

                let player = PlayerId(self.next_player);
                self.next_player += 1;
                self.player_of.insert(from, player);
                self.peer_of.insert(player, from);
                self.active_from.insert(player, self.next_tick + DELAY + 1);

                // Always the whole world, never "here is the seed, build it
                // yourself". Design §8 offers the shortcut of sending no
                // snapshot before the run starts, and it is a trap: the map a
                // seed produces depends on how many players it was generated
                // for, so a joiner rebuilding from the seed while the host had
                // a different count builds a *different map* and every
                // checksum after that disagrees. Design §8 budgets 50–150 KB
                // for this message; a fresh six-player world is 60 KB, and it
                // is sent once per joiner.
                peer.send(
                    from,
                    &encode(&Message::Welcome {
                        player,
                        seed: self.world.seed,
                        tick: self.world.tick,
                        players: self.world.players.clone(),
                        snapshot: Some(Box::new(self.world.clone())),
                    }),
                    true,
                );
                let roster = encode(&Message::Roster { players: self.world.players.clone() });
                peer.broadcast(&roster, true);
            }

            Message::Welcome { player, seed, tick, players, snapshot } if !self.host => {
                self.me = player;
                self.world = match snapshot {
                    Some(w) => *w,
                    // Only reachable if a host is built that does not send one.
                    None => World::new(seed, players.len().max(2) as u32),
                };
                self.next_tick = self.world.tick.max(tick);
                self.turn_tick = self.next_tick;
                self.nav = Nav::new();
                // Still `Lobby`. Being welcomed is not the same as the game
                // having begun: the host has a Start button and until it is
                // pressed there is nothing to simulate, so a joiner that
                // called itself Playing here would sit in front of a world
                // that never moved and no longer had a lobby to explain why.
                // The first `Bundle` is what starts the run, and it arrives on
                // the same tick for everybody.
            }

            // The roster is who has actually connected. It does not change
            // `world.players`, which is fixed when the world is generated —
            // an empty seat is a city standing there with nobody commanding
            // it, not a hole in the world.
            Message::Roster { .. } => {}

            Message::Turn { player, tick, commands, checked_tick, checksum } if self.host => {
                self.collected.entry(tick).or_default().insert(player, commands);
                self.reported.entry(checked_tick).or_default().insert(player, checksum);
                self.waited.insert(player, 0);
                self.check_agreement(checked_tick);
            }

            Message::Bundle { tick, turns } if !self.host => {
                self.bundles.insert(tick, turns);
                if self.status == Status::Lobby {
                    self.status = Status::Playing;
                }
            }

            Message::Bye { reason } => self.status = Status::Ended(reason),
            _ => {}
        }
    }

    /// Everyone who reported a checksum for `tick` must have reported the same
    /// one. The host is the only peer that sees them all, which is why it is
    /// the one that notices.
    fn check_agreement(&mut self, tick: u32) {
        let Some(reports) = self.reported.get(&tick) else {
            return;
        };
        let mut seen: Option<(PlayerId, u64)> = None;
        for (&player, &sum) in reports {
            match seen {
                None => seen = Some((player, sum)),
                Some((first, first_sum)) if first_sum != sum => {
                    // Name the other one: the host is as likely to be wrong as
                    // anybody, but it is the only peer that can see the
                    // disagreement, so it is the one that has to report it.
                    let with = if player == self.me { first } else { player };
                    self.status = Status::Desync { with, tick };
                    return;
                }
                _ => {}
            }
        }
    }

    // ---- sending -----------------------------------------------------------

    fn send_turns(&mut self, peer: &mut impl Peer) {
        // Run `DELAY` ticks ahead of the simulation, so a bundle is ready when
        // the world needs one.
        while self.turn_tick <= self.next_tick + DELAY {
            let commands = std::mem::take(&mut self.pending);
            // The last tick this peer has actually simulated, and what it made
            // of it. Not the tick this turn is *for* — that is `DELAY` ticks
            // away and has not happened yet.
            let checked_tick = self.next_tick;
            let checksum = self.world.checksum();
            let turn = Message::Turn {
                player: self.me,
                tick: self.turn_tick,
                commands: commands.clone(),
                checked_tick,
                checksum,
            };
            if self.host {
                self.collected.entry(self.turn_tick).or_default().insert(self.me, commands);
                self.reported.entry(checked_tick).or_default().insert(self.me, checksum);
            } else {
                peer.broadcast(&encode(&turn), true);
            }
            self.turn_tick += 1;
        }
    }

    /// Host: if every live player has spoken for the next tick, say so.
    fn bundle_up(&mut self, peer: &mut impl Peer) {
        // Only the players who are actually here. A world is generated with a
        // fixed number of cities, and a seat nobody has taken is a city
        // standing there with nobody commanding it — waiting for a turn from
        // it would stop the game before it started, which is exactly what
        // hosting for four and connecting three did.
        let mut want: BTreeSet<PlayerId> = self.peer_of.keys().copied().collect();
        want.insert(self.me);
        want.retain(|p| {
            !self.world.dropped.contains(p)
                && !self.dropping.contains(p)
                && self.active_from.get(p).copied().unwrap_or(0) <= self.next_tick
        });

        let tick = self.next_tick;
        let have: BTreeSet<PlayerId> =
            self.collected.get(&tick).map(|m| m.keys().copied().collect()).unwrap_or_default();

        let missing: Vec<PlayerId> = want.difference(&have).copied().collect();
        if !missing.is_empty() {
            // Somebody is holding everyone up. Count how long, warn, and
            // eventually give up on them — as a command, so every peer drops
            // them on the same tick.
            let mut give_up: Vec<PlayerId> = Vec::new();
            for &p in &missing {
                let waited = self.waited.entry(p).or_insert(0);
                *waited += 1;
                if *waited >= DROP_AFTER_TICKS {
                    give_up.push(p);
                }
            }
            for p in give_up {
                self.give_up_on(p, tick);
            }
            let warned: Vec<PlayerId> = missing
                .iter()
                .copied()
                .filter(|p| self.waited.get(p).copied().unwrap_or(0) >= WAIT_WARN_TICKS)
                .collect();
            self.status = if warned.is_empty() {
                Status::Playing
            } else {
                Status::WaitingOn(warned)
            };
            return;
        }

        self.status = Status::Playing;
        let turns: Vec<(PlayerId, Vec<Command>)> = self
            .collected
            .get(&tick)
            .map(|m| m.iter().map(|(&p, c)| (p, c.clone())).collect())
            .unwrap_or_default();
        peer.broadcast(&encode(&Message::Bundle { tick, turns: turns.clone() }), true);
        self.bundles.insert(tick, turns);
        self.collected.remove(&tick);
    }

    /// Simulate a tick, if the bundle for it has arrived.
    fn apply_bundle(&mut self) {
        let Some(turns) = self.bundles.remove(&self.next_tick) else {
            return;
        };
        // Flattened in player order, which is the order `sim` will apply them
        // in on every peer. A `BTreeMap` got them here sorted; this keeps it.
        let mut commands: Vec<(PlayerId, Command)> = Vec::new();
        for (player, cmds) in turns {
            for c in cmds {
                commands.push((player, c));
            }
        }
        commands.sort_by_key(|(p, _)| p.0);
        self.world.tick(&mut self.nav, &commands);
        self.next_tick += 1;
        // Reports for ticks long past are no longer interesting, and keeping
        // them would grow without bound over a thirty-minute run.
        self.reported.remove(&self.next_tick.saturating_sub(DELAY + 2));
    }

    fn peer_left(&mut self, who: PeerId, _peer: &mut impl Peer) {
        if !self.host {
            // The star has one edge, and it was that one.
            self.status = Status::Ended("the host left the game".into());
            return;
        }
        if let Some(player) = self.player_of.remove(&who) {
            self.peer_of.remove(&player);
            let tick = self.next_tick;
            self.give_up_on(player, tick);
        }
    }

    /// Host: give up on a player, now.
    ///
    /// The `Drop` goes straight into the host's own turn for the tick being
    /// bundled rather than into `pending`, and the player stops being waited
    /// for immediately. Queuing it as an ordinary command deadlocks: `pending`
    /// is only flushed while sending turns, turns are only sent while the
    /// simulation is moving, and the simulation is stopped waiting for the
    /// very player the command exists to give up on.
    ///
    /// It is still a command, so every peer drops them on the same tick —
    /// which is the whole reason design §8 says to do it this way rather than
    /// letting each peer decide for itself.
    fn give_up_on(&mut self, player: PlayerId, tick: u32) {
        if self.dropping.contains(&player) || self.world.dropped.contains(&player) {
            return;
        }
        self.dropping.insert(player);
        self.waited.remove(&player);
        self.collected
            .entry(tick)
            .or_default()
            .entry(self.me)
            .or_default()
            .push(Command::Drop { player });
        self.reported.entry(tick).or_default().remove(&player);
    }
}
