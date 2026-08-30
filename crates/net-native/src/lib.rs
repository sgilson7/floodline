//! `Peer` over `matchbox_socket`, configured to look identical on the wire to
//! what `quad_rtc.js` produces: two negotiated channels, ids 0 and 1, the
//! first ordered and the second `maxRetransmits: 0`.
//!
//! Native only, for design §9.1's reason: `matchbox_socket` is built on
//! web-sys and macroquad never runs wasm-bindgen.

#![forbid(unsafe_code)]
#![cfg(not(target_arch = "wasm32"))]
