#!/usr/bin/env bash
# Build the browser version into dist/web/.
#
# No node, no npm, no bundler. macroquad boots the wasm through its own
# mq_js_bundle.js and never runs wasm-bindgen (design 9.1), so the whole job is
# one cargo build, three JS files copied out of pinned crate sources, and one
# hash stamped into the page.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"; WEB="$DIST/web"
CRATE=gui
WASM=gui           # cargo's output name for the gui binary
OUT=floodline      # what it is called on the web, regardless of the crate
TARGET=wasm32-unknown-unknown

say() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# `shasum` is BSD, `sha256sum` is what the Linux CI runner has. Keep both
# rather than working only on the machine this was written on.
sha256() {
  if command -v shasum >/dev/null; then shasum -a 256; else sha256sum; fi
}
# Literal string replacement, no regex metacharacters to escape.
bust() { S="$2" R="$3" perl -0777 -pi -e 's/\Q$ENV{S}\E/$ENV{R}/g' "$1"; }

rustup target list --installed 2>/dev/null | grep -q "^$TARGET$" \
  || die "missing target. Run: rustup target add $TARGET"

say "Building $CRATE for $TARGET"
cargo build --release --target "$TARGET" -p "$CRATE"

# Both JS shims ship inside the crates that need them, so take them from the
# exact versions the lockfile pinned rather than from a CDN: a loader that
# disagrees with the wasm it is loading fails in ways that look like game bugs.
pinned() { # crate name -> version
  awk -v n="$1" '$0 == "name = \"" n "\"" {f=1} f && /^version = /{gsub(/"/,"");print $3;exit}' \
    "$ROOT/Cargo.lock"
}
REGISTRY=$(find "${CARGO_HOME:-$HOME/.cargo}/registry/src" -maxdepth 1 -type d -name 'index.crates.io-*' | head -1)
[ -n "$REGISTRY" ] || die "no crates.io registry source dir. Run \`cargo fetch\`."

MQ_VERSION=$(pinned macroquad)
JS_VERSION=$(pinned sapp-jsutils)
MQ_BUNDLE="$REGISTRY/macroquad-$MQ_VERSION/js/mq_js_bundle.js"
JS_UTILS="$REGISTRY/sapp-jsutils-$JS_VERSION/js/sapp_jsutils.js"
[ -f "$MQ_BUNDLE" ] || die "no mq_js_bundle.js for macroquad $MQ_VERSION at $MQ_BUNDLE"
[ -f "$JS_UTILS" ]  || die "no sapp_jsutils.js for sapp-jsutils $JS_VERSION at $JS_UTILS"

# Trystero is not a crate and cannot be pinned by the lockfile, so it is pinned
# by hand: the file is committed under web/vendor/ and its hash is checked here
# before anything ships. The same rule the two JS shims above get, applied to
# the one dependency that arrived by curl. Updating it means a new file, a new
# hash on this line, and a new row in web/vendor/README.md.
check_pin() { # file, sha256
  [ -f "$1" ] || die "missing vendored file $1 — see web/vendor/README.md"
  have=$(sha256 < "$1" | cut -d' ' -f1)
  [ "$have" = "$2" ] || die "$1 is not the pinned copy
    expected $2
    found    $have
  Either it was edited (do not edit vendored files) or it was replaced without
  updating the pin here and in web/vendor/README.md."
}
check_pin "$ROOT/web/vendor/trystero-nostr-0.25.4.js" \
  6bfce15d72a64384cc66c2693917994e1b900f4f98c9b7a2d54e4e86f5202906
check_pin "$ROOT/web/vendor/trystero-torrent-0.25.4.js" \
  93ed42a50b03b0deaf6d3ee278971416e1f28e5f3bc3bd50233da0ba152558f0

# A vendored bundle that imports anything is a bundle that phones home at load
# time, which would mean the game stops working the day somebody else's CDN
# does. Both were checked for this when they were vendored; check it again,
# because it is one grep and the failure is silent and total.
for f in "$ROOT"/web/vendor/*.js; do
  if grep -qE '(^|[^A-Za-z_$.])(import|from) *["(]' "$f"; then
    die "$(basename "$f") has an import in it — vendored bundles must be self-contained"
  fi
done

say "Assembling $WEB"
rm -rf "$WEB"; mkdir -p "$WEB"
cp -R "$ROOT/web/." "$WEB/"
cp "$ROOT/target/$TARGET/release/$WASM.wasm" "$WEB/$OUT.wasm"
cp "$MQ_BUNDLE" "$WEB/mq_js_bundle.js"
cp "$JS_UTILS"  "$WEB/sapp_jsutils.js"

# --- cache busting ----------------------------------------------------------
# A browser will hold a multi-megabyte wasm for as long as it is allowed to,
# and GitHub Pages serves everything with `Cache-Control: max-age=600` — so for
# ten minutes after a deploy, a reload keeps running the old game and reads as
# "the fix did not deploy". Stamping the binary's own hash into its URL means a
# changed build is simply a different URL. It is a hash of the wasm rather than
# a timestamp so it changes exactly when the game does, which is what keeps
# `make publish` honest about "docs/ unchanged".
#
# The same value is the build id in design 8's `Hello`, so two peers on
# different builds are refused instead of desyncing ten minutes later.
say "Stamping build id"
BUILD=$(sha256 < "$WEB/$OUT.wasm" | cut -c1-12)
bust "$WEB/index.html" "load(\"$OUT.wasm\")" "load(\"$OUT.wasm?v=$BUILD\")"
bust "$WEB/index.html" '__BUILD__'           "$BUILD"
# echo.html is not linked from anywhere and is not the game (design 9.6), but
# it has to be stamped too or it hosts its rooms under the name "__BUILD__".
bust "$WEB/echo.html"  '__BUILD__'           "$BUILD"

# Fail loudly rather than shipping a page that silently serves a stale wasm or
# tells every peer its build is called "__BUILD__".
grep -q "$OUT.wasm?v=$BUILD" "$WEB/index.html" || die "wasm URL not stamped"
grep -q "FLOODLINE_BUILD = \"$BUILD\"" "$WEB/index.html" || die "build id not stamped"
if grep -q '__BUILD__' "$WEB/index.html"; then die "a __BUILD__ placeholder survived"; fi

# Jekyll would otherwise skip files it thinks are private and mangle the rest.
touch "$WEB/.nojekyll"

say "Done: $WEB (build $BUILD)"
du -sh "$WEB" | sed 's/^/    /'
ls -l "$WEB/$OUT.wasm" | awk '{printf "    wasm: %.0f KB\n", $5/1024}'
