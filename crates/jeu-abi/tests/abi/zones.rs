//! Les deux formes écrites hors de postcard (§ 3.2, § 3.5) : la zone de panique et les
//! bords de la forme canonique, comparées à des octets écrits à la main. Une seule
//! définition pour la colle qui les écrit et pour le moteur qui les lit : un écart
//! entre les deux classerait une panique comme un piège anonyme, et le rapport
//! perdrait son message (HIS-07).

use jeu_abi::{
    Canonique, PANIQUE_CODE, PANIQUE_ENTETE, entete_de_panique, lire_canonique, lire_panique,
};

#[test]
fn zone_de_panique_forme_figee() {
    assert_eq!(PANIQUE_CODE, 1);
    assert_eq!(PANIQUE_ENTETE, 8);
    assert_eq!(entete_de_panique(3), [1, 0, 0, 0, 3, 0, 0, 0]);
    let mut zone = vec![1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c', b'd'];
    assert_eq!(lire_panique(&zone), Some(&b"abc"[..]));
    // Une longueur qui dépasse la zone est coupée à la zone.
    zone[4] = 200;
    assert_eq!(lire_panique(&zone), Some(&b"abcd"[..]));
    // Sans le code, pas de panique ; une zone plus courte que son entête non plus.
    zone[0] = 0;
    assert_eq!(lire_panique(&zone), None);
    assert_eq!(lire_panique(&[1, 0, 0, 0, 1]), None);
}

#[test]
fn bords_de_la_forme_canonique() {
    let mut o = 0x0102_0304_0506_0708_u64.to_le_bytes().to_vec();
    o.extend([9, 9, 9]);
    o.extend(2_u64.to_le_bytes());
    assert_eq!(
        lire_canonique(&o),
        Some(Canonique {
            forme: 0x0102_0304_0506_0708,
            corps: &[9, 9, 9],
            non_finis: 2,
        })
    );
    assert_eq!(lire_canonique(&o[..16]).map(|c| c.corps.len()), Some(0));
    assert_eq!(lire_canonique(&o[..15]), None);
}
