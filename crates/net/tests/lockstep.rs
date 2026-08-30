//! Phase 3's checklist, all of it in `cargo test` with no browser.
//!
//! Design §9.6 is the reason this file exists: the lockstep, the desync
//! banner, dropping a silent player and late-joining from a snapshot are the
//! four things most likely to be wrong, and all four are testable against
//! `Loopback` in-process. Under the v1 plan they would have been debugged
//! through a WebRTC connection, where a desync and a dropped data channel look
//! identical.

use net::lockstep::{DROP_AFTER_TICKS, WAIT_WARN_TICKS};
use net::loopback::{Conditions, Loopback, LoopbackPeer, HOST};
use net::{Lockstep, PeerId, Status};
use sim::building::Kind;
use sim::{Command, PlayerId};
use std::collections::BTreeMap;

const BUILD: &str = "test-build";

/// How often the test records a peer's checksum for its own comparison.
const HISTORY_EVERY: u32 = 20;

/// A game of `n` players on a loopback star, already past the handshake.
struct Game {
    net: Loopback,
    peers: Vec<LoopbackPeer>,
    steps: Vec<Lockstep>,
    /// What each peer's world checksummed to at each tick it simulated.
    ///
    /// Compared per *tick*, never per moment. With any latency at all the host
    /// is a couple of ticks ahead of everyone else at any given instant, so
    /// "do all the peers agree right now" is a question with no useful answer;
    /// "did they all get the same world at tick 400" is the property lockstep
    /// actually promises.
    history: Vec<BTreeMap<u32, u64>>,
}

impl Game {
    fn new(n: u32, conditions: Conditions) -> Game {
        let net = Loopback::new(n, conditions);
        let mut peers: Vec<LoopbackPeer> = (0..n).map(|i| net.peer(PeerId(i))).collect();
        let mut steps = vec![Lockstep::host(31, n, BUILD)];
        for p in peers.iter_mut().skip(1) {
            steps.push(Lockstep::join(BUILD, p));
        }
        let history = vec![BTreeMap::new(); n as usize];
        let mut g = Game { net, peers, steps, history };
        // Let everybody connect, then start — the host does not simulate a
        // tick before that, so every peer begins the run together at zero.
        g.run(20);
        assert_eq!(g.steps[0].connected(), n as usize, "not everybody connected");
        g.steps[0].start();
        g.run(5);
        g
    }

    fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            for (i, ls) in self.steps.iter_mut().enumerate() {
                ls.advance(&mut self.peers[i]);
                // Sampled, not every tick. A checksum serialises the whole
                // world, and recording one per peer per tick over two ages
                // costs more than the game does. The lockstep compares them
                // every tick anyway — that is the mechanism under test; this
                // is only the test's own second opinion.
                if ls.tick() % HISTORY_EVERY == 0 {
                    self.history[i].insert(ls.tick(), ls.world.checksum());
                }
            }
            self.net.step();
        }
    }

    /// Every tick that more than one peer reached, and whether they agreed.
    fn disagreements(&self) -> Vec<(u32, Vec<u64>)> {
        let mut out = Vec::new();
        for (&tick, &first) in &self.history[0] {
            let mut sums = vec![first];
            for h in &self.history[1..] {
                if let Some(&s) = h.get(&tick) {
                    sums.push(s);
                }
            }
            if sums.len() > 1 && sums.iter().any(|&s| s != sums[0]) {
                out.push((tick, sums));
            }
        }
        out
    }

    /// Run until the slowest peer reaches `tick`, or give up.
    ///
    /// Tick-driven rather than counting calls to `advance`, because the two
    /// are not the same thing: lockstep advances only when every peer's turn
    /// has arrived, so a wire with two ticks of one-way latency — a four-tick
    /// round trip against a three-tick input delay — runs the world at about
    /// four fifths of the rate it is polled at. That is the design working,
    /// not a fault, and a test that counted polls would be measuring the wire.
    fn run_to_tick(&mut self, tick: u32) {
        let mut spent = 0;
        while self.steps.iter().any(|s| s.tick() < tick) {
            self.run(1);
            spent += 1;
            assert!(
                spent < tick * 10 + 500,
                "the game stalled at ticks {:?}, status {:?}",
                self.ticks(),
                self.steps[0].status
            );
            if self.steps.iter().any(|s| s.status.is_stopped()) {
                return;
            }
        }
    }

    fn ticks(&self) -> Vec<u32> {
        self.steps.iter().map(|s| s.tick()).collect()
    }



    /// Found a city for player `who`, entirely through commands.
    ///
    /// Not a convenience: without it the three-player test starved. Nobody
    /// builds a farm, food runs out after a thousand ticks, everybody is dead
    /// three days later and the world stops — at tick 4599, which is exactly
    /// right and nothing whatever to do with lockstep. A test that means to
    /// run to age two has to play the game.
    fn found(&mut self, who: usize) {
        let player = self.steps[who].me;
        let (hx, hy) = self.steps[who].world.map.hearth_sites[player.0 as usize];
        // Spots are remembered as they are chosen. `issue` checks a command
        // against the world *now*, and the world does not yet know about the
        // placements still sitting in this turn — so without this the farm and
        // the cottage both pick the same empty square and the second is thrown
        // out three ticks later when it is applied.
        let mut taken: Vec<(i32, i32)> = Vec::new();
        for kind in [Kind::Farm, Kind::Granary, Kind::Cottage] {
            'place: for r in 3..30i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        let (x, y) = (hx + dx, hy + dy);
                        if taken.iter().any(|&(tx, ty)| (tx - x).abs() < 4 && (ty - y).abs() < 4) {
                            continue;
                        }
                        let cmd = Command::Place { kind, x: x as u8, y: y as u8 };
                        if self.steps[who].issue(cmd).is_ok() {
                            taken.push((x, y));
                            break 'place;
                        }
                    }
                }
            }
        }
    }

    /// Keep this player's city alive: farmers on the farm once it stands,
    /// builders on whatever is not finished, and everybody else hauling.
    ///
    /// Both, and to different people. Assigning everyone to whichever building
    /// was unfinished left the farm standing with nobody on it, so nothing was
    /// ever grown and the city starved at tick 4599 with a perfectly good farm
    /// in it — which is the sim being right and the test being wrong.
    fn keep_working(&mut self, who: usize) {
        let player = self.steps[who].me;
        let mine: Vec<_> = self.steps[who]
            .world
            .citizens
            .iter()
            .filter(|c| c.owner == player && c.alive())
            .map(|c| c.id)
            .collect();
        if mine.len() < 5 {
            return;
        }

        let farm = self.steps[who]
            .world
            .buildings
            .iter()
            .find(|b| b.owner == player && b.kind == Kind::Farm && b.standing_now())
            .map(|b| b.id);
        if let Some(farm) = farm {
            let _ = self.steps[who]
                .issue(Command::Assign { citizens: mine[..3].to_vec(), building: farm });
        }

        let site = self.steps[who]
            .world
            .buildings
            .iter()
            .find(|b| b.owner == player && !b.standing_now() && b.kind != Kind::Hearth)
            .map(|b| b.id);
        if let Some(site) = site {
            let hands = if farm.is_some() { mine[3..5].to_vec() } else { mine[..3].to_vec() };
            let _ = self.steps[who].issue(Command::Assign { citizens: hands, building: site });
        }
    }
}

#[test]
fn everybody_gets_a_player_and_the_same_world() {
    let g = Game::new(3, Conditions::default());
    let ids: Vec<PlayerId> = g.steps.iter().map(|s| s.me).collect();
    assert_eq!(ids, vec![PlayerId(0), PlayerId(1), PlayerId(2)], "player ids");
    for s in &g.steps {
        assert_eq!(s.world.players.len(), 3, "everybody knows about everybody");
        assert_eq!(s.world.seed, 31, "and they are all on the host's seed");
    }
}

/// The plan's first phase-3 test: three players, real latency, identical
/// checksums, all the way into age two.
#[test]
fn three_players_stay_in_step_through_two_ages_with_latency() {
    let mut g = Game::new(3, Conditions { latency: 2, loss_percent: 0 });

    // Somebody does something now and then, so the turns are not all empty.
    // Every city is founded through commands, or they all starve before age
    // two and the world stops for reasons that have nothing to do with the
    // wire.
    for who in 0..3 {
        g.found(who);
    }
    g.run_to_tick(200);

    let mut done = 0;
    let target = sim::balance::TICKS_PER_DAY * sim::balance::DAYS_PER_AGE + 400;
    for round in 0..40 {
        g.run_to_tick((round + 1) * target / 40);
        for who in 0..3 {
            g.keep_working(who);
        }
        if round % 4 == 0 {
            let who = ((round / 4) % 3) as usize;
            let _ = g.steps[who].issue(Command::Ping { x: 10 + round as u8, y: 20 });
            done += 1;
        }
        let bad = g.disagreements();
        assert!(bad.is_empty(), "peers disagreed at ticks {:?}", &bad[..bad.len().min(3)]);
        if g.steps[0].world.finished().is_some() {
            break;
        }
    }

    assert!(done > 5, "nobody ever did anything");
    let ticks = g.ticks();
    assert!(ticks[0] >= target, "only reached tick {} of {target}", ticks[0]);
    assert!(g.steps[0].world.age() >= 2, "the game never reached age two");
    assert!(
        g.steps[0].world.players.iter().any(|&p| g.steps[0].world.population(p) > 0),
        "every city starved before age two"
    );
    assert!(g.net.broken_star().is_empty(), "somebody talked round the host");
}

/// The plan's second: a peer whose world is deliberately different is caught,
/// and everyone stops.
#[test]
fn a_peer_whose_world_differs_is_caught_and_the_game_stops() {
    let mut g = Game::new(3, Conditions::default());
    g.run(50);
    assert!(matches!(g.steps[0].status, Status::Playing | Status::WaitingOn(_)));

    // Reach past the lockstep and change one citizen's mind. This is the
    // divergence a stray `f32` or a `HashMap` iteration would cause, done on
    // purpose — and it is the exact experiment the plan asks for to prove the
    // guard works.
    g.steps[2].world.citizens[0].food -= 1;

    let mut caught = None;
    for _ in 0..50 {
        g.run(1);
        if let Status::Desync { with, tick } = g.steps[0].status.clone() {
            caught = Some((with, tick));
            break;
        }
    }
    let (with, _tick) = caught.expect("the host never noticed the worlds had parted");
    assert_eq!(with, PlayerId(2), "it blamed the wrong player");

    // And the game does not carry on regardless.
    let stopped_at = g.steps[0].tick();
    g.run(20);
    assert_eq!(g.steps[0].tick(), stopped_at, "the host kept simulating after a desync");
}

/// The plan's third: a silent player is dropped, and on the same tick for
/// everyone.
#[test]
fn a_silent_player_is_dropped_on_the_same_tick_everywhere() {
    let mut g = Game::new(3, Conditions::default());
    g.run(100);
    let before = g.steps[0].tick();

    // Player 2 stops advancing — a backgrounded tab that has stopped being
    // scheduled at all. Its connection is still open, so this is silence
    // rather than a disconnection.
    for _ in 0..(DROP_AFTER_TICKS + WAIT_WARN_TICKS + 50) {
        g.steps[0].advance(&mut g.peers[0]);
        g.steps[1].advance(&mut g.peers[1]);
        g.net.step();
    }

    assert!(g.steps[0].tick() > before, "the game never got going again");
    assert!(
        g.steps[0].world.dropped.contains(&PlayerId(2)),
        "the host never gave up on a player that went quiet"
    );
    assert!(
        g.steps[1].world.dropped.contains(&PlayerId(2)),
        "the other player did not hear about it"
    );

    // The same tick on both, because it was a command rather than a decision
    // each peer made for itself. Compared where their histories overlap: the
    // host is a tick or two ahead of the joiner at any instant.
    assert!(
        g.disagreements().is_empty(),
        "they dropped the player at different ticks: {:?}",
        g.disagreements()
    );
}

#[test]
fn a_player_who_goes_quiet_is_announced_before_being_given_up_on() {
    let mut g = Game::new(2, Conditions::default());
    g.run(50);

    let mut warned = false;
    for _ in 0..WAIT_WARN_TICKS + 20 {
        g.steps[0].advance(&mut g.peers[0]);
        g.net.step();
        if let Status::WaitingOn(who) = &g.steps[0].status {
            assert_eq!(who, &vec![PlayerId(1)]);
            warned = true;
            break;
        }
    }
    assert!(warned, "a player held the game up and nobody was told");
    assert!(
        !g.steps[0].world.dropped.contains(&PlayerId(1)),
        "warned and dropped in the same breath"
    );
}

/// The plan's fourth: somebody joins a game already in progress and catches up
/// from a snapshot.
#[test]
fn a_late_joiner_catches_up_from_a_snapshot() {
    // Four connections, but only three players to begin with.
    let net = Loopback::new(4, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..4).map(|i| net.peer(PeerId(i))).collect();
    // Hosted for four: a world is generated with a fixed number of cities and
    // a joiner takes one of them. There is no adding a city to a world that
    // has already been made — the map's hearth sites were placed for a
    // particular number of players.
    let mut steps = vec![Lockstep::host(31, 4, BUILD)];
    for p in peers.iter_mut().take(3).skip(1) {
        steps.push(Lockstep::join(BUILD, p));
    }

    let run = |steps: &mut Vec<Lockstep>, peers: &mut Vec<LoopbackPeer>, n: u32| {
        for _ in 0..n {
            for (i, ls) in steps.iter_mut().enumerate() {
                ls.advance(&mut peers[i]);
            }
            net.step();
        }
    };
    run(&mut steps, &mut peers, 20);
    steps[0].start();
    run(&mut steps, &mut peers, 60);
    assert!(steps[0].tick() > 0, "the game never started");

    // The fourth arrives.
    let joiner = Lockstep::join(BUILD, &mut peers[3]);
    steps.push(joiner);
    run(&mut steps, &mut peers, 120);

    assert_eq!(steps[3].me, PlayerId(3), "the joiner was never given a player");
    assert_eq!(steps[3].world.players.len(), 4, "the roster did not reach them");
    // Compared at a tick they have all reached, not at this instant: with the
    // host a couple of ticks ahead, "now" is a different world for each of
    // them.
    let common = steps.iter().map(|s| s.tick()).min().unwrap();
    run(&mut steps, &mut peers, 0);
    assert!(common > 0);

    // And they can play: a command of theirs reaches everybody.
    steps[3].issue(Command::Ping { x: 5, y: 5 }).unwrap();
    // Polled rather than checked after a run: a ping fades after
    // `PING_LIFETIME`, which is thirty ticks, so looking two hundred ticks
    // later finds nothing whether it arrived or not.
    let mut arrived = false;
    for _ in 0..200 {
        run(&mut steps, &mut peers, 1);
        if steps[0].world.pings.iter().any(|p| p.by == PlayerId(3)) {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the joiner's commands never arrived");
    // And the host's world and the joiner's agree once the joiner catches up.
    let target = steps[0].tick();
    for _ in 0..2000 {
        if steps[3].tick() >= target {
            break;
        }
        run(&mut steps, &mut peers, 1);
    }
    assert!(steps[3].tick() >= target, "the joiner never caught up");
}

#[test]
fn a_different_build_is_refused_with_a_reason() {
    let net = Loopback::new(2, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..2).map(|i| net.peer(PeerId(i))).collect();
    let mut host = Lockstep::host(31, 2, BUILD);
    let mut joiner = Lockstep::join("a-different-build", &mut peers[1]);

    for _ in 0..20 {
        host.advance(&mut peers[0]);
        joiner.advance(&mut peers[1]);
        net.step();
    }

    match &joiner.status {
        Status::Ended(reason) => {
            assert!(reason.contains("different builds"), "unhelpful refusal: {reason}");
            assert!(reason.contains("Reload"), "it should say what to do: {reason}");
        }
        other => panic!("a mismatched build was allowed in: {other:?}"),
    }
    assert_eq!(host.world.players.len(), 2, "the host counted them as a player anyway");
}

#[test]
fn a_command_the_rules_refuse_never_reaches_the_wire() {
    let mut g = Game::new(2, Conditions::default());
    g.run(30);

    // Somebody else's citizens are not yours to move, and the sender is told
    // straight away rather than watching it vanish three ticks later.
    let theirs = g.steps[0]
        .world
        .citizens
        .iter()
        .find(|c| c.owner == PlayerId(1))
        .unwrap()
        .id;
    let refused = g.steps[0].issue(Command::MoveTo { citizens: vec![theirs], x: 10, y: 10 });
    assert!(refused.is_err(), "the lockstep passed on a command the rules refuse");

    g.run_to_tick(g.steps[0].tick() + 30);
    assert!(g.disagreements().is_empty(), "peers parted over a refused command");
}

#[test]
fn losing_the_host_ends_the_game_for_a_joiner() {
    let mut g = Game::new(2, Conditions::default());
    g.run(40);
    g.net.disconnect(HOST);

    for _ in 0..10 {
        g.steps[1].advance(&mut g.peers[1]);
        g.net.step();
    }
    match &g.steps[1].status {
        Status::Ended(reason) => assert!(reason.contains("host"), "{reason}"),
        other => panic!("the host vanished and the joiner played on: {other:?}"),
    }
}

#[test]
fn a_dropped_connection_drops_the_player() {
    let mut g = Game::new(3, Conditions::default());
    g.run(60);
    g.net.disconnect(PeerId(2));

    for _ in 0..60 {
        g.steps[0].advance(&mut g.peers[0]);
        g.steps[1].advance(&mut g.peers[1]);
        g.net.step();
    }
    assert!(
        g.steps[0].world.dropped.contains(&PlayerId(2)),
        "a closed connection did not drop the player"
    );
    assert!(
        g.disagreements().is_empty(),
        "the remaining players parted over it: {:?}",
        g.disagreements()
    );
}

#[test]
fn commands_take_effect_after_the_input_delay_and_on_every_peer_at_once() {
    let mut g = Game::new(2, Conditions::default());
    g.run(40);

    g.steps[1].issue(Command::Ping { x: 33, y: 44 }).unwrap();
    let issued_at = g.steps[1].tick();

    let mut landed = None;
    for _ in 0..200 {
        g.run(1);
        if g.steps[0].world.pings.iter().any(|p| p.x == 33) {
            landed = Some(g.steps[0].tick());
            break;
        }
    }
    let at = landed.expect("the ping never landed");
    assert!(at > issued_at, "it took effect before it was issued");

    // And it reaches everybody. The joiner is a tick or two behind the host at
    // any moment, so this needs the world run on, not sampled at once.
    g.run_to_tick(at + 20);
    assert!(
        g.steps[1].world.pings.iter().any(|p| p.x == 33),
        "the ping never reached the peer that issued it"
    );
    assert!(g.disagreements().is_empty(), "peers parted over a ping");
}

#[test]
fn an_unreliable_channel_that_loses_messages_changes_nothing() {
    // Turns go on the reliable channel; only cursors and chat do not. A lossy
    // wire must not be able to desync a game.
    let mut g = Game::new(3, Conditions { latency: 2, loss_percent: 30 });
    g.run_to_tick(400);
    assert!(g.disagreements().is_empty(), "loss desynced the game");
    assert!(g.steps[0].tick() >= 400, "the game stalled");
}
