//! Citizen names.
//!
//! Design §11 asked whether citizens should be nameable and the answer was
//! yes, cheaply: a `u16` index into this table, drawn from `World.rng` when a
//! citizen spawns. Two bytes per person, no heap allocation in `World`, and
//! nothing here can be edited into a desync.
//!
//! Two hundred and fifty-six of them, so `Rng::below(256)` picks one and a
//! city of eight is very unlikely to hold a duplicate. English names of the
//! eleventh and twelfth centuries, because the buildings are a hearth, a
//! cottage and a dike and the people should sound like they belong in front of
//! them.

/// Indexed by `Citizen::name`.
#[rustfmt::skip]
pub const NAMES: [&str; 256] = [
    "Aldwin", "Alys", "Anselm", "Avice", "Baldric", "Beatrix",
    "Bertram", "Cecily", "Cuthbert", "Denise", "Drogo", "Edith",
    "Egbert", "Elaine", "Emma", "Everard", "Fulk", "Geoffrey",
    "Gilbert", "Godiva", "Gunnora", "Hamon", "Hawise", "Herbert",
    "Hugh", "Isolde", "Ivo", "Joan", "Jocelyn", "Lettice",
    "Leofric", "Mabel", "Maud", "Miles", "Nesta", "Odo",
    "Osbert", "Petronilla", "Ralph", "Reynold", "Roger", "Rohese",
    "Sibyl", "Simon", "Swein", "Thurstan", "Ulf", "Walter",
    "Wilfrid", "Wulfric", "Adela", "Alfred", "Amice", "Ancel",
    "Arnold", "Audrey", "Bardolf", "Bennet", "Blythe", "Brice",
    "Cristina", "Diota", "Dunstan", "Edmund", "Elias", "Ellis",
    "Emeline", "Ernald", "Esmond", "Eudo", "Fulke", "Garnet",
    "Gervase", "Gunhild", "Hadwisa", "Harding", "Hawkin", "Helewise",
    "Hereward", "Hilda", "Ida", "Ingrid", "Jordan", "Juliana",
    "Katherine", "Kenelm", "Lambert", "Leofwin", "Lovel", "Lucy",
    "Margery", "Martin", "Mathilda", "Merewald", "Milburga", "Nicola",
    "Norman", "Oswin", "Payn", "Perrin", "Quenilda", "Randal",
    "Regenweald", "Rhiannon", "Richenda", "Robert", "Rosamund", "Rowena",
    "Sampson", "Saeric", "Selwyn", "Sigrid", "Stephen", "Sybilla",
    "Tedric", "Theobald", "Thomasine", "Tibold", "Tobias", "Turold",
    "Uctred", "Urse", "Vivien", "Waldef", "Warin", "Wibert",
    "Wilburh", "Winifred", "Wymark", "Ysabel", "Aelfric", "Agnes",
    "Alban", "Aline", "Anketil", "Anstice", "Arlette", "Ascelin",
    "Athelstan", "Avelina", "Baldwin", "Bartholomew", "Basilia", "Bethoc",
    "Bogo", "Botild", "Brictric", "Burgred", "Cadell", "Ceolwulf",
    "Clarice", "Colswein", "Constance", "Custance", "Dionisia", "Drew",
    "Eadgyth", "Ealdred", "Edeva", "Eldred", "Elfgiva", "Emelot",
    "Engelram", "Eustace", "Fastrada", "Felice", "Frideswide", "Fromund",
    "Galiena", "Gamel", "Gerard", "Gladuse", "Godfrey", "Godwin",
    "Goscelin", "Grimbald", "Guiscard", "Gundreda", "Hamelin", "Havoise",
    "Hawkyn", "Hemming", "Hildegard", "Humphrey", "Idonea", "Ingram",
    "Isabel", "Ivetta", "Jocosa", "Kentigern", "Lecelina", "Leofstan",
    "Letice", "Lewin", "Mainard", "Manasses", "Marjory", "Maynard",
    "Melisent", "Muriel", "Nigel", "Ordgar", "Orm", "Osgood",
    "Osmund", "Pagan", "Paulina", "Peverel", "Philippa", "Ranulf",
    "Reginald", "Rhys", "Richilde", "Riculf", "Roese", "Roland",
    "Sabina", "Saewulf", "Sewale", "Sigar", "Siward", "Sperling",
    "Sunniva", "Thoraldus", "Thurkill", "Tovi", "Ulfketel", "Uhtred",
    "Valentine", "Vitalis", "Wadard", "Walkelin", "Wandrille", "Warner",
    "Werburh", "Wigod", "Wimund", "Wistan", "Wulfnoth", "Wulfstan",
    "Ymania", "Yseult", "Zouche", "Aethelred", "Ada", "Alditha",
    "Alfwold", "Almaric", "Alwara", "Amabel", "Ansgar", "Archil",
    "Arkil", "Asketil", "Aubrey", "Auti",
];

/// The name of a citizen, or a legible fallback if the index is somehow out of
/// range. Out of range should be impossible — the field is only ever written
/// from `Rng::below(NAMES.len())` — but a panic in a draw routine would take
/// the frame down over a cosmetic detail.
pub fn name_of(index: u16) -> &'static str {
    NAMES.get(index as usize).copied().unwrap_or("Someone")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use std::collections::BTreeSet;

    #[test]
    fn the_table_is_full_and_has_no_repeats() {
        assert_eq!(NAMES.len(), 256);
        let distinct: BTreeSet<&str> = NAMES.iter().copied().collect();
        assert_eq!(distinct.len(), 256, "a name appears twice");
        assert!(NAMES.iter().all(|n| !n.is_empty()));
    }

    #[test]
    fn every_index_a_draw_can_produce_has_a_name() {
        let mut r = Rng::new(1);
        for _ in 0..5000 {
            let i = r.below(NAMES.len() as u32) as u16;
            assert_ne!(name_of(i), "Someone", "index {i} fell through");
        }
    }

    #[test]
    fn an_impossible_index_does_not_panic() {
        assert_eq!(name_of(60_000), "Someone");
    }
}
