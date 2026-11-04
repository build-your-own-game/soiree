//! Le décodage strict (§ 3.7).

use jeu_abi::{Appel, EntreeFil, ErreurFil, Illisible, Retour, decoder};

use crate::commun::{corpus, h};

/// Un message suivi d'un octet, ou de deux messages collés, ne se lit pas comme le
/// premier (REM-59, OUT-146, SAL-118).
#[test]
fn decodage_refuse_un_octet_restant() {
    for c in corpus() {
        for suffixe in [h("00"), h("FF"), c.attendu.clone()] {
            let mut octets = c.attendu.clone();
            octets.extend_from_slice(&suffixe);
            let ou = u32::try_from(c.attendu.len()).unwrap();
            assert_eq!(
                c.message.relire(&octets),
                Err(ErreurFil {
                    ou,
                    quoi: Illisible::OctetsRestants
                }),
                "{} suivi de {suffixe:02X?}",
                c.nom
            );
        }
    }
}

/// Une valeur coupée ne se lit pas : chaque préfixe strict de chaque message est refusé.
#[test]
fn decodage_refuse_un_message_coupe() {
    for c in corpus() {
        for n in 0..c.attendu.len() {
            assert!(
                c.message.relire(&c.attendu[..n]).is_err(),
                "{} coupé à {n} octets",
                c.nom
            );
        }
    }
}

/// Une longueur absurde échoue, sans lire au-delà de la fin (REM-58). La mémoire
/// réservée est mesurée à part, dans `tests/allocations.rs`.
#[test]
fn longueur_absurde_echoue() {
    // Regarder, avec 2^62 regards annoncés et rien derrière.
    let octets = h("05 80 80 80 80 80 80 80 80 40");
    assert_eq!(
        decoder::<Appel<'_>>(&octets).map(|_| ()),
        Err(ErreurFil {
            ou: 10,
            quoi: Illisible::Fin
        })
    );
    // Des octets bruts de 2^62 octets annoncés.
    let octets = h("09 80 80 80 80 80 80 80 80 40 01 02");
    assert!(decoder::<Appel<'_>>(&octets).is_err());
}

#[test]
fn decodage_refuse_les_formes_invalides() {
    // Un varint de onze octets pour un u64.
    let varint = h("06 01 FF FF FF FF FF FF FF FF FF FF 01");
    assert_eq!(
        decoder::<Appel<'_>>(&varint)
            .map(|_| ())
            .map_err(|e| e.quoi),
        Err(Illisible::Varint)
    );
    // Un variant inconnu : `Appel` en a 21 dans l'ABI 2, numérotés de 0 à 0x14.
    assert_eq!(
        decoder::<Appel<'_>>(&h("15"))
            .map(|_| ())
            .map_err(|e| e.quoi),
        Err(Illisible::Valeur)
    );
    // Une option dont l'octet ne vaut ni 0 ni 1 : `garde` de `Creer`.
    let option = h("02 00*32 00 00 00 00 02");
    assert_eq!(
        decoder::<Appel<'_>>(&option)
            .map(|_| ())
            .map_err(|e| e.quoi),
        Err(Illisible::Option)
    );
    // Un texte qui n'est pas de l'UTF-8 : `libre` d'une attente, dans `Retour::Etat`.
    let texte = h("05 00 00 01 02 00 01 01 FF 00 00 00 00 00 00 00");
    assert_eq!(
        decoder::<Retour<'_>>(&texte)
            .map(|_| ())
            .map_err(|e| e.quoi),
        Err(Illisible::Texte)
    );
    // Rien du tout.
    assert_eq!(
        decoder::<EntreeFil<'_>>(&[])
            .map(|_| ())
            .map_err(|e| e.quoi),
        Err(Illisible::Fin)
    );
}

/// postcard lit un varint non minimal comme la valeur qu'il porte : deux suites
/// d'octets peuvent donner la même valeur. Ce test fige ce comportement, que la
/// documentation de `jeu_abi` annonce ; `encoder` n'écrit que la forme minimale.
#[test]
fn varint_non_minimal_se_lit_comme_sa_valeur() {
    let attendu = Appel::Etat { connues: vec![0] };
    assert_eq!(decoder::<Appel<'_>>(&h("06 01 80 00")).unwrap(), attendu);
    let mut minimal = Vec::new();
    jeu_abi::encoder(&attendu, &mut minimal).unwrap();
    assert_eq!(minimal, h("06 01 00"));
}
