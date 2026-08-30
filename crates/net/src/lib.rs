//! The `Peer` trait, the star, the lockstep scheduler, and the wire format.
//!
//! Everything above the trait is transport-agnostic on purpose: the lockstep
//! cannot tell whether it is running on a data channel in a browser or on
//! `Loopback`, which is what lets the whole of it — desync detection, dropping
//! a silent player, a late joiner catching up from a snapshot — be tested in
//! `cargo test` with no browser and no network at all (design §9.6).

#![forbid(unsafe_code)]

pub mod lockstep;
pub mod loopback;
pub mod wire;
pub mod peer;

pub use loopback::{Conditions, Loopback, HOST};
pub use peer::{Event, Peer, PeerId};
pub use lockstep::{Lockstep, Status, DELAY};
pub use wire::{Message, Refusal, PROTO_VERSION};
