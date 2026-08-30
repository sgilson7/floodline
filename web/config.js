// Deployment settings, read by the page and never compiled in.
//
// This file is copied into dist/web/ as-is, so the signaling server can move
// without rebuilding the wasm — which matters because the Pages build is
// stamped with its own hash and rebuilding it invalidates every room code in
// flight. Edit it here, or edit the copy in dist/web/ on a server.
window.FLOODLINE_CONFIG = {
  // ws:// for `make signal`, wss:// for the deployed one. A `?signal=` query
  // parameter overrides this, which is how you test a branch server without
  // touching the file.
  signaling: "ws://localhost:3536",

  // Full-mesh WebRTC connects directly for most home networks. For the ones
  // it does not, a TURN entry goes here — coturn on your own box, or a
  // metered.ca free tier. Design 9.4: budget for this in the config, not in
  // the code, so adding it is one edit and no rebuild.
  ice: [
    { urls: "stun:stun.l.google.com:19302" },
  ],
};
