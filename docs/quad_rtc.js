// The miniquad plugin: signalling, one connection per peer, two data channels.
//
// Registered with `miniquad_add_plugin` before `load()`, which is what puts
// these functions in the wasm module's import object. Design 9.2 names the
// imports; DECISIONS.md, under "The handshake, written down before the
// plugin", is the sequence this file implements and the place any argument
// with it belongs.
//
// The shape of it, in one paragraph. Two paths reach the same object — one
// `RTCPeerConnection` to the host with a reliable and an unreliable channel —
// so `net-web` above cannot tell them apart. Trystero introduces peers over
// public relays and hands back a connection; the pasted-code path builds the
// connection here out of one offer and one answer. From the channels down the
// paths are identical: both ends create the same two *negotiated* channels on
// stream ids 40 and 41, so there is no `ondatachannel` event and therefore no
// race about who was listening when. The first byte on the reliable channel
// says whether the sender is a host or a joiner, and that is what makes the
// star an invariant rather than a guess — Trystero rooms are meshes, and two
// joiners will meet.

"use strict";

// Wrapped, because a classic <script> shares one global scope with
// sapp_jsutils.js and that file declares a top-level `register_plugin` too.
// It registers itself before this file is parsed, so the collision is
// currently harmless — but "currently" is doing all the work in that sentence.
(function () {
  // Out-of-band channels: both ends name these ids and the channels open with
  // no negotiation and no event to miss. Not 0 and 1, because Trystero opens
  // its own in-band channel and in-band ids are handed out from zero upwards.
  var RELIABLE_ID = 40;
  var UNRELIABLE_ID = 41;

  // The first byte on the reliable channel. One byte per connection buys the
  // star; see DECISIONS.md.
  var ROLE_HOST = 0x48; // 'H'
  var ROLE_JOINER = 0x4a; // 'J'

  // What rtc_poll hands back. Integers rather than design 9.2's strings
  // because this field is read on every event, sixty times a second per peer.
  var K_PEER = 0, K_LEFT = 1, K_MSG = 2, K_ERROR = 3;

  var MODE_TRYSTERO = 0, MODE_CODE = 1;

  // Long enough for STUN from a slow network, short enough that a blocked
  // STUN server does not look like a hang. Host candidates alone still
  // connect two machines on the same LAN.
  var GATHER_TIMEOUT_MS = 6000;

  // The one session this page has, or null.
  var S = null;

  // What the Copy button would put on the clipboard, or null when the cursor
  // is not over one.
  //
  // It lives here, and the copying happens in the canvas's own click handler,
  // because *when* matters: a browser only lets a page write to the clipboard
  // while a user gesture is still live, and macroquad reads a click in the
  // animation frame after the browser delivered it. By then the gesture has
  // expired, so `navigator.clipboard.writeText` was rejected — and its
  // rejection arrives as a failed promise, which `try`/`catch` cannot see, so
  // the old fallback never ran either and the button did nothing and said
  // nothing. Rust arms this while the cursor is over the button; the listener
  // below fires inside the real click, where writing is allowed.
  var COPY_ARMED = null;
  // Which session that is. Rust hands it back when it closes one, so a stale
  // handle cannot close a live session — see `close(gen)`.
  var GENERATION = 0;

  function cfg() {
    return window.FLOODLINE_CONFIG || {};
  }

  function push(s, ev) {
    if (s) s.queue.push(ev);
  }

  // `code` says whether a pasted code would get round this. Signalling that
  // never answered: yes, that is exactly what the pasted path is for. A
  // connection that could not be opened once the two ends had found each
  // other: *no* — the pasted path needs the same direct link and will fail in
  // the same place. Telling somebody to try it then sends them round a loop.
  function fail(s, text, code) {
    push(s, { k: K_ERROR, text: text, code: code ? 1 : 0 });
  }

  // ---- links ---------------------------------------------------------------

  // Give a connection our two channels and start listening. Called at exactly
  // one place per path, so the two paths cannot drift apart.
  function attach(s, pc, id, mine) {
    var link = {
      id: id,
      pc: pc,
      // Whether the plugin built this connection or Trystero handed it over.
      // It decides who is allowed to close it: calling `pc.close()` on one of
      // Trystero's makes Trystero log "User-Initiated Abort" at the *other*
      // end, which is a real error message about nothing and exactly the kind
      // of console noise phase 4 cannot afford.
      mine: mine,
      rel: null,
      unrel: null,
      role: 0,
      reported: false,
      gone: false,
    };
    var rel = pc.createDataChannel("floodline-r", {
      negotiated: true,
      id: RELIABLE_ID,
      ordered: true,
    });
    var unrel = pc.createDataChannel("floodline-u", {
      negotiated: true,
      id: UNRELIABLE_ID,
      ordered: false,
      maxRetransmits: 0,
    });
    rel.binaryType = "arraybuffer";
    unrel.binaryType = "arraybuffer";
    link.rel = rel;
    link.unrel = unrel;

    rel.onopen = function () {
      try {
        rel.send(new Uint8Array([s.isHost ? ROLE_HOST : ROLE_JOINER]));
      } catch (e) {
        fail(s, "could not greet a peer: " + e, false);
      }
    };
    rel.onmessage = function (e) {
      reliableMessage(s, link, e.data);
    };
    unrel.onmessage = function (e) {
      // Nothing on the unreliable channel is worth reading before the roles
      // are known: it may be another joiner's, and it may arrive first.
      if (link.reported) {
        push(s, {
          k: K_MSG,
          id: link.id,
          reliable: 0,
          bytes: new Uint8Array(e.data),
        });
      }
    };
    rel.onclose = function () {
      depart(s, link);
    };
    unrel.onclose = function () {
      depart(s, link);
    };
    pc.addEventListener("connectionstatechange", function () {
      // A closed tab has no signalling channel left to say goodbye on, so
      // this is how the pasted-code path learns anybody left.
      if (pc.connectionState === "failed" || pc.connectionState === "closed") {
        depart(s, link);
      }
    });

    s.links.set(id, link);
    return link;
  }

  function reliableMessage(s, link, data) {
    var bytes = new Uint8Array(data);
    if (link.role === 0) {
      link.role = bytes[0];
      if (s.isHost) {
        // Two hosts in one room. Nothing sensible to do with the other one.
        if (link.role !== ROLE_JOINER) {
          disown(s, link);
          return;
        }
      } else if (link.role === ROLE_JOINER) {
        // Another joiner. Trystero put us in the same room; the star says we
        // have nothing to say to each other. Close our own two channels and
        // leave Trystero's connection alone — that one is Trystero's.
        disown(s, link);
        return;
      } else if (s.hostLink) {
        // A second host offering itself. The first one is the game.
        disown(s, link);
        return;
      } else {
        s.hostLink = link;
      }
      link.reported = true;
      push(s, { k: K_PEER, id: link.id });
      return;
    }
    push(s, { k: K_MSG, id: link.id, reliable: 1, bytes: bytes });
  }

  // Shut down our end of a link. The two channels are always ours to close;
  // the connection under them is only ours if we made it.
  function hangUp(link) {
    try {
      link.rel.close();
      link.unrel.close();
      if (link.mine) link.pc.close();
    } catch (e) {
      /* already closing */
    }
  }

  // A link that turned out not to be ours. Never reported, so never departs.
  function disown(s, link) {
    s.links.delete(link.id);
    link.gone = true;
    hangUp(link);
  }

  function depart(s, link) {
    if (link.gone) return;
    link.gone = true;
    s.links.delete(link.id);
    if (link.reported) push(s, { k: K_LEFT, id: link.id });
  }

  // ---- trystero ------------------------------------------------------------

  function loadStrategy() {
    var c = cfg();
    var file = (c.strategies || {})[c.strategy];
    if (!file) {
      return Promise.reject(
        new Error('config.js names no bundle for strategy "' + c.strategy + '"')
      );
    }
    return import(new URL(file, document.baseURI).href);
  }

  function startTrystero(s) {
    loadStrategy().then(
      function (lib) {
        if (s.closed) return;
        var c = cfg();
        // The build hash is in the room name (design 9.4) so a stale tab
        // cannot find a newer build's game. A hyphen rather than the design's
        // slash: some strategies treat a room name as a path.
        var name = (window.FLOODLINE_BUILD || "dev") + "-" + s.room;
        var room;
        try {
          room = lib.joinRoom(
            { appId: c.appId, password: c.password, rtcConfig: c.rtcConfig },
            name,
            {
              onJoinError: function (d) {
                // Trystero raises this for three different things and they
                // want three different answers.
                if (d && d.peerId) {
                  fail(
                    s,
                    "found the other player, but could not open a direct " +
                      "connection to them. On one network that usually means the " +
                      "router keeps its own clients apart; across the internet it " +
                      "means a strict NAT. A pasted code will not help - it needs " +
                      "the same direct link. A TURN server in config.js is the fix.",
                    false
                  );
                } else {
                  fail(
                    s,
                    "could not join the room: " +
                      (d && d.error ? d.error : "the relays did not answer"),
                    true
                  );
                }
              },
            }
          );
        } catch (e) {
          fail(s, "could not join the room: " + e, true);
          return;
        }
        if (s.closed) {
          room.leave();
          return;
        }
        s.trystero = room;

        room.onPeerJoin = function (pid) {
          var pc = room.getPeers()[pid];
          if (!pc) {
            fail(s, "a peer arrived with no connection", false);
            return;
          }
          var link = attach(s, pc, s.nextId++, false);
          s.byRelay.set(pid, link);
        };
        room.onPeerLeave = function (pid) {
          var link = s.byRelay.get(pid);
          s.byRelay.delete(pid);
          if (link) depart(s, link);
        };

        // Phase 6: if no relay ever answers, say so instead of waiting.
        var wait = c.relayTimeoutMs || 15000;
        setTimeout(function () {
          if (s.closed || s.links.size > 0) return;
          var open = 0;
          try {
            var sockets = lib.getRelaySockets();
            for (var k in sockets) {
              if (sockets[k] && sockets[k].readyState === 1) open++;
            }
          } catch (e) {
            /* the strategy may not expose them */
          }
          if (open === 0) {
            fail(
              s,
              "no signalling relay answered in " +
                Math.round(wait / 1000) +
                "s - they may be blocked from this network. A pasted code needs " +
                "none of them.",
              true
            );
          }
        }, wait);
      },
      function (e) {
        // The detail goes to the console and the advice goes on screen. A
        // player cannot act on a module specifier and the lobby has one line.
        console.error("floodline: the signalling bundle would not load", e);
        fail(
          s,
          "the signalling library did not load - this page may be incompletely " +
            "deployed. Hosting by pasted code needs nothing but this tab.",
          true
        );
      }
    );
  }

  // ---- the pasted code -----------------------------------------------------

  function gathered(pc) {
    if (pc.iceGatheringState === "complete") return Promise.resolve();
    return new Promise(function (resolve) {
      var done = false;
      function finish() {
        if (done) return;
        done = true;
        pc.removeEventListener("icegatheringstatechange", check);
        resolve();
      }
      function check() {
        if (pc.iceGatheringState === "complete") finish();
      }
      pc.addEventListener("icegatheringstatechange", check);
      // No trickle (design 9.2): the blob is the only thing that crosses, so
      // a candidate that arrives after it is a candidate that is lost. But a
      // STUN server that never answers must not mean a lobby that never
      // shows a code.
      setTimeout(finish, GATHER_TIMEOUT_MS);
    });
  }

  function makeOffer(s) {
    var pc = new RTCPeerConnection(cfg().rtcConfig || {});
    // Before createOffer, so the offer has an m=application section in it.
    var link = attach(s, pc, s.nextId++, true);
    s.pending = link;
    return pc
      .createOffer()
      .then(function (o) {
        return pc.setLocalDescription(o);
      })
      .then(function () {
        return gathered(pc);
      })
      .then(function () {
        return pack("O", pc.localDescription.sdp);
      })
      .then(function (blob) {
        if (s.pending === link) s.localBlob = blob;
      });
  }

  function takeOffer(s, blob) {
    var sdp = null;
    return unpack("O", blob)
      .then(function (text) {
        sdp = text;
        var pc = new RTCPeerConnection(cfg().rtcConfig || {});
        var link = attach(s, pc, s.nextId++, true);
        s.pending = link;
        return pc
          .setRemoteDescription({ type: "offer", sdp: sdp })
          .then(function () {
            return pc.createAnswer();
          })
          .then(function (a) {
            return pc.setLocalDescription(a);
          })
          .then(function () {
            return gathered(pc);
          })
          .then(function () {
            return pack("A", pc.localDescription.sdp);
          })
          .then(function (out) {
            s.localBlob = out;
          });
      });
  }

  function takeAnswer(s, blob) {
    var link = s.pending;
    if (!link) return Promise.reject(new Error("nobody is waiting for a reply"));
    return unpack("A", blob).then(function (sdp) {
      return link.pc
        .setRemoteDescription({ type: "answer", sdp: sdp })
        .then(function () {
          s.pending = null;
          s.localBlob = null;
          // The next joiner's invitation, gathering while this one connects.
          // One paste per joiner is what the star bought.
          return makeOffer(s);
        });
    });
  }

  function codeRemote(s, blob) {
    if (!blob) return Promise.reject(new Error("nothing was pasted"));
    var kind = blob[0];
    if (s.isHost) {
      if (kind === "O") {
        return Promise.reject(
          new Error("that is an invitation, not a reply - you are the host; paste what the other player sent back")
        );
      }
      return takeAnswer(s, blob);
    }
    if (kind === "A") {
      return Promise.reject(
        new Error("that is a reply, not an invitation - paste the code the host gave you")
      );
    }
    return takeOffer(s, blob);
  }

  // ---- the blob ------------------------------------------------------------
  //
  // A data-channel-only SDP is nearly all boilerplate. Only eight kinds of
  // line say anything the other end cannot infer, so the blob carries those
  // and `grow` rebuilds a valid session description around them — valid, not
  // identical: the two ends may be different browsers, and what has to survive
  // is the meaning.

  function shrink(sdp) {
    var keep = [];
    var lines = sdp.split(/\r\n|\n/);
    for (var i = 0; i < lines.length; i++) {
      var l = lines[i];
      if (l.indexOf("a=ice-ufrag:") === 0) keep.push("u" + l.slice(12));
      else if (l.indexOf("a=ice-pwd:") === 0) keep.push("p" + l.slice(10));
      else if (l.indexOf("a=fingerprint:") === 0) keep.push("f" + l.slice(14));
      else if (l.indexOf("a=setup:") === 0) keep.push("s" + l.slice(8));
      else if (l.indexOf("a=mid:") === 0) keep.push("m" + l.slice(6));
      else if (l.indexOf("a=sctp-port:") === 0) keep.push("P" + l.slice(12));
      else if (l.indexOf("a=max-message-size:") === 0) keep.push("M" + l.slice(19));
      else if (l.indexOf("a=candidate:") === 0) {
        // UDP only. Chrome offers TCP host candidates that are no use to
        // anybody without a TCP relay on the other side, and they are a third
        // of the candidate lines.
        if (/ (udp|UDP) /.test(l)) keep.push("c" + l.slice(12));
      }
    }
    return keep.join("\n");
  }

  function grow(min) {
    var ufrag = "", pwd = "", fp = "", setup = "actpass", mid = "0";
    var port = "5000", mms = "262144";
    var cands = [];
    var lines = min.split("\n");
    for (var i = 0; i < lines.length; i++) {
      var t = lines[i][0], v = lines[i].slice(1);
      if (t === "u") ufrag = v;
      else if (t === "p") pwd = v;
      else if (t === "f") fp = v;
      else if (t === "s") setup = v;
      else if (t === "m") mid = v;
      else if (t === "P") port = v;
      else if (t === "M") mms = v;
      else if (t === "c") cands.push("a=candidate:" + v);
    }
    return [
      "v=0",
      "o=- 0 0 IN IP4 127.0.0.1",
      "s=-",
      "t=0 0",
      "a=group:BUNDLE " + mid,
      "a=msid-semantic: WMS",
      "m=application 9 UDP/DTLS/SCTP webrtc-datachannel",
      "c=IN IP4 0.0.0.0",
    ]
      .concat(cands)
      .concat([
        "a=ice-ufrag:" + ufrag,
        "a=ice-pwd:" + pwd,
        "a=ice-options:trickle",
        "a=fingerprint:" + fp,
        "a=setup:" + setup,
        "a=mid:" + mid,
        "a=sctp-port:" + port,
        "a=max-message-size:" + mms,
        "",
      ])
      .join("\r\n");
  }

  function b64url(bytes) {
    var s = "";
    for (var i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  function unb64url(text) {
    var s = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
    var out = new Uint8Array(s.length);
    for (var i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
  }

  function through(stream, bytes) {
    var w = stream.writable.getWriter();
    w.write(bytes);
    w.close();
    return new Response(stream.readable).arrayBuffer();
  }

  // "O" for an invitation, "A" for a reply. The letter is outside the
  // compressed part so a blob pasted into the wrong box is refused with a
  // sentence rather than a decode error.
  function pack(kind, sdp) {
    var raw = new TextEncoder().encode(shrink(sdp));
    return through(new CompressionStream("deflate-raw"), raw).then(function (buf) {
      return kind + b64url(new Uint8Array(buf));
    });
  }

  function unpack(kind, blob) {
    var body = blob.slice(1).replace(/\s+/g, "");
    return through(new DecompressionStream("deflate-raw"), unb64url(body)).then(
      function (buf) {
        return grow(new TextDecoder().decode(buf));
      }
    );
  }

  // ---- the session ---------------------------------------------------------

  function open(isHost, mode, room) {
    close();
    GENERATION += 1;
    S = {
      gen: GENERATION,
      isHost: isHost,
      mode: mode,
      room: room || "",
      queue: [],
      links: new Map(),
      byRelay: new Map(),
      nextId: 1,
      trystero: null,
      pending: null,
      localBlob: null,
      hostLink: null,
      closed: false,
    };
    var s = S;
    if (mode === MODE_TRYSTERO) {
      startTrystero(s);
    } else if (isHost) {
      makeOffer(s).catch(function (e) {
        fail(s, "could not make an invitation: " + e, false);
      });
    }
    return s;
  }

  // `gen` is which session the caller means. Omitted, it means "whatever is
  // open"; given, it closes only that one.
  //
  // The generation is not defensive programming, it is required. Rust closes
  // the transport when it drops a session, and it drops the *old* session
  // after constructing the new one — so hosting a second game would open a
  // room and then immediately have the previous session's teardown close it.
  // Which is the bug this whole change exists to fix, wearing a hat.
  function close(gen) {
    if (!S) return;
    if (gen !== undefined && gen !== S.gen) return;
    var s = S;
    S = null;
    s.closed = true;
    if (s.trystero) {
      try {
        s.trystero.leave();
      } catch (e) {
        /* already gone */
      }
    }
    s.links.forEach(hangUp);
    s.links.clear();
  }

  function send(id, reliable, bytes) {
    if (!S) return false;
    var link = S.links.get(id);
    if (!link || !link.reported) return false;
    var ch = reliable ? link.rel : link.unrel;
    if (!ch || ch.readyState !== "open") return false;
    try {
      ch.send(bytes);
      return true;
    } catch (e) {
      fail(S, "could not send to a peer: " + e, false);
      return false;
    }
  }

  // Copy in the gesture, not in the frame after it. See `COPY_ARMED`.
  window.addEventListener("DOMContentLoaded", function () {
    var canvas = document.getElementById("glcanvas");
    if (!canvas) return;
    canvas.addEventListener("click", function (e) {
      var armed = COPY_ARMED;
      if (!armed) return;
      // Inside the button Rust drew, in the page's own pixels.
      if (e.offsetX < armed.x || e.offsetX > armed.x + armed.w ||
          e.offsetY < armed.y || e.offsetY > armed.y + armed.h) {
        return;
      }
      var text = armed.text;
      var wrote = false;
      try {
        var box = document.createElement("textarea");
        box.value = text;
        box.setAttribute("readonly", "");
        box.style.position = "fixed";
        box.style.top = "0";
        box.style.opacity = "0";
        document.body.appendChild(box);
        box.focus();
        box.select();
        box.setSelectionRange(0, text.length);
        wrote = document.execCommand("copy");
        document.body.removeChild(box);
        canvas.focus();
      } catch (e) {
        /* fall through to the async one */
      }
      if (!wrote && navigator.clipboard) {
        navigator.clipboard.writeText(text).catch(function (e) {
          console.error("floodline: could not copy - press ctrl-C instead. " + e);
        });
      }
    });
  });

  // A tab that is going away says so, in the only way there is: closing the
  // connections, which puts an SCTP shutdown on the wire and fires `onclose`
  // at the other end within milliseconds. Without this the other end waits out
  // ICE consent freshness instead — measured at sixteen seconds in headless
  // Chromium, which is inside the lockstep's thirty-second patience but well
  // outside the ten the plan asks for. The slow path is still there and is
  // still the one that matters for a crash, a killed process or a yanked
  // cable, none of which get to run any code at all.
  //
  // `pagehide` rather than `unload`: `unload` is not fired for a page that
  // went into the back/forward cache, and Chrome has been narrowing when it
  // fires it at all.
  window.addEventListener("pagehide", function () {
    close();
  });

  // What the wasm sees, and what `web/echo.html` drives directly. The same
  // functions either way, so the echo page is testing the thing that ships and
  // not a copy of it — design 9.6's whole point, that a networking bug and a
  // lockstep bug can never be confused for one another.
  window.FLOODLINE_RTC = {
    host: function (room, mode) {
      return open(true, mode, room);
    },
    join: function (room, mode) {
      return open(false, mode, room);
    },
    poll: function () {
      return S && S.queue.length ? S.queue.shift() : null;
    },
    send: send,
    codeLocal: function () {
      return S ? S.localBlob : null;
    },
    codeRemote: function (blob) {
      if (!S) return false;
      var s = S;
      codeRemote(s, String(blob).trim()).catch(function (e) {
        fail(s, String(e && e.message ? e.message : e), false);
      });
      return true;
    },
    close: close,
    // For echo.html's measurements only. Nothing in the game reads these.
    debug: function () {
      return S;
    },
  };

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

    // The lobby's three questions about the page it is running in. Not part
    // of the transport, but they are the only other things Rust cannot know
    // without asking the browser, and a second plugin for three functions
    // would be a second thing to keep in step with the wasm.
    importObject.env.fl_url_room = function () {
      var room = "";
      try {
        room = new URLSearchParams(location.search).get("room") || "";
      } catch (e) {
        /* no URL to speak of */
      }
      return js_object(room);
    };

    importObject.env.fl_share_link = function (room) {
      var code = consume_js_object(room);
      return js_object(
        location.origin + location.pathname + "?room=" + encodeURIComponent(code)
      );
    };

    // Copying has to happen while the click that asked for it still counts as
    // a user gesture. macroquad handles input inside its animation frame,
    // milliseconds after the click, which is inside Chrome's five-second
    // transient activation window — but not inside Safari's stricter one, so
    // the old execCommand path stays as the fallback rather than as history.
    // Arm or disarm the Copy button. An empty string disarms.
    importObject.env.fl_arm_copy = function (text, x, y, w, h) {
      var s = consume_js_object(text);
      COPY_ARMED = s && s.length ? { text: s, x: x, y: y, w: w, h: h } : null;
    };

    importObject.env.rtc_host = function (room, mode) {
      return open(true, mode, consume_js_object(room)).gen;
    };
    importObject.env.rtc_join = function (room, mode) {
      return open(false, mode, consume_js_object(room)).gen;
    };
    importObject.env.rtc_close = function (gen) {
      close(gen);
    };
    importObject.env.rtc_poll = function () {
      return js_object(S && S.queue.length ? S.queue.shift() : null);
    };
    importObject.env.rtc_send = function (id, reliable, bytes) {
      send(id, reliable, consume_js_object(bytes));
    };
    importObject.env.rtc_code_local = function () {
      return js_object(S && S.localBlob ? S.localBlob : null);
    };
    importObject.env.rtc_code_remote = function (blob) {
      return window.FLOODLINE_RTC.codeRemote(consume_js_object(blob)) ? 1 : 0;
    };
  }

  // `name` and `version` are not decoration: miniquad calls the wasm's
  // `quad_rtc_crate_version()` and console.errors if it disagrees with this
  // number. That is the guard that catches a deployed page whose JS plugin and
  // whose Rust side have drifted apart — the failure that would otherwise
  // present as "the handshake just hangs". Five since `fl_copy` became
  // `fl_arm_copy`.
  miniquad_add_plugin({ register_plugin: register_plugin, version: 5, name: "quad_rtc" });
})();
