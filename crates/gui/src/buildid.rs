//! Which build this is.
//!
//! In the browser the answer is the wasm's own sha256, which the binary
//! cannot contain, so `packaging/package-web.sh` stamps it into the page and
//! `web/quad_rtc.js` hands it back through `sapp-jsutils`. Doing that in
//! phase 0 is deliberate: §8 needs this hash inside `Hello` to refuse
//! mismatched peers, so it has to cross the JS boundary eventually, and
//! crossing it now means the bridge all of phase 3 rests on is proven by the
//! first deployment instead of first exercised under a WebRTC handshake.
//!
//! Natively there is no page, so the answer is the short git hash compiled
//! in, or `dev` outside a checkout.

#[cfg(target_arch = "wasm32")]
mod imp {
    use sapp_jsutils::JsObject;

    /// miniquad calls this on start-up and compares it with the `version` in
    /// `web/quad_rtc.js`'s `miniquad_add_plugin` call, shouting into the
    /// console if they disagree. It is the guard against a deployed page whose
    /// JS plugin and whose Rust side have drifted apart — the phase 3 failure
    /// that would otherwise present as "the handshake just hangs". Bump both
    /// numbers together whenever the plugin's imports change. Two since
    /// phase 4 added the seven `rtc_*` imports to the two the stub had.
    #[no_mangle]
    pub extern "C" fn quad_rtc_crate_version() -> u32 {
        2
    }

    extern "C" {
        fn fl_build_hash() -> JsObject;
        fn fl_log(msg: JsObject);
    }

    pub fn build_hash() -> String {
        let mut out = String::new();
        // Safe: the plugin returns a JS string, and `to_string` is how
        // sapp-jsutils reads one. If quad_rtc.js failed to load, miniquad
        // refuses to start the module at all, so there is no case where this
        // is called against a missing import.
        unsafe { fl_build_hash().to_string(&mut out) };
        if out.is_empty() {
            "unstamped".to_owned()
        } else {
            out
        }
    }

    pub fn log(msg: &str) {
        unsafe { fl_log(JsObject::string(msg)) };
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub fn build_hash() -> String {
        option_env!("FLOODLINE_GIT_HASH").unwrap_or("dev").to_owned()
    }
}

pub use imp::build_hash;
#[cfg(target_arch = "wasm32")]
pub use imp::log;
