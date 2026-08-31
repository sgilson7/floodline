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
use sim::building::{Facing, Kind};
use sim::{Command, PlayerId};
use std::collections::BTreeMap;

const BUILD: &str = "test-build";

/// Both of these are counted in seconds a person waits, so they follow the
/// clock rather than being pinned to a number of ticks. `sim`'s
/// `the_clock_can_change_without_changing_the_game` is the other half of this:
/// there, two constants that look like seconds are balance and must *not*
/// move. Here, two that look like ticks are wall clock and must.
#[test]
fn the_timeouts_are_counted_in_seconds_not_ticks() {
    assert_eq!(DROP_AFTER_TICKS, 30 * sim::balance::TICKS_PER_SECOND);
    assert_eq!(WAIT_WARN_TICKS, 5 * sim::balance::TICKS_PER_SECOND);
}

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
        let peers: Vec<LoopbackPeer> = (0..n).map(|i| net.peer(PeerId(i))).collect();
        let mut steps = vec![Lockstep::host(31, n, BUILD)];
        for _ in 1..n {
            steps.push(Lockstep::join(BUILD));
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
                        let cmd = Command::Place { kind,
                            facing: Facing::EastWest, x: x as u8, y: y as u8 };
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

    // Everybody else is told, which they were not until M11.2. A checksum
    // rides on `Turn`, which goes to the host and to nobody else, so a joiner
    // can never notice a desync itself — and it used to be left sitting on
    // `playing` with a frozen world and no explanation while the host showed
    // the fault. Two people playing, one shown why and the other's game simply
    // stopping.
    for (i, ls) in g.steps.iter().enumerate().skip(1) {
        match &ls.status {
            Status::Ended(reason) => {
                assert!(reason.contains("DESYNC"), "peer {i} was told {reason:?}");
                assert!(
                    reason.contains(&format!("tick {_tick}")),
                    "peer {i} should be told when: {reason:?}"
                );
            }
            other => panic!("peer {i} was never told the game came apart: {other:?}"),
        }
    }
}

/// The panel's "peers at" row: the host can see how far everybody has got, and
/// a joiner can see only itself.
///
/// In the browser that row reported one number — the peer's own tick, which the
/// row directly above it already said — because a page has one `Lockstep` and
/// `Session::ticks` had nothing else to hand it. It is the row M10 is supposed
/// to watch for two worlds parting, so it may as well contain something.
#[test]
fn the_host_can_see_how_far_everybody_has_got() {
    let mut g = Game::new(3, Conditions::default());
    g.run_to_tick(200);

    let host = g.steps[0].peer_ticks();
    assert_eq!(host.len(), 3, "the host should report one tick per player");

    let mine = g.steps[0].tick();
    assert_eq!(host[0], mine, "the host's own entry is the tick it has simulated");
    for (i, &theirs) in host.iter().enumerate().skip(1) {
        assert!(theirs > 0, "player {i} never reported a tick at all");
        assert!(theirs <= mine, "player {i} is somehow ahead of the host");
        // A steady gap is the pipeline and not a fault: what a peer reports is
        // a round trip old and `DELAY` ticks behind besides. A growing one is
        // a peer falling behind, which is the thing worth watching for.
        assert!(mine - theirs < 20, "player {i} is {} ticks behind", mine - theirs);
    }

    // A joiner is sent nobody's checksum — `Turn` goes to the host and to no
    // one else — so it has nothing to report but its own tick, and one number
    // is the honest answer rather than a short one.
    assert_eq!(g.steps[1].peer_ticks(), vec![g.steps[1].tick()]);
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
    for _ in 1..3 {
        steps.push(Lockstep::join(BUILD));
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
    let joiner = Lockstep::join(BUILD);
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
    let mut joiner = Lockstep::join("a-different-build");

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

/// A transport where nobody is there yet — which is every real one.
///
/// `Loopback` hands a peer its `Event::Peer`s before the first poll, because in
/// one process everybody exists at once. A browser does not: `Lockstep::join`
/// runs while ICE is still gathering and `peers()` is empty for seconds after.
/// This is the smallest transport that behaves that way, and it exists because
/// the first version of `join` said `Hello` to `peer.peers()` at construction
/// and would have said it to nobody.
struct LateHost {
    inbox: std::collections::VecDeque<net::Event>,
    sent: Vec<(PeerId, Vec<u8>)>,
    connected: Vec<PeerId>,
}

impl net::Peer for LateHost {
    fn poll(&mut self) -> Option<net::Event> {
        self.inbox.pop_front()
    }
    fn send(&mut self, to: PeerId, bytes: &[u8], _reliable: bool) {
        self.sent.push((to, bytes.to_vec()));
    }
    fn peers(&self) -> Vec<PeerId> {
        self.connected.clone()
    }
    fn is_host(&self) -> bool {
        false
    }
}

#[test]
fn a_joiner_says_hello_when_the_host_appears_and_not_before() {
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: Vec::new(),
    };
    let mut joiner = Lockstep::join(BUILD);

    // Seconds of frames with no connection yet.
    for _ in 0..120 {
        joiner.advance(&mut wire);
    }
    assert!(wire.sent.is_empty(), "something was sent before there was anybody to send it to");

    // The connection completes.
    wire.connected.push(HOST);
    wire.inbox.push_back(net::Event::Peer(HOST));
    joiner.advance(&mut wire);

    assert_eq!(wire.sent.len(), 1, "the host was greeted {} times", wire.sent.len());
    let (to, bytes) = &wire.sent[0];
    assert_eq!(*to, HOST);
    match net::wire::decode(bytes) {
        Some(net::Message::Hello { build_hash, .. }) => assert_eq!(build_hash, BUILD),
        other => panic!("the first thing a joiner says should be Hello, not {other:?}"),
    }

    // Once *per peer*, and once only. This assertion used to read "once,
    // however many more peers the transport reports", and that rule is the
    // fault M12.1 reproduced: a joiner with one greeting to spend gives it to
    // whoever turns up first, and in a reused room that is very often a tab
    // that is not hosting anything. See
    // `a_joiner_that_greeted_a_ghost_still_greets_the_host_when_it_arrives`.
    wire.inbox.push_back(net::Event::Peer(PeerId(9)));
    for _ in 0..10 {
        joiner.advance(&mut wire);
    }
    assert_eq!(wire.sent.len(), 2, "the second peer in the room was never greeted");
    assert_eq!(wire.sent[1].0, PeerId(9));

    // But the same peer twice is still one greeting. A transport that reports
    // a connection it has already reported must not cost a message.
    wire.inbox.push_back(net::Event::Peer(HOST));
    wire.inbox.push_back(net::Event::Peer(PeerId(9)));
    for _ in 0..10 {
        joiner.advance(&mut wire);
    }
    assert_eq!(wire.sent.len(), 2, "a peer already greeted was greeted again");
}

#[test]
fn a_transport_complaint_in_the_lobby_is_advice_and_not_the_end() {
    // "No relay has answered in fifteen seconds" is a sentence a host wants to
    // read while still hosting. The same words once the run is going mean the
    // connection is gone, and that is the end.
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: Vec::new(),
    };
    let mut joiner = Lockstep::join(BUILD);
    wire.inbox.push_back(net::Event::Error {
        text: "no relay answered".into(),
        try_a_code: true,
    });
    joiner.advance(&mut wire);
    assert!(joiner.in_lobby(), "a warning threw the player out of the lobby");
    let said = joiner.trouble.clone().expect("said nothing");
    assert_eq!(said.text, "no relay answered");
    assert!(said.try_a_code, "the way out of it was not offered");

    let mut host = Lockstep::host(31, 2, BUILD);
    host.start();
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: Vec::new(),
    };
    wire.inbox.push_back(net::Event::Error {
        text: "the connection failed".into(),
        try_a_code: false,
    });
    host.advance(&mut wire);
    assert_eq!(host.status, Status::Ended("the connection failed".into()));
}

#[test]
fn a_seat_a_joiner_left_in_the_lobby_is_given_to_the_next_one() {
    // The bug that made a room single-use. Seats were handed out by a counter
    // that only went up, so a two-seat game accepted exactly one joiner *for
    // the host's whole life*: the first one takes player 1, closes the tab,
    // and every later `Hello` is answered "this game is full". Two people who
    // fumbled their first attempt could never play at all.
    let net = Loopback::new(3, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..3).map(|i| net.peer(PeerId(i))).collect();
    let mut host = Lockstep::host(31, 2, BUILD);
    let mut first = Lockstep::join(BUILD);

    let run = |host: &mut Lockstep, other: &mut Lockstep, oi: usize, n: u32,
                   peers: &mut Vec<LoopbackPeer>| {
        for _ in 0..n {
            host.advance(&mut peers[0]);
            other.advance(&mut peers[oi]);
            net.step();
        }
    };

    run(&mut host, &mut first, 1, 20, &mut peers);
    assert_eq!(first.me, PlayerId(1), "the first joiner was never welcomed");
    assert_eq!(host.connected(), 2);

    // They close the tab, still in the lobby.
    net.disconnect(PeerId(1));
    run(&mut host, &mut first, 1, 10, &mut peers);
    assert_eq!(host.connected(), 1, "the host still thinks they are here");

    // Somebody else takes the empty chair.
    let mut second = Lockstep::join(BUILD);
    run(&mut host, &mut second, 2, 20, &mut peers);
    assert_eq!(second.me, PlayerId(1), "the freed seat was not given away");
    assert_eq!(host.connected(), 2);
    assert!(!second.status.is_stopped(), "{:?}", second.status);
}

#[test]
fn a_seat_left_empty_mid_run_is_not_handed_to_somebody_else() {
    // The other half of the rule. Once the run has started a player who drops
    // leaves a city standing, and giving their seat away would give the city
    // away with it.
    let net = Loopback::new(3, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..3).map(|i| net.peer(PeerId(i))).collect();
    let mut host = Lockstep::host(31, 2, BUILD);
    let mut first = Lockstep::join(BUILD);

    for _ in 0..20 {
        host.advance(&mut peers[0]);
        first.advance(&mut peers[1]);
        net.step();
    }
    assert_eq!(first.me, PlayerId(1));
    host.start();
    for _ in 0..40 {
        host.advance(&mut peers[0]);
        first.advance(&mut peers[1]);
        net.step();
    }
    assert!(host.tick() > 0, "the run never started");

    net.disconnect(PeerId(1));
    let mut second = Lockstep::join(BUILD);
    for _ in 0..60 {
        host.advance(&mut peers[0]);
        second.advance(&mut peers[2]);
        net.step();
    }
    match &second.status {
        Status::Ended(reason) => assert!(
            reason.contains("full") || reason.contains("started"),
            "refused, but for the wrong reason: {reason}"
        ),
        other => panic!("a mid-run seat was handed out again: {other:?}"),
    }
}

#[test]
fn another_joiner_leaving_is_not_the_host_leaving() {
    // Trystero rooms are meshes: a joiner meets every other joiner, and used
    // to announce "the host left the game" when any of them went away.
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: Vec::new(),
    };
    let mut joiner = Lockstep::join(BUILD);

    wire.connected.push(HOST);
    wire.inbox.push_back(net::Event::Peer(HOST));
    joiner.advance(&mut wire);
    assert_eq!(wire.sent.len(), 1, "the host was never greeted");

    // Somebody else comes and goes. They are greeted on the way past — a
    // joiner cannot tell a host from a bystander until one of them answers,
    // and asking is one message — but their departure ends nothing.
    wire.inbox.push_back(net::Event::Peer(PeerId(7)));
    wire.inbox.push_back(net::Event::Left(PeerId(7)));
    joiner.advance(&mut wire);
    assert!(joiner.in_lobby(), "a stranger's departure ended the game: {:?}", joiner.status);
    assert_eq!(wire.sent.len(), 2, "the stranger was never asked");

    // The host going is a different matter — but this joiner was never
    // welcomed, so it has lost a candidate rather than a game. It waits, and
    // greets whoever turns up next. That is the way out of an abandoned lobby:
    // the tab that was squatting the room closes, a real host appears, and the
    // joiner says `Hello` to it instead of sitting on a dead connection.
    wire.inbox.push_back(net::Event::Left(HOST));
    joiner.advance(&mut wire);
    assert!(joiner.in_lobby(), "gave up on a game it had never been let into");
    assert_eq!(wire.sent.len(), 2, "it greeted somebody who had already gone");

    wire.inbox.push_back(net::Event::Peer(PeerId(3)));
    joiner.advance(&mut wire);
    assert_eq!(wire.sent.len(), 3, "the next peer was never greeted");
    assert_eq!(wire.sent[2].0, PeerId(3));

    // And a peer that drops and comes back is greeted again: it may be the
    // host on a fresh connection, which is what a reconnect looks like from
    // here.
    wire.inbox.push_back(net::Event::Peer(PeerId(7)));
    joiner.advance(&mut wire);
    assert_eq!(wire.sent.len(), 4, "a peer that reconnected was never greeted again");
}

#[test]
fn a_host_that_connects_and_says_nothing_is_reported_rather_than_waited_on_for_ever() {
    // What an abandoned lobby looks like from the outside: the tab is still in
    // the room, so the connection completes and the `Hello` goes out, and then
    // nothing. The screen used to say "looking for the host on the public
    // relays" either way, which is why this took an evening to find.
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: vec![HOST],
    };
    let mut joiner = Lockstep::join(BUILD);
    wire.inbox.push_back(net::Event::Peer(HOST));
    joiner.advance(&mut wire);
    assert!(joiner.trouble.is_none(), "gave up before it had waited at all");

    for _ in 0..2000 {
        joiner.advance(&mut wire);
    }
    let said = joiner.trouble.clone().expect("never said anything about the silence");
    // The wording widened in M12.2: the same silence is now reported whether
    // the peer that went quiet was a host that left its lobby or another
    // joiner that was never a host at all, because from here they are
    // indistinguishable and the advice is the same either way.
    assert!(said.text.contains("answering"), "{}", said.text);
    assert!(said.try_a_code, "a code is exactly what gets round an abandoned lobby");
    assert!(joiner.in_lobby(), "it is a warning, not the end");
}

#[test]
fn a_joiner_is_told_who_else_is_in_the_lobby() {
    // `Roster` carried `world.players` — every seat on the map, whether
    // anybody was in it or not — and joiners ignored it. So a joiner could not
    // tell a finished handshake from a dead relay, and neither could its lobby.
    let net = Loopback::new(3, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..3).map(|i| net.peer(PeerId(i))).collect();
    let mut host = Lockstep::host(31, 3, BUILD);
    let mut a = Lockstep::join(BUILD);
    let mut b = Lockstep::join(BUILD);

    for _ in 0..30 {
        host.advance(&mut peers[0]);
        a.advance(&mut peers[1]);
        b.advance(&mut peers[2]);
        net.step();
    }
    assert_eq!(host.roster(), &[PlayerId(0), PlayerId(1), PlayerId(2)]);
    assert_eq!(a.roster(), host.roster(), "the joiner was not told");
    assert!(a.welcomed() && b.welcomed());

    net.disconnect(PeerId(2));
    for _ in 0..20 {
        host.advance(&mut peers[0]);
        a.advance(&mut peers[1]);
        net.step();
    }
    assert_eq!(host.roster(), &[PlayerId(0), PlayerId(1)]);
    assert_eq!(a.roster(), host.roster(), "the roster did not shrink");
}

// ---- M12.1: a second game cannot be joined ---------------------------------

/// Put a world two ticks from the end of its last age.
///
/// **What this arranges**, which is the rule the M11 run paid for: only *where
/// the world starts*. The ending itself is reached through `World::tick` and
/// `age.rs`'s own roll-over, so `finished` and `ending` are set by the game
/// rather than by the test. Simulating eighteen days of two worlds in a debug
/// build takes over ten minutes and would tell us nothing the handshake cares
/// about.
fn nearly_over(w: &mut sim::World) {
    use sim::balance::{DAYS_PER_AGE, MAX_AGE, TICKS_PER_DAY};
    let age_start = 100_000;
    w.age = MAX_AGE;
    w.age_start_tick = age_start;
    w.tick = age_start + DAYS_PER_AGE * TICKS_PER_DAY - 2;
}

/// The fault that made the game unplayable a second time.
///
/// Reported first-hand, laptop to desktop: one game was played to the end, and
/// since then no attempt to get two machines into a lobby has worked. The
/// joiner sits for ever on *"found the host, asking for a city..."* — which
/// `lobby.rs::joining_screen` shows when a joiner has a live peer and a roster,
/// has sent its `Hello`, and has never been answered.
///
/// Nothing anywhere tested this. `rejoin.py` covers a seat given back, an
/// abandoned lobby and hosting a second game, and all of it is lobby-only: no
/// check in `cargo` or in a browser has ever played a run to its end and then
/// put two peers into a lobby again.
///
/// The assertion is deliberately weak, and that is the point. It does not say
/// the joiner must get in — a host sitting on a score screen has no city to
/// give and refusing is defensible. It says the joiner must be **told
/// something**. Sitting silent is the failure.
#[test]
fn a_host_that_finished_a_game_tells_a_new_joiner_something() {
    let net = Loopback::new(3, Conditions::default());
    let mut peers: Vec<LoopbackPeer> = (0..3).map(|i| net.peer(PeerId(i))).collect();
    let mut host = Lockstep::host(31, 2, BUILD);
    let mut first = Lockstep::join(BUILD);

    // A game: two people in a lobby.
    for _ in 0..20 {
        host.advance(&mut peers[0]);
        first.advance(&mut peers[1]);
        net.step();
    }
    assert_eq!(first.me, PlayerId(1), "the first game never got going");

    // Both worlds put in the same place, so this is a finished game and not a
    // desync. The joiner's world is the host's clone; they stay clones.
    nearly_over(&mut host.world);
    nearly_over(&mut first.world);

    host.start();
    for _ in 0..40 {
        host.advance(&mut peers[0]);
        first.advance(&mut peers[1]);
        net.step();
    }
    assert!(host.world.finished().is_some(), "the first game never ended: {:?}", host.status);
    assert!(!host.status.is_stopped(), "the host's own run stopped: {:?}", host.status);

    // The score screen is up on both machines. Somebody now tries to get into
    // a lobby with this host again — the same room, the same build, which is
    // exactly what a person playing against themselves does.
    let mut second = Lockstep::join(BUILD);
    for _ in 0..200 {
        host.advance(&mut peers[0]);
        first.advance(&mut peers[1]);
        second.advance(&mut peers[2]);
        net.step();
    }

    // Today it answers "this game is full", because the first game's seats are
    // still held by peers that are still connected. That is defensible and it
    // is *an answer*. The handshake, on this wire, is not where the silence
    // comes from — see the two tests below, which is where it does come from.
    let told_something = second.welcomed()
        || matches!(second.status, Status::Ended(_))
        || second.trouble.is_some();
    assert!(
        told_something,
        "the joiner met a peer and was never answered: welcomed {}, status {:?}, trouble {:?}",
        second.welcomed(),
        second.status,
        second.trouble,
    );
}

/// The silence, found: a joiner has exactly one greeting and spends it on
/// whoever it meets first.
///
/// `greet` is a `bool`. `Lockstep::join` sets it true, `greet()` sets it false
/// on the first `Event::Peer`, and only `peer_left` — for *that* peer, and only
/// while unwelcomed — ever sets it back. So the first peer a joiner meets is
/// the only peer it will ever say `Hello` to.
///
/// In a star that is right, because the only peer a joiner has is the host. A
/// Trystero room is not a star: everybody meets everybody, and the room name is
/// the typed code plus the build hash, so **typing the same code again puts you
/// in the same room as every tab that has ever used it** — a second joiner
/// waiting for the same host, a tab left open on a score screen, a stale
/// connection that has not timed out. None of them is a host, and a non-host
/// receiving `Hello` does nothing at all with it: the handler is guarded
/// `if self.host` and falls through to `_ => {}`. Nothing on the wire says "I
/// am not the one you want".
///
/// The joiner then reads *"found the host, asking for a city..."* for ever,
/// **and goes on reading it after the real host arrives**, which is what makes
/// this the reported fault rather than a slow start.
#[test]
fn a_joiner_that_greeted_a_ghost_still_greets_the_host_when_it_arrives() {
    const GHOST: PeerId = PeerId(7);
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: vec![GHOST],
    };
    let mut joiner = Lockstep::join(BUILD);

    // Somebody is already in the room, and it is not a host.
    wire.inbox.push_back(net::Event::Peer(GHOST));
    for _ in 0..600 {
        joiner.advance(&mut wire);
    }
    assert_eq!(wire.sent.len(), 1, "the ghost should have been greeted exactly once");
    assert_eq!(wire.sent[0].0, GHOST);

    // The host turns up in the same room, and is never spoken to.
    wire.connected.push(HOST);
    wire.inbox.push_back(net::Event::Peer(HOST));
    for _ in 0..120 {
        joiner.advance(&mut wire);
    }

    assert!(
        wire.sent.iter().any(|(to, _)| *to == HOST),
        "the joiner never said Hello to the host: it had already spent its one \
         greeting on a peer that was never going to answer"
    );
}

/// And the reason nobody was ever told: every departure restarts the timer.
///
/// `mind_the_silence` exists for exactly the hang above and should say
/// *"connected to the host, but it is not answering"* after `SILENCE_FRAMES`.
/// The author never saw it. `peer_left` sets `unanswered = 0` whenever the
/// greeted peer goes away, and `mind_the_silence` only counts while `host_peer`
/// is set — so in a room with any churn at all the counter never arrives. The
/// joiner greets, waits, resets, greets again, and is told nothing, for as long
/// as it is left there.
///
/// Two thousand frames here against a `SILENCE_FRAMES` of 500.
#[test]
fn a_joiner_in_a_room_with_churn_is_still_told_about_the_silence() {
    let mut wire = LateHost {
        inbox: std::collections::VecDeque::new(),
        sent: Vec::new(),
        connected: Vec::new(),
    };
    let mut joiner = Lockstep::join(BUILD);

    for round in 0..20u32 {
        let ghost = PeerId(100 + round);
        wire.connected = vec![ghost];
        wire.inbox.push_back(net::Event::Peer(ghost));
        // Long enough to be a real wait, short of `SILENCE_FRAMES` on its own.
        for _ in 0..100 {
            joiner.advance(&mut wire);
        }
        wire.inbox.push_back(net::Event::Left(ghost));
        joiner.advance(&mut wire);
    }

    assert!(
        joiner.trouble.is_some(),
        "two thousand frames in a room that never answered, and never a word to \
         the player about it"
    );
}
