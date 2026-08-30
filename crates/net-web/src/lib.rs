//! `Peer` over `web/quad_rtc.js`, the miniquad plugin: signalling through
//! trystero or a pasted code, then one connection to the host (design §9).
//!
//! wasm32 only. On any other target this crate is deliberately empty rather
//! than absent, so `cargo test --workspace` on a laptop still type-checks the
//! workspace it is a member of.

#![forbid(unsafe_code)]
#![cfg(target_arch = "wasm32")]
