//! Everything a player can do, and nothing else.
//!
//! gear-master's `Verb` by another name, and for its reasons: one enum whose
//! variants are the complete set of a player's affordances, a `line`/`parse`
//! pair so a game is a replayable transcript, and the principle that what is
//! deliberately absent is as much of the definition as what is present. There
//! is no `set_seed`, no `spawn`, no `give`. A peer holding a `Command` cannot
//! spell them.
//!
//! It matters more here than it did there. A command is the only thing that
//! goes over the wire (design §8), so this enum is the wire format, and
//! `World::apply` is the one door into the world. Every rule lives inside it,
//! including ownership — so a peer commanding another city's citizens is
//! rejected identically on every machine, whether it did so by bug, by desync
//! or by tampering.

use crate::building::{BuildingId, Facing, Good, Kind};
use crate::citizen::{CitizenId, PlayerId};
use crate::road::{RoadId, TradeId};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Command {
    /// Start a construction site. `facing` is which way the footprint runs
    /// and is ignored by every kind that is square — but it is carried by all
    /// of them, so there is one `Place` on the wire and not two.
    Place { kind: Kind, facing: Facing, x: u8, y: u8 },
    /// Pull something down, or clear rubble.
    Demolish { building: BuildingId },
    /// Raise a dike by one level. Design §3.3 has dikes grow; this is how.
    RaiseDike { dike: BuildingId },
    /// Buy a building one more level, in gold. A level is one more citizen it
    /// can hold — see `Building::slots_for`.
    Upgrade { building: BuildingId },
    /// Pick a building up and put it down somewhere else.
    ///
    /// It keeps its id, its store and its level, and arrives as a construction
    /// site with its materials already in it — so moving costs time and not
    /// materials, and the building shelters nobody and produces nothing until
    /// it is finished. That is what makes moving the granary the day before the
    /// water comes a decision rather than a free tidy-up.
    Move { building: BuildingId, x: u8, y: u8 },
    /// Put citizens to work at a building. Which job is decided by what the
    /// building is, exactly as right-clicking it would.
    Assign { citizens: Vec<CitizenId>, building: BuildingId },
    /// Back to hauling, which is what an unassigned citizen does.
    Unassign { citizens: Vec<CitizenId> },
    /// "Get uphill." The one order that matters during a flood.
    MoveTo { citizens: Vec<CitizenId>, x: u8, y: u8 },
    SetHome { citizens: Vec<CitizenId>, cottage: BuildingId },
    /// Point at something. Changes nothing, but it is a command rather than a
    /// chat message so it lands on the same tick for everyone and replays with
    /// the rest of the game (design §6).
    Ping { x: u8, y: u8 },
    /// Draw a wall along a straight run, in three-cell segments.
    ///
    /// Beside `Road` rather than reusing it, because the two are opposites: a
    /// road takes the cheapest path between two cells and a wall stays on the
    /// line you drew. They would share a name and nothing else.
    DikeLine { from: (u8, u8), to: (u8, u8) },
    /// Lay a road from one cell to another along the cheapest path, bridging
    /// shallows where it must (design §6).
    Road { from: (u8, u8), to: (u8, u8) },
    /// Join a road somebody laid to your city. Until this, it is their road
    /// that happens to end near you.
    AcceptRoad { road: RoadId },
    /// Propose a standing daily exchange.
    Trade { with: PlayerId, give: (Good, u16), take: (Good, u16) },
    AcceptTrade { trade: TradeId },
    /// Stop the world. Design §6: it takes everyone's `Resume` to lift.
    Pause,
    Resume,
    /// Give up on a player who has stopped sending.
    ///
    /// Design §8: after thirty seconds of silence the host emits this "as a
    /// command so it is deterministic". It is a command rather than a decision
    /// each peer makes for itself precisely because each peer would make it at
    /// a slightly different moment, and a world that drops somebody on tick
    /// 4001 is a different world from one that drops them on 4002.
    Drop { player: PlayerId },
}

impl Command {
    /// The citizens this command speaks for, if any. Every one of them has to
    /// belong to the player issuing it.
    pub fn citizens(&self) -> &[CitizenId] {
        match self {
            Command::Assign { citizens, .. }
            | Command::Unassign { citizens }
            | Command::MoveTo { citizens, .. }
            | Command::SetHome { citizens, .. } => citizens,
            _ => &[],
        }
    }

    /// One line of a transcript: what a person would type.
    ///
    /// The `bot` crate's script mode in phase 4 reads these, which is how a
    /// real browser gets a deterministic partner to disagree with.
    pub fn line(&self) -> String {
        let ids = |c: &[CitizenId]| {
            c.iter().map(|i| format!("#{}", i.0)).collect::<Vec<_>>().join(",")
        };
        match self {
            Command::Place { kind, facing, x, y } => {
                format!("place {} {x} {y} {}", kind_key(*kind), facing_key(*facing))
            }
            Command::Demolish { building } => format!("demolish @{}", building.0),
            Command::RaiseDike { dike } => format!("raise @{}", dike.0),
            Command::Upgrade { building } => format!("upgrade @{}", building.0),
            Command::Move { building, x, y } => format!("shift @{} {x} {y}", building.0),
            Command::Assign { citizens, building } => {
                format!("assign {} @{}", ids(citizens), building.0)
            }
            Command::Unassign { citizens } => format!("unassign {}", ids(citizens)),
            Command::MoveTo { citizens, x, y } => format!("move {} {x} {y}", ids(citizens)),
            Command::SetHome { citizens, cottage } => {
                format!("home {} @{}", ids(citizens), cottage.0)
            }
            Command::DikeLine { from, to } => {
                format!("dikeline {} {} {} {}", from.0, from.1, to.0, to.1)
            }
            Command::Road { from, to } => {
                format!("road {} {} {} {}", from.0, from.1, to.0, to.1)
            }
            Command::AcceptRoad { road } => format!("acceptroad %{}", road.0),
            Command::Trade { with, give, take } => format!(
                "trade !{} {} {} {} {}",
                with.0,
                good_key(give.0),
                give.1,
                good_key(take.0),
                take.1
            ),
            Command::AcceptTrade { trade } => format!("accepttrade ${}", trade.0),
            Command::Ping { x, y } => format!("ping {x} {y}"),
            Command::Pause => "pause".into(),
            Command::Resume => "resume".into(),
            Command::Drop { player } => format!("drop !{}", player.0),
        }
    }

    /// Read a transcript line back.
    ///
    /// `None` means "that is not a command", which is not the same as "that
    /// command is illegal here" — `World::apply` answers the second question.
    pub fn parse(line: &str) -> Option<Command> {
        // `;` starts a comment, because `#` is already the citizen marker.
        let raw = line.split(';').next().unwrap_or("").trim();
        let parts: Vec<&str> = raw.split_whitespace().collect();

        let ids = |s: &str| -> Option<Vec<CitizenId>> {
            s.split(',')
                .map(|t| t.trim().strip_prefix('#')?.parse().ok().map(CitizenId))
                .collect()
        };
        let bid = |s: &str| -> Option<BuildingId> {
            s.strip_prefix('@').and_then(|n| n.parse().ok()).map(BuildingId)
        };
        let rid = |s: &str| -> Option<RoadId> {
            s.strip_prefix('%').and_then(|n| n.parse().ok()).map(RoadId)
        };
        let tid = |s: &str| -> Option<TradeId> {
            s.strip_prefix('$').and_then(|n| n.parse().ok()).map(TradeId)
        };
        let pid = |s: &str| -> Option<PlayerId> {
            s.strip_prefix('!').and_then(|n| n.parse().ok()).map(PlayerId)
        };

        Some(match parts.as_slice() {
            ["place", k, x, y, f] => Command::Place {
                kind: parse_kind(k)?,
                facing: parse_facing(f)?,
                x: x.parse().ok()?,
                y: y.parse().ok()?,
            },
            ["demolish", b] => Command::Demolish { building: bid(b)? },
            ["raise", b] => Command::RaiseDike { dike: bid(b)? },
            ["upgrade", b] => Command::Upgrade { building: bid(b)? },
            ["shift", b, x, y] => Command::Move {
                building: bid(b)?,
                x: x.parse().ok()?,
                y: y.parse().ok()?,
            },
            ["assign", c, b] => Command::Assign { citizens: ids(c)?, building: bid(b)? },
            ["unassign", c] => Command::Unassign { citizens: ids(c)? },
            ["move", c, x, y] => {
                Command::MoveTo { citizens: ids(c)?, x: x.parse().ok()?, y: y.parse().ok()? }
            }
            ["home", c, b] => Command::SetHome { citizens: ids(c)?, cottage: bid(b)? },
            ["dikeline", fx, fy, tx, ty] => Command::DikeLine {
                from: (fx.parse().ok()?, fy.parse().ok()?),
                to: (tx.parse().ok()?, ty.parse().ok()?),
            },
            ["road", fx, fy, tx, ty] => Command::Road {
                from: (fx.parse().ok()?, fy.parse().ok()?),
                to: (tx.parse().ok()?, ty.parse().ok()?),
            },
            ["acceptroad", r] => Command::AcceptRoad { road: rid(r)? },
            ["trade", w, gg, ga, tg, ta] => Command::Trade {
                with: pid(w)?,
                give: (parse_good(gg)?, ga.parse().ok()?),
                take: (parse_good(tg)?, ta.parse().ok()?),
            },
            ["accepttrade", t] => Command::AcceptTrade { trade: tid(t)? },
            ["ping", x, y] => Command::Ping { x: x.parse().ok()?, y: y.parse().ok()? },
            ["pause"] => Command::Pause,
            ["resume"] => Command::Resume,
            ["drop", p] => Command::Drop { player: pid(p)? },
            _ => return None,
        })
    }
}

/// The canonical spelling of a building, for a transcript.
pub fn kind_key(k: Kind) -> &'static str {
    match k {
        Kind::Hearth => "hearth",
        Kind::Cottage => "cottage",
        Kind::Farm => "farm",
        Kind::Forester => "forester",
        Kind::Quarry => "quarry",
        Kind::Granary => "granary",
        Kind::Stockpile => "stockpile",
        Kind::TradingPost => "post",
        Kind::Nursery => "nursery",
        Kind::BuildersHut => "hut",
        Kind::Dike => "dike",
        Kind::Road => "road",
        Kind::Bridge => "bridge",
    }
}

pub fn parse_kind(s: &str) -> Option<Kind> {
    Kind::ALL.into_iter().find(|&k| kind_key(k) == s.to_ascii_lowercase())
}

/// The canonical spelling of a facing, for a transcript.
pub fn facing_key(f: Facing) -> &'static str {
    match f {
        Facing::EastWest => "ew",
        Facing::NorthSouth => "ns",
    }
}

pub fn parse_facing(s: &str) -> Option<Facing> {
    Facing::ALL.into_iter().find(|&f| facing_key(f) == s.to_ascii_lowercase())
}

pub fn good_key(g: Good) -> &'static str {
    match g {
        Good::Food => "food",
        Good::Wood => "wood",
        Good::Stone => "stone",
        Good::Gold => "gold",
    }
}

pub fn parse_good(s: &str) -> Option<Good> {
    Good::ALL.into_iter().find(|&g| good_key(g) == s.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant, for the round-trip test and for the determinism script.
    fn one_of_each() -> Vec<Command> {
        vec![
            Command::Place { kind: Kind::Cottage, facing: Facing::EastWest, x: 10, y: 20 },
            Command::Place { kind: Kind::Dike, facing: Facing::NorthSouth, x: 4, y: 9 },
            Command::Place { kind: Kind::Bridge, facing: Facing::EastWest, x: 0, y: 255 },
            Command::Demolish { building: BuildingId(7) },
            Command::RaiseDike { dike: BuildingId(3) },
            Command::Upgrade { building: BuildingId(5) },
            Command::Move { building: BuildingId(6), x: 40, y: 41 },
            Command::Assign { citizens: vec![CitizenId(1)], building: BuildingId(2) },
            Command::Assign {
                citizens: vec![CitizenId(0), CitizenId(9), CitizenId(65535)],
                building: BuildingId(0),
            },
            Command::Unassign { citizens: vec![CitizenId(4), CitizenId(5)] },
            Command::MoveTo { citizens: vec![CitizenId(8)], x: 127, y: 3 },
            Command::SetHome { citizens: vec![CitizenId(2)], cottage: BuildingId(11) },
            Command::DikeLine { from: (40, 12), to: (40, 60) },
            Command::Road { from: (5, 6), to: (100, 120) },
            Command::AcceptRoad { road: RoadId(2) },
            Command::Trade {
                with: PlayerId(1),
                give: (Good::Food, 20),
                take: (Good::Wood, 15),
            },
            Command::AcceptTrade { trade: TradeId(0) },
            Command::Ping { x: 64, y: 64 },
            Command::Pause,
            Command::Resume,
            Command::Drop { player: PlayerId(1) },
        ]
    }

    #[test]
    fn every_command_survives_being_written_down_and_read_back() {
        for c in one_of_each() {
            let line = c.line();
            assert_eq!(
                Command::parse(&line).as_ref(),
                Some(&c),
                "{line:?} did not come back as {c:?}"
            );
        }
    }

    #[test]
    fn the_sample_covers_every_variant() {
        // If a variant is added and not listed above, the determinism script
        // stops covering it silently. This is what notices.
        let seen: std::collections::BTreeSet<&str> = one_of_each()
            .iter()
            .map(|c| c.line().split_whitespace().next().unwrap_or("").to_owned())
            .map(|s| Box::leak(s.into_boxed_str()) as &str)
            .collect();
        assert_eq!(
            seen.len(),
            18,
            "one_of_each names {} distinct commands, not 18: {seen:?}",
            seen.len()
        );
    }

    #[test]
    fn nonsense_is_not_a_command() {
        for line in [
            "", "   ", "; just a comment", "fly to the moon", "place", "place nowhere 1 2 ew",
            "place cottage 1", "place cottage 1 2", "place cottage 1 2 up",
            "demolish 7", "demolish @", "move 1 2 3", "assign #1",
            "pause now", "resume please", "ping 1", "home #1 2",
            "road 1 2 3", "dikeline 1 2 3", "acceptroad 2", "trade !1 food 20 wood",
            "upgrade 5", "shift @6 40", "shift 6 40 41",
            "accepttrade 0",
        ] {
            assert_eq!(Command::parse(line), None, "{line:?} parsed as a command");
        }
    }

    #[test]
    fn a_comment_after_a_command_is_ignored() {
        assert_eq!(
            Command::parse("ping 5 6   ; look at this"),
            Some(Command::Ping { x: 5, y: 6 })
        );
    }

    #[test]
    fn every_facing_has_a_spelling_and_only_one() {
        let mut seen = std::collections::BTreeSet::new();
        for f in Facing::ALL {
            let key = facing_key(f);
            assert!(seen.insert(key), "{key} names two facings");
            assert_eq!(parse_facing(key), Some(f));
            assert_eq!(parse_facing(&key.to_uppercase()), Some(f));
        }
        assert_eq!(parse_facing("sideways"), None);
    }

    #[test]
    fn every_good_has_a_spelling_and_only_one() {
        let mut seen = std::collections::BTreeSet::new();
        for g in Good::ALL {
            let key = good_key(g);
            assert!(seen.insert(key), "{key} names two goods");
            assert_eq!(parse_good(key), Some(g));
            assert_eq!(parse_good(&key.to_uppercase()), Some(g));
        }
        assert_eq!(parse_good("silver"), None);
    }

    #[test]
    fn every_kind_has_a_spelling_and_only_one() {
        let mut seen = std::collections::BTreeSet::new();
        for k in Kind::ALL {
            let key = kind_key(k);
            assert!(seen.insert(key), "{key} names two kinds");
            assert_eq!(parse_kind(key), Some(k));
            assert_eq!(parse_kind(&key.to_uppercase()), Some(k), "case does not matter");
        }
        assert_eq!(parse_kind("castle"), None);
    }

    #[test]
    fn a_command_knows_which_citizens_it_speaks_for() {
        // The ownership check reads this, so a variant that carries citizens
        // and is missing from it would be a command that skips the check.
        for c in one_of_each() {
            let named = c.line().contains('#');
            assert_eq!(
                !c.citizens().is_empty(),
                named,
                "{c:?} names citizens in its transcript but not in citizens()"
            );
        }
    }
}
