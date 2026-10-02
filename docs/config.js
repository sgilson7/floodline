// Deployment settings, read by the page and never compiled in.
//
// Copied into dist/web/ as-is, so any of this can change without rebuilding
// the wasm — which matters because the build is stamped with its own hash and
// rebuilding it invalidates every room code in flight (design 9.4: the hash is
// part of the room name). Edit it here, or edit the copy in dist/web/.
window.FLOODLINE_CONFIG = {
  // Which vendored Trystero bundle to load, and only when somebody actually
  // hosts or joins by room code. Pinned by name and sha256 in
  // web/vendor/README.md and checked by packaging/package-web.sh.
  //
  // "nostr" is the default. Switch to "torrent" if the Nostr relays are slow
  // or blocked from where you are; both ends must agree, since they have to
  // meet on the same medium. Neither is ours and neither costs anything.
  strategy: "nostr",
  strategies: {
    nostr: "vendor/trystero-nostr-0.25.4.js",
    torrent: "vendor/trystero-torrent-0.25.4.js",
  },

  // Namespaces this game's rooms away from every other Trystero app sharing
  // the same public relays. The room name adds the build hash and the room
  // code; see design 9.4.
  appId: "floodline-p2p",

  // Encrypts the session descriptions as they cross a relay we do not run, so
  // an untrusted relay cannot tamper with a handshake. It is not a secret —
  // it ships in this file — and it is not protecting the game traffic, which
  // is DTLS-encrypted end to end regardless. Two peers must agree on it.
  password: "floodline",

  // ICE needs STUN to learn public addresses; Google's are free. When both
  // players are behind strict NATs there is no direct path and only a TURN
  // relay can carry the traffic — there is no serverless substitute for that
  // case. Adding one is one edit here and no rebuild, and it is the only thing
  // in this game that can cost money. A free tier entry looks like:
  //
  //   { urls: "turn:relay.example:3478", username: "u", credential: "p" }
  //
  rtcConfig: {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      { urls: "stun:global.stun.twilio.com:3478" },
    ],
  },

  // How long to wait for a relay to introduce anybody before the lobby offers
  // the pasted-code path instead (plan, phase 6).
  relayTimeoutMs: 15000,
};
