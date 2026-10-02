# Vendored, pinned, and not fetched at build time

Two Trystero strategy bundles. `packaging/package-web.sh` checks their sha256
before it will package a build, and `DECISIONS.md` records where they came from
and why these two.

| file | version | sha256 |
|---|---|---|
| `trystero-nostr-0.25.4.js` | 0.25.4 | `6bfce15d72a64384cc66c2693917994e1b900f4f98c9b7a2d54e4e86f5202906` |
| `trystero-torrent-0.25.4.js` | 0.25.4 | `93ed42a50b03b0deaf6d3ee278971416e1f28e5f3bc3bd50233da0ba152558f0` |

Each is a self-contained ES module — no imports, nothing fetched at load time
— produced by esm.sh's build service from the published npm packages:

    curl -o trystero-nostr-0.25.4.js \
      https://esm.sh/@trystero-p2p/nostr@0.25.4/es2022/nostr.bundle.mjs
    curl -o trystero-torrent-0.25.4.js \
      https://esm.sh/@trystero-p2p/torrent@0.25.4/es2022/torrent.bundle.mjs

To update: fetch a new version, put the new hashes in this table and in
`packaging/package-web.sh`, name the new file in `web/config.js`, and check
that `joinRoom`, `selfId` and `getPeers()` still mean what `web/quad_rtc.js`
thinks they mean. Nothing else in the repo knows about Trystero.
