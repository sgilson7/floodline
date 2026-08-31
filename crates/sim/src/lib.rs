//! The whole game, and nothing that draws or talks.
//!
//! Every peer runs this crate from the same seed and feeds it the same
//! commands on the same tick, so it must produce byte-identical results on a
//! laptop and in a browser. That is why the dependency list is two crates
//! long, why there is no `f32` anywhere below this line, and why the one
//! `Rng` lives in `World` rather than wherever it happens to be needed.
//! `tests/boundary.rs` and `tests/determinism.rs` are what keep it honest.

#![forbid(unsafe_code)]

pub mod age;
pub mod balance;
pub mod building;
pub mod citizen;
pub mod command;
pub mod crowd;
pub mod flood;
pub mod fx;
pub mod jobs;
pub mod map;
pub mod mule;
pub mod names;
pub mod nav;
pub mod rng;
pub mod road;
pub mod water;
pub mod world;

pub use building::{Building, BuildingId, Facing, Good, Goods, Kind};
pub use citizen::{Citizen, CitizenId, Job, PlayerId, State};
pub use age::{Disaster, Ending, Omen, Score};
pub use command::Command;
pub use mule::{Leg, Mule, MuleId};
pub use road::{Road, RoadId, Trade, TradeId};
pub use fx::{Fx, Turns, V2};
pub use map::{Corner, Ground, Map, MAP_H, MAP_W};
pub use rng::Rng;
pub use world::World;
