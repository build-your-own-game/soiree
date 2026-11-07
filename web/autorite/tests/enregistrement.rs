//! La forme de ce que le Worker garde dans `IndexedDB` (PHASE-3, § 2.4, P37 ; lot 10a) :
//! ses octets figés, une seconde lecture faite à la main (SAL-90), et un enregistrement
//! abîmé refusé sans panique (REM-64), jamais pris pour une salle neuve.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y ont
//! droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles d'écriture »).

#![cfg(test)]

use autorite::enregistrement::{Enregistrement, Illisible, VERSION_ENREGISTREMENT};

fn un() -> Enregistrement {
    Enregistrement {
        salle: [0x3f, 0xa9, 0xc2, 0xd1],
        privee: [1; 32],
        secret: [2; 16],
        garde: [3; 32],
        module: [4; 32],
        dernier: Some(0x0102),
    }
}

fn hexa(o: &[u8]) -> String {
    use std::fmt::Write;
    o.iter().fold(String::new(), |mut s, b| {
        let _: std::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

#[test]
fn la_forme_de_l_enregistrement_est_figee() {
    assert_eq!(VERSION_ENREGISTREMENT, 1);
    let attendu = format!(
        "01000000{}{}{}{}{}{}",
        "3fa9c2d1",
        "01".repeat(32),
        "02".repeat(16),
        "03".repeat(32),
        "04".repeat(32),
        "010201000000000000"
    );
    assert_eq!(hexa(&un().encoder()), attendu);
    let mut sans = un();
    sans.dernier = None;
    assert!(hexa(&sans.encoder()).ends_with(&format!("{}00", "04".repeat(32))));
    assert_eq!(Enregistrement::decoder(&un().encoder()).unwrap(), un());
    assert_eq!(Enregistrement::decoder(&sans.encoder()).unwrap(), sans);
}

/// SAL-90 : une seconde lecture, à la main, retrouve chaque champ.
#[test]
fn une_seconde_lecture_relit_l_enregistrement_sal90() {
    let o = un().encoder();
    assert_eq!(u32::from_le_bytes(o[0..4].try_into().unwrap()), 1);
    assert_eq!(&o[4..8], &[0x3f, 0xa9, 0xc2, 0xd1]);
    assert_eq!(&o[8..40], &[1; 32]);
    assert_eq!(&o[40..56], &[2; 16]);
    assert_eq!(&o[56..88], &[3; 32]);
    assert_eq!(&o[88..120], &[4; 32]);
    assert_eq!(o[120], 1);
    assert_eq!(u64::from_le_bytes(o[121..129].try_into().unwrap()), 0x0102);
    assert_eq!(o.len(), 129);
}

/// Un enregistrement d'une autre version, d'une autre longueur, au drapeau faux, est refusé
/// (et le Worker le dit) ; aucun octet ne fait paniquer le décodeur (REM-64).
#[test]
fn un_enregistrement_abime_est_refuse_rem64() {
    let mut o = un().encoder();
    o[0] = 2;
    assert_eq!(Enregistrement::decoder(&o), Err(Illisible::Version(2)));
    let o = un().encoder();
    assert!(matches!(
        Enregistrement::decoder(&o[..o.len() - 1]),
        Err(Illisible::Longueur(_))
    ));
    let mut long = o.clone();
    long.push(0);
    assert!(Enregistrement::decoder(&long).is_err());
    let mut drapeau = o.clone();
    drapeau[120] = 0;
    assert_eq!(
        Enregistrement::decoder(&drapeau),
        Err(Illisible::Drapeau(0))
    );
    let mut sans = un();
    sans.dernier = None;
    let mut court = sans.encoder();
    court[120] = 1;
    assert_eq!(Enregistrement::decoder(&court), Err(Illisible::Drapeau(1)));
    court[120] = 7;
    assert_eq!(Enregistrement::decoder(&court), Err(Illisible::Drapeau(7)));
    // Tous les préfixes, et chaque octet abîmé.
    for n in 0..=o.len() {
        let _: Result<Enregistrement, Illisible> = Enregistrement::decoder(&o[..n]);
    }
    for i in 0..o.len() {
        for b in [0u8, 1, 0x7f, 0xff] {
            let mut x = o.clone();
            x[i] = b;
            let _: Result<Enregistrement, Illisible> = Enregistrement::decoder(&x);
        }
    }
}

#[test]
fn le_debug_ne_montre_aucun_secret() {
    let d = format!("{:?}", un());
    assert!(!d.contains("[1, 1") && !d.contains("[2, 2") && !d.contains("[3, 3"));
}
