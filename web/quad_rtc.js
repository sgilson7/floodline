// The miniquad plugin. Phase 3 fills this in; today it does one real thing.
//
// Registered with `miniquad_add_plugin` before `load()`, which is what puts
// these functions in the wasm module's import object. The plugin exists this
// early on purpose: everything in phase 3 — the signaling socket, one
// RTCPeerConnection per peer, two data channels — crosses this same boundary,
// so the boundary is proven by the first deployment rather than first
// exercised under a WebRTC handshake.
//
// What phase 3 adds here, from matchbox_socket/src/webrtc_socket/wasm.rs:
//   * a WebSocket to <signaling>/<build>/<room>?next=<players>, speaking
//     matchbox_protocol's JSON: IdAssigned / NewPeer / PeerLeft / Signal in,
//     Signal / KeepAlive out;
//   * an offer when NewPeer arrives, an answer when an unknown sender's
//     Signal arrives. Arrival order decides who offers, NOT peer id — see
//     DECISIONS.md, the design note that says otherwise is wrong;
//   * two negotiated channels, ids 0 and 1, named matchbox_socket_0 and
//     matchbox_socket_1: {ordered: true} and {ordered: false,
//     maxRetransmits: 0}, binaryType "arraybuffer";
//   * ICE gathering waited out before the offer or answer is sent, because
//     matchbox found trickling broke NAT punching in practice;
//   * KeepAlive every 10 s, and an inbound queue drained by rtc_poll.

"use strict";

// Wrapped, because a classic <script> shares one global scope with
// sapp_jsutils.js and that file declares a top-level `register_plugin` too.
// It registers itself before this file is parsed, so the collision is
// currently harmless — but "currently" is doing all the work in that sentence,
// and load order is exactly the thing phase 3 will be tempted to change.
(function () {
  function register_plugin(importObject) {
    // The wasm's own sha256, stamped into the page by package-web.sh. The
    // binary cannot contain its own hash, so it asks for it. Hello carries
    // it (design 8) to refuse peers on a different build.
    importObject.env.fl_build_hash = function () {
      return js_object(window.FLOODLINE_BUILD || "");
    };

    // Rust's panic hook, and anything else worth seeing in devtools.
    importObject.env.fl_log = function (msg) {
      console.error(consume_js_object(msg));
    };
  }

  // `name` and `version` are not decoration: miniquad calls the wasm's
  // `quad_rtc_crate_version()` and console.errors if it disagrees with this
  // number. That is the guard that catches a deployed page whose JS plugin and
  // whose Rust side have drifted apart — the phase 3 failure that would
  // otherwise present as "the handshake just hangs".
  miniquad_add_plugin({ register_plugin, version: 1, name: "quad_rtc" });
})();
