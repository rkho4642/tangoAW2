//! Tag data for AW2's Sturm, who is not in Dual Strike. **tangoAW2's own,
//! made up for tangoAW2, not Dual Strike's**: his compatibility with every
//! CO, his special pairs with his Black Hole teammates (stars, Tag Power
//! names) and their victory exchanges, written for tangoAW2 in AW2's terse
//! villain voice to fit AW2's results quote box (104 pixels a line).
//! Symmetric (Sturm + X as X + Sturm); with the Dual Strike pack only, as
//! every tag pair (crate::tag). Sturm's tag-in line is one of AW2's own
//! Sturm power quotes (crate::tag_extras).
//!
//! | Pair | Compatibility | Stars | Tag Power |
//! |---|---|---|---|
//! | Sturm + Von Bolt | 125 | 3 | Black Apocalypse |
//! | Sturm + Hawke | 120 | 2 | Storm Front |
//! | Sturm + Lash | 115 | 2 | Mad Genius |
//! | Sturm + Flak | 110 | 1 | Iron Fist |
//! | Sturm + Adder | 110 | 1 | Viper's Nest |
//! | Sturm + Kindle, Jugger, Koal | 105 | - | - |
//! | Sturm + anyone else | 95 | - | - |

/// AW2's Sturm.
pub const STURM: u8 = 10;

/// A special pair of Sturm's (tangoAW2's own).
pub struct Pair {
    /// The partner's Dual Strike id.
    pub partner: u8,
    pub compatibility: u8,
    pub stars: u8,
    pub name: &'static str,
    /// Sturm winning: two exchanges, his line then the partner's.
    pub sturm_first: [(&'static str, &'static str); 2],
    /// The partner winning: two exchanges, its line then Sturm's.
    pub partner_first: [(&'static str, &'static str); 2],
}

/// Sturm's special pairs, in his TAG box's order.
pub const PAIRS: [Pair; 5] = [
    Pair {
        partner: 11, // Von Bolt
        compatibility: 125,
        stars: 3,
        name: "Black Apocalypse",
        sturm_first: [("Bow before Black Hole!", "Delicious."), ("Your world is ours.", "Hhhh... yes.")],
        partner_first: [("Hhhh... your despair...", "Delicious."), ("The world withers.", "As it should.")],
    },
    Pair {
        partner: 15, // Hawke
        compatibility: 120,
        stars: 2,
        name: "Storm Front",
        sturm_first: [("Pathetic. As expected.", "Too easy."), ("Kneel, worms!", "Hmph. Over.")],
        partner_first: [("A fitting end.", "Ha! Worms."), ("Black Hole prevails.", "Naturally.")],
    },
    Pair {
        partner: 13, // Lash
        compatibility: 115,
        stars: 2,
        name: "Mad Genius",
        sturm_first: [("Fools! Black Hole wins!", "Ha! Fun!"), ("Your end has come.", "Told you!")],
        partner_first: [("Ha! Too easy!", "Of course."), ("Genius, right?", "Hmph. Yes.")],
    },
    Pair {
        partner: 26, // Flak
        compatibility: 110,
        stars: 1,
        name: "Iron Fist",
        sturm_first: [("Crushed like insects.", "Har! Easy!"), ("Despair, fools!", "Smashed 'em!")],
        partner_first: [("Har! Smashed 'em!", "Good."), ("Who's next, boss?", "Everyone.")],
    },
    Pair {
        partner: 27, // Adder
        compatibility: 110,
        stars: 1,
        name: "Viper's Nest",
        sturm_first: [("Black Hole is eternal!", "Sss... slow."), ("Grovel before me!", "Hssss!")],
        partner_first: [("Sss... too slow.", "Pathetic."), ("Such fragile prey.", "Crush them.")],
    },
];

/// Black Hole's other COs (Dual Strike ids: Kindle, Jugger, Koal): 105.
const TEAMMATES: [u8; 3] = [25, 12, 14];
const TEAMMATE_COMPATIBILITY: u8 = 105;
const OTHER_COMPATIBILITY: u8 = 95;

/// The other CO of a pair with Sturm (None: no Sturm in it).
fn other(a: u8, b: u8) -> Option<u8> {
    if a == STURM {
        Some(b)
    } else if b == STURM {
        Some(a)
    } else {
        None
    }
}

/// A pair's compatibility if Sturm is in it.
pub fn compatibility(a: u8, b: u8) -> Option<u8> {
    let o = other(a, b)?;
    let d = crate::tag::ds_id(o);
    Some(match d {
        Some(d) => match PAIRS.iter().find(|p| p.partner == d) {
            Some(p) => p.compatibility,
            None if TEAMMATES.contains(&d) => TEAMMATE_COMPATIBILITY,
            None => OTHER_COMPATIBILITY,
        },
        None => OTHER_COMPATIBILITY,
    })
}

/// Sturm's special pair with the other CO, if they are one.
pub fn special(a: u8, b: u8) -> Option<&'static Pair> {
    let d = crate::tag::ds_id(other(a, b)?)?;
    PAIRS.iter().find(|p| p.partner == d)
}

/// The pair's Tag Power name and four victory lines in Dual Strike's order
/// for `a`'s record: `a`'s line, `b`'s, `a`'s, `b`'s.
pub fn texts(a: u8, b: u8) -> Option<(Vec<u8>, [Vec<u8>; 4])> {
    let p = special(a, b)?;
    let ex = if a == STURM { &p.sturm_first } else { &p.partner_first };
    let v = |s: &str| s.as_bytes().to_vec();
    Some((v(p.name), [v(ex[0].0), v(ex[0].1), v(ex[1].0), v(ex[1].1)]))
}

/// Sturm's partners and stars, his TAG box's order (AW2 CO ids).
pub fn partners() -> Vec<(u8, u8)> {
    PAIRS.iter().filter_map(|p| Some((crate::tag::aw2_co(p.partner)?, p.stars))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        assert_eq!(PAIRS.len(), 5);
        for p in &PAIRS {
            assert!((1..=3).contains(&p.stars));
            assert!(p.name.bytes().all(|c| c.is_ascii_alphabetic() || c == b' ' || c == b'\''));
        }
        // Symmetric, and Sturm with himself or a stranger: 95.
        assert_eq!(compatibility(STURM, STURM), Some(95));
        assert_eq!(compatibility(1, 2), None);
    }
}
