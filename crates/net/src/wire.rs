//! What goes over the reliable channel, from design §8.
//!
//! `postcard`, like everything else that has to mean the same thing on two
//! machines. These are the only messages there are; a peer that receives
//! something it cannot decode has a different build, which is what
//! `build_hash` in `Hello` exists to catch before it matters.

use serde::{Deserialize, Serialize};
use sim::{Command, PlayerId, World};

/// Bumped when the shape of anything here changes. `build_hash` catches
/// mismatches between two deployments of the game; this catches a mismatch
/// between two versions of the protocol, which is a clearer thing to report.
pub const PROTO_VERSION: u16 = 1;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Message {
    /// Joiner → host, the first thing said.
    Hello { proto_version: u16, build_hash: String, name: String },
    /// Host → joiner, in answer. The snapshot is `None` before the run starts,
    /// when the seed is enough to build the same world.
    Welcome {
        player: PlayerId,
        seed: u64,
        tick: u32,
        players: Vec<PlayerId>,
        snapshot: Option<Box<World>>,
    },
    /// Host → everyone, whenever the roster changes.
    Roster { players: Vec<PlayerId> },
    /// One player's commands for one tick. Joiner → host.
    ///
    /// The checksum is tagged with the tick it belongs to rather than being
    /// implicitly "the tick before this one", which is what design §8 says and
    /// cannot be true: a turn for tick T is sent `DELAY` ticks early, so the
    /// sender does not yet know what the world will look like after T − 1. It
    /// reports the last tick it has actually finished. Untagged, a late joiner
    /// priming its pipeline sends four turns carrying the same checksum, the
    /// host reads them as claims about four different ticks, and an innocent
    /// player is thrown out for a desync on the tick it arrived.
    Turn {
        player: PlayerId,
        tick: u32,
        commands: Vec<Command>,
        checked_tick: u32,
        checksum: u64,
    },
    /// Host → everyone: the commands for a tick, from every live player. This
    /// is the thing peers actually advance on.
    Bundle { tick: u32, turns: Vec<(PlayerId, Vec<Command>)> },
    /// Something went wrong that the other end should hear about.
    Bye { reason: String },
}

/// Why the host refused a `Hello`, or why a game stopped.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Refusal {
    /// A different build of the game. Design §8: mismatched builds cannot
    /// join, which prevents the commonest desync before it can happen.
    BuildMismatch { theirs: String, ours: String },
    ProtoMismatch { theirs: u16, ours: u16 },
    /// Six is the most design §6 allows.
    GameFull,
    /// The run has started and this joiner missed the window (MVP: joining is
    /// only allowed before age one).
    TooLate,
}

impl Refusal {
    /// Something a person can read, because phase 6 wants failures that say
    /// what to do about them.
    pub fn to_message(&self) -> String {
        match self {
            Refusal::BuildMismatch { theirs, ours } => format!(
                "different builds: they have {theirs}, this game is {ours}. \
                 Reload the page and try again."
            ),
            Refusal::ProtoMismatch { .. } => {
                "different versions of the game. Reload the page.".to_owned()
            }
            Refusal::GameFull => "this game is full.".to_owned(),
            Refusal::TooLate => "this game has already started.".to_owned(),
        }
    }
}

pub fn encode(m: &Message) -> Vec<u8> {
    postcard::to_allocvec(m).expect("a Message is always encodable")
}

pub fn decode(bytes: &[u8]) -> Option<Message> {
    postcard::from_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_survives_the_wire() {
        let world = World::new(9, 2);
        for m in [
            Message::Hello {
                proto_version: PROTO_VERSION,
                build_hash: "abc123".into(),
                name: "Alice".into(),
            },
            Message::Welcome {
                player: PlayerId(1),
                seed: 42,
                tick: 0,
                players: vec![PlayerId(0), PlayerId(1)],
                snapshot: None,
            },
            Message::Welcome {
                player: PlayerId(1),
                seed: 42,
                tick: 300,
                players: vec![PlayerId(0), PlayerId(1)],
                snapshot: Some(Box::new(world.clone())),
            },
            Message::Roster { players: vec![PlayerId(0)] },
            Message::Turn {
                player: PlayerId(0),
                tick: 7,
                commands: vec![sim::Command::Ping { x: 1, y: 2 }],
                checked_tick: 4,
                checksum: 0xDEAD_BEEF,
            },
            Message::Bundle {
                tick: 7,
                turns: vec![(PlayerId(0), vec![sim::Command::Pause]), (PlayerId(1), vec![])],
            },
            Message::Bye { reason: "went to lunch".into() },
        ] {
            let bytes = encode(&m);
            assert_eq!(decode(&bytes).as_ref(), Some(&m), "{m:?}");
        }
    }

    #[test]
    fn a_snapshot_is_a_sendable_size() {
        // Design §8 budgets 50–150 KB for a late joiner's Welcome.
        let m = Message::Welcome {
            player: PlayerId(1),
            seed: 1,
            tick: 100,
            players: vec![PlayerId(0), PlayerId(1)],
            snapshot: Some(Box::new(World::new(1, 6))),
        };
        let n = encode(&m).len();
        assert!(n < 150_000, "a Welcome with a snapshot is {n} bytes");
    }

    #[test]
    fn rubbish_is_not_a_message() {
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(&[0xFF; 32]), None);
    }

    #[test]
    fn a_refusal_says_what_to_do_about_it() {
        let r = Refusal::BuildMismatch { theirs: "aaa".into(), ours: "bbb".into() };
        let text = r.to_message();
        assert!(text.contains("aaa") && text.contains("bbb"));
        assert!(text.contains("Reload"), "it should say what to do: {text}");
    }
}
