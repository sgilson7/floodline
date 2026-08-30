//! `Peer` over `web/quad_rtc.js`, the miniquad plugin: signalling through
//! Trystero or a pasted code, then one connection to the host (design 9).
//!
//! The transport itself is wasm32 only. `Roster` is not, deliberately: it is
//! the only bookkeeping in here that a laptop can check, and `cargo test` on a
//! laptop should be able to check something.
//!
//! Not `#![forbid(unsafe_code)]`, unlike `sim` and `net`. Every call into the
//! plugin is an `extern "C"` one and there is no way to make it otherwise;
//! forbidding it here would only mean the attribute moves to the module that
//! matters, which is worse than saying so once at the top.

mod roster;
pub use roster::Roster;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{Mode, WebPeer};
