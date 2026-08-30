//! `sim` depends on `serde` and `postcard`, and on nothing else, ever.
//!
//! Design §7 and the plan both make this the crate's defining property, and
//! CLAUDE.md makes it non-negotiable. It is asserted by reading `Cargo.toml`
//! rather than by anybody remembering, because the way this rule gets broken
//! is not a decision to break it — it is one `rand` or one `hashbrown` added
//! to fix something at eleven at night.
//!
//! The plan puts this test at item 11. It is here at item 3 instead: a rule
//! enforced from the first commit costs nothing, and a rule enforced after
//! eight more items is a rule that has already been broken once.

use std::collections::BTreeSet;

const ALLOWED: [&str; 2] = ["serde", "postcard"];

#[test]
fn sim_depends_on_serde_and_postcard_and_nothing_else() {
    let manifest = include_str!("../Cargo.toml");
    let found = dependency_names(manifest);
    let allowed: BTreeSet<&str> = ALLOWED.into_iter().collect();

    assert_eq!(
        found,
        allowed,
        "\n`sim`'s dependencies are {found:?}, and the only ones allowed are \
         {allowed:?}.\n\
         If a new one is genuinely needed, that is a decision: write the \
         paragraph in DECISIONS.md and change ALLOWED here. Do not delete \
         this test.\n"
    );
}

#[test]
fn sim_names_no_graphics_or_networking_crate() {
    // Belt and braces against the above being edited without thought: these
    // are the specific crates whose arrival would mean the boundary has gone,
    // and naming them makes the failure message say what actually went wrong.
    let manifest = include_str!("../Cargo.toml");
    for banned in [
        "macroquad", "miniquad", "sapp-jsutils", "matchbox_socket", "futures", "rand",
        "hashbrown", "rapier2d", "wasm-bindgen", "web-sys",
    ] {
        assert!(
            !dependency_names(manifest).contains(banned),
            "`sim` has picked up {banned}, which is exactly what this crate exists not to do"
        );
    }
}

/// Every dependency name in a manifest, from all three dependency tables.
///
/// A deliberately small parser rather than a toml crate: `sim` may not have
/// dependencies, and neither may the test that checks it, or the check is
/// worth less than the thing it checks.
fn dependency_names(manifest: &str) -> BTreeSet<&str> {
    let mut names = BTreeSet::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`,
            // and any `[target.'...'.dependencies]` form of the three.
            in_deps = line.ends_with("dependencies]");
            continue;
        }
        if !in_deps || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = line.split_once('=') {
            // `serde.workspace = true` puts the dotted key on the left, so the
            // dependency's name is the first segment. Crate names cannot
            // contain a dot, so this is unambiguous.
            let name = key.trim().split('.').next().unwrap_or("").trim_matches('"');
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    names
}

#[test]
fn the_manifest_parser_reads_what_it_claims_to() {
    // The parser is the thing standing between us and a silently passing
    // boundary test, so it gets its own. `serde.workspace = true` is the case
    // that caught it: the dotted key put "serde.workspace" in the set and the
    // real check went green against the wrong names.
    let sample = r#"
[package]
name = "sim"
edition = "2021"

[dependencies]
serde.workspace = true
postcard = { version = "1" }
"quoted-key" = "1"
# commented = "1"

[dev-dependencies]
proptest = "1"

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[features]
default = []
"#;
    let found = dependency_names(sample);
    assert!(found.contains("serde"), "a dotted workspace key is still a dependency");
    assert!(found.contains("postcard"));
    assert!(found.contains("quoted-key"), "quoted keys count");
    assert!(found.contains("proptest"), "dev-dependencies count too");
    assert!(found.contains("libc"), "target-gated dependencies count too");
    assert!(!found.contains("commented"), "comments do not");
    assert!(!found.contains("serde.workspace"), "and the dot is not part of the name");
    assert!(!found.contains("default"), "[features] is not a dependency table");
    assert!(!found.contains("name"), "neither is [package]");
}

#[test]
fn the_hearth_can_hold_what_a_city_starts_with() {
    // A city's whole opening stock is put in its Hearth. If that is more than
    // the Hearth will hold, every hauler that comes back with leftovers finds
    // no room, keeps them, and the goods are gone from the city for good — a
    // fifth of the run's stone disappeared this way when the starting stone
    // went up and this number stayed where it was.
    use sim::balance::{STARTING_STONE, STARTING_WOOD};
    use sim::building::Kind;

    let room = Kind::Hearth.capacity();
    assert!(
        room.wood >= STARTING_WOOD,
        "a city starts with {STARTING_WOOD} wood and its hearth holds {}",
        room.wood
    );
    assert!(
        room.stone >= STARTING_STONE,
        "a city starts with {STARTING_STONE} stone and its hearth holds {}",
        room.stone
    );
}
