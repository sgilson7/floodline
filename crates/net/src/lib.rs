//! The `Peer` trait, the lockstep scheduler, and the wire format.
//!
//! Everything above the trait is transport-agnostic on purpose: the lockstep
//! cannot tell whether it is running on a data channel in a browser or on
//! `matchbox_socket` on a laptop, which is what lets a headless bot stand in
//! for a second player in a test.

#![forbid(unsafe_code)]
