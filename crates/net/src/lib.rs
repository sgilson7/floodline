//! The `Peer` trait, the star, the lockstep scheduler, and the wire format.
//!
//! Everything above the trait is transport-agnostic on purpose: the lockstep
//! cannot tell whether it is running on a data channel in a browser or on
//! `Loopback`, which is what lets the whole of it — desync detection, dropping
//! a silent player, a late joiner catching up from a snapshot — be tested in
//! `cargo test` with no browser and no network at all (design §9.6).

#![forbid(unsafe_code)]
