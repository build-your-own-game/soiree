//! Les valeurs validées à la construction et au décodage (§ 2.3, SAL-117).

use jeu_abi::{
    Appui, Axe, Axe2, Borne, Bouton, Cle, CodeTouche, Cumul, Duree, Fini, Fixe, Illisible, Manette,
    NoeudFil, Oui, Places, Rect, Rgba, SceneFil, SorteNoeud, TexteCourt, decoder, encoder, fnv1a64,
    fnv1a64_suite, raison_suffisante,
};

use crate::commun::h;

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    o
}

fn quoi<T: for<'a> serde::Deserialize<'a>>(octets: &[u8]) -> Result<T, Illisible> {
    decoder::<T>(octets).map_err(|e| e.quoi)
}

/// HIS-45, OUT-175, CAT-04, SQ-67 : ni NaN ni infini n'entrent ; un sous-normal et
/// le zéro négatif deviennent 0.
#[test]
fn fini_refuse_nan_infini_et_ramene_les_sous_normaux() {
    assert_eq!(Fini::new(f32::NAN), None);
    assert_eq!(Fini::new(f32::INFINITY), None);
    assert_eq!(Fini::new(f32::NEG_INFINITY), None);
    let sous_normal = f32::from_bits(1);
    assert!(sous_normal.is_subnormal());
    assert_eq!(Fini::new(sous_normal).unwrap().get().to_bits(), 0);
    assert_eq!(Fini::new(-sous_normal).unwrap().get().to_bits(), 0);
    assert_eq!(Fini::new(-0.0).unwrap().get().to_bits(), 0);
    assert_eq!(Fini::new(1.5).unwrap().get(), 1.5);
    assert_eq!(Fini::new(f32::MAX).unwrap().get(), f32::MAX);

    // Au décodage : les octets d'un NaN sont refusés, ceux d'un sous-normal ramenés à 0.
    assert_eq!(
        quoi::<Fini>(&f32::NAN.to_le_bytes()),
        Err(Illisible::Valeur)
    );
    assert_eq!(
        quoi::<Fini>(&f32::INFINITY.to_le_bytes()),
        Err(Illisible::Valeur)
    );
    assert_eq!(quoi::<Fini>(&sous_normal.to_le_bytes()), Ok(Fini::ZERO));
    assert_eq!(
        quoi::<Fini>(&2.0f32.to_le_bytes()),
        Ok(Fini::new(2.0).unwrap())
    );
}

#[test]
fn axe_refuse_hors_de_moins_un_a_un() {
    assert!(Axe::new(1.0).is_some());
    assert!(Axe::new(-1.0).is_some());
    assert_eq!(Axe::new(1.000_001), None);
    assert_eq!(Axe::new(f32::NAN), None);
    assert_eq!(quoi::<Axe>(&1.5f32.to_le_bytes()), Err(Illisible::Valeur));
    assert_eq!(
        quoi::<Axe>(&(-0.5f32).to_le_bytes()),
        Ok(Axe::new(-0.5).unwrap())
    );
}

/// CAT-22 : une croix (1, 1), un NaN ou 1e30 ne font jamais bouger au-delà de la norme 1.
#[test]
fn axe2_refuse_une_norme_au_dela_de_un() {
    assert_eq!(Axe2::new(1.0, 1.0), None);
    assert_eq!(Axe2::new(f32::NAN, 0.0), None);
    let a = Axe2::new(0.6, 0.8).unwrap();
    assert_eq!((a.x(), a.y()), (0.6, 0.8));
    let mut croix = Vec::new();
    croix.extend(1.0f32.to_le_bytes());
    croix.extend(1.0f32.to_le_bytes());
    assert_eq!(quoi::<Axe2>(&croix), Err(Illisible::Valeur));

    // La croix ramenée a la norme 1, dans la même direction.
    let r = Axe2::ramener(1.0, 1.0).unwrap();
    assert!((r.x() - r.y()).abs() < 1e-6);
    assert!((r.x() * r.x() + r.y() * r.y() - 1.0).abs() < 1e-5);
    let grand = Axe2::ramener(1e30, -1e30).unwrap();
    assert!(grand.x() > 0.7 && grand.y() < -0.7);
    assert_eq!(Axe2::ramener(f32::INFINITY, 0.0), None);
    assert_eq!(Axe2::ramener(0.0, 0.0), Some(Axe2::ZERO));
}

#[test]
fn borne_refuse_hors_plage() {
    type Chiffre = Borne<0, 9>;
    assert_eq!(Chiffre::new(10), None);
    assert_eq!(Chiffre::new(-1), None);
    assert_eq!(Chiffre::new(9).unwrap().get(), 9);
    assert_eq!(Chiffre::PLUS_PETIT.get(), 0);
    assert_eq!(Chiffre::PLUS_GRAND.get(), 9);
    assert_eq!(Borne::<-5, 5>::new(-5).unwrap().indice(), 0);
    assert_eq!(Borne::<-5, 5>::new(5).unwrap().indice(), 10);
    assert_eq!(
        Borne::<{ i64::MIN }, { i64::MAX }>::PLUS_GRAND.indice(),
        usize::MAX
    );
    // 10 en zigzag : 20 ; -1 : 1.
    assert_eq!(quoi::<Chiffre>(&h("14")), Err(Illisible::Valeur));
    assert_eq!(quoi::<Chiffre>(&h("01")), Err(Illisible::Valeur));
    assert_eq!(quoi::<Chiffre>(&h("12")), Ok(Chiffre::new(9).unwrap()));
}

/// REM-13 : un compteur absolu en boucle ; un paquet perdu ne décale rien.
#[test]
fn cumul_compte_en_boucle() {
    let avant = Cumul::new(u32::MAX);
    assert_eq!(avant.plus(3).depuis(avant), 3);
    assert_eq!(Cumul::new(5).depuis(Cumul::new(2)), 3);
}

/// REM-16, REM-17 : relâcher une manette garde le compte de ses appuis.
#[test]
fn manette_relachee_garde_les_appuis() {
    let mut m = Manette::REPOS;
    m.sticks[0] = Axe2::new(0.5, 0.0).unwrap();
    m.gachettes[1] = Borne::new(700).unwrap();
    m.boutons[3] = Bouton {
        etat: Appui::Tenu,
        appuis: Cumul::new(2),
    };
    assert_eq!(m.est_relachee(), Oui::Non);
    let r = m.relachee();
    assert_eq!(r.est_relachee(), Oui::Oui);
    assert_eq!(r.boutons[3].appuis, Cumul::new(2));
    assert_eq!(r.boutons[3].etat, Appui::Relache);
    assert_eq!(r.sticks[0], Axe2::ZERO);
}

#[test]
fn texte_court_refuse_les_controles_et_la_longueur() {
    assert!(TexteCourt::essayer(&"a".repeat(64)).is_some());
    assert_eq!(TexteCourt::essayer(&"a".repeat(65)), None);
    assert_eq!(TexteCourt::essayer("bip\u{7}"), None);
    assert_eq!(TexteCourt::essayer("ligne\nsuivante"), None);
    assert_eq!(TexteCourt::essayer("\u{7f}"), None);
    assert_eq!(TexteCourt::essayer("C1 \u{85}"), None);
    assert_eq!(
        TexteCourt::essayer("éèà — ok 🎉").unwrap().as_str(),
        "éèà — ok 🎉"
    );
    assert_eq!(TexteCourt::VIDE.as_str(), "");
    // Au décodage : 65 octets, ou un caractère de contrôle, sont refusés.
    let mut long = vec![65];
    long.extend(std::iter::repeat_n(b'a', 65));
    assert_eq!(quoi::<TexteCourt>(&long), Err(Illisible::Valeur));
    assert_eq!(quoi::<TexteCourt>(&h("02 61 00")), Err(Illisible::Valeur));
    assert_eq!(quoi::<TexteCourt>(&h("02 C3 A9")).unwrap().as_str(), "é");
}

#[test]
fn code_touche_est_de_l_ascii_visible() {
    assert_eq!(CodeTouche::ESPACE.as_str(), "Space");
    assert_eq!(CodeTouche::essayer(""), None);
    assert_eq!(CodeTouche::essayer("Key W"), None);
    assert_eq!(CodeTouche::essayer("Touché"), None);
    assert_eq!(CodeTouche::essayer(&"K".repeat(25)), None);
    assert!(CodeTouche::essayer(&"K".repeat(24)).is_some());
    assert_eq!(quoi::<CodeTouche>(&h("00")), Err(Illisible::Valeur));
}

/// OUT-200, HIS-13 : des places incohérentes ne se décodent pas.
#[test]
fn places_refusent_l_incoherence() {
    assert_eq!(Places::essayer(2, 1, 3), None);
    assert_eq!(Places::essayer(0, 0, 0), None);
    assert_eq!(Places::essayer(1, 8, 4), None);
    let p = Places::joueurs(1, 8).appareils(16);
    assert_eq!(
        (p.joueurs_min(), p.joueurs_max(), p.appareils_max()),
        (1, 8, 16)
    );
    assert_eq!(Places::joueurs(2, 4).appareils_max(), 4);
    assert_eq!(quoi::<Places>(&h("02 01 03")), Err(Illisible::Valeur));
    assert_eq!(quoi::<Places>(&encode(&p)), Ok(p));
}

/// HIS-04, OUT-03 : une raison dit quelque chose, en trois mots au moins.
#[test]
fn raison_creuse_refusee() {
    assert!(!raison_suffisante(""));
    assert!(!raison_suffisante("n/a"));
    assert!(!raison_suffisante("sans objet"));
    assert!(!raison_suffisante("   \t\n "));
    assert!(raison_suffisante("tour par tour"));
    assert!(raison_suffisante("  aucune  manette\tici "));
}

/// CAT-15, SQ-35 : des durées en pas entiers, arrondies vers le haut.
#[test]
fn duree_compte_en_pas_entiers() {
    assert_eq!(Duree::ms(450).en_pas(), 27);
    assert_eq!(Duree::ms(1).en_pas(), 1);
    assert_eq!(Duree::ms(0).en_pas(), 0);
    assert_eq!(Duree::ms(1000).en_pas(), 60);
    assert_eq!(Duree::ms(1001).en_pas(), 61);
    assert_eq!(Duree::s(20).en_pas(), 1200);
    assert_eq!(Duree::pas(27).en_ms(), 450);
    assert_eq!(Duree::pas(1).en_ms(), 16);
    assert_eq!(Duree::pas(1).secondes_restantes(), 1);
    assert_eq!(Duree::pas(60).secondes_restantes(), 1);
    assert_eq!(Duree::pas(61).secondes_restantes(), 2);
    assert_eq!(Duree::ZERO.secondes_restantes(), 0);
    // Saturé, jamais un débordement.
    assert_eq!(Duree::s(u64::MAX).en_pas(), u64::MAX);
    assert_eq!(Duree::pas(u64::MAX).plus(Duree::pas(1)).en_pas(), u64::MAX);
    assert_eq!(Duree::pas(u64::MAX).en_ms(), u64::MAX);
    assert!(Duree::ms(u64::MAX).en_pas() > 0);
}

/// REM-57 : une longueur de la scène n'est jamais NaN ; la saturation se voit.
#[test]
fn fixe_sature_et_nan_donne_zero() {
    assert_eq!(Fixe::depuis_f32(f32::NAN), Fixe::ZERO);
    assert_eq!(Fixe::depuis_f32(f32::INFINITY), Fixe::MAX);
    assert_eq!(Fixe::depuis_f32(-1e30), Fixe::MIN);
    assert_eq!(Fixe::depuis_f32(1.5).brut(), 384);
    assert_eq!(Fixe::unites(2).brut(), 512);
    assert_eq!(Fixe::unites(i32::MAX), Fixe::MAX);
    assert_eq!(Fixe::MAX.saturee(), Oui::Oui);
    assert_eq!(Fixe::unites(3).saturee(), Oui::Non);
    assert!((Fixe::depuis_brut(384).en_unites() - 1.5).abs() < f64::EPSILON);
}

/// SAL-165 : une clé est une fonction de la valeur, jamais de la position.
#[test]
fn cle_est_le_fnv_de_l_encodage() {
    let v = ("nombre", 3u32);
    assert_eq!(Cle::de(&v), Cle::de(&("nombre", 3u32)));
    assert_ne!(Cle::de(&v), Cle::de(&("nombre", 4u32)));
    assert_eq!(Cle::de(&v).fil(), fnv1a64(&encode(&v)));
}

/// Les vecteurs de référence de FNV-1a 64 bits.
#[test]
fn fnv1a64_vecteurs_de_reference() {
    const A_LA_COMPILATION: u64 = fnv1a64(b"foobar");
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    assert_eq!(fnv1a64_suite(fnv1a64(b"foo"), b"bar"), fnv1a64(b"foobar"));
    assert_eq!(A_LA_COMPILATION, 0x8594_4171_f739_67e8);
}

fn noeud(cle: u64, parent: Option<u32>, sorte: SorteNoeud) -> NoeudFil {
    NoeudFil::simple(Cle::du_fil(cle), parent, sorte)
}

fn rect() -> SorteNoeud {
    SorteNoeud::Rect {
        r: Rect {
            x: Fixe::ZERO,
            y: Fixe::ZERO,
            l: Fixe::ZERO,
            h: Fixe::ZERO,
        },
        couleur: Rgba::NOIR,
    }
}

/// La scène à plat : un parent précède ses enfants et peut en avoir.
#[test]
fn scene_refuse_une_structure_mal_formee() {
    let mut s = SceneFil::vide();
    s.noeuds = vec![
        noeud(1, None, SorteNoeud::Definitif),
        noeud(2, Some(0), rect()),
    ];
    assert_eq!(quoi::<SceneFil>(&encode(&s)), Ok(s.clone()));

    // Un enfant avant son parent.
    s.noeuds = vec![
        noeud(2, Some(1), rect()),
        noeud(1, None, SorteNoeud::Definitif),
    ];
    assert_eq!(quoi::<SceneFil>(&encode(&s)), Err(Illisible::Valeur));
    // Son propre parent.
    s.noeuds = vec![noeud(1, Some(0), SorteNoeud::Definitif)];
    assert_eq!(quoi::<SceneFil>(&encode(&s)), Err(Illisible::Valeur));
    // Un rectangle comme parent.
    s.noeuds = vec![noeud(1, None, rect()), noeud(2, Some(0), rect())];
    assert!(s.verifier_structure().is_err());
    assert_eq!(quoi::<SceneFil>(&encode(&s)), Err(Illisible::Valeur));
}

/// Besoin 5 des jeux (`conception/BESOINS-DES-JEUX.md`, Punchline n° 5) : une clé se
/// calcule dans une constante, et c'est la même que celle de `Cle::de`.
#[test]
fn cle_nommee_et_numero_valent_cle_de() {
    const POLICE: Cle = Cle::nommee("police des titres");
    assert_eq!(POLICE, Cle::de("police des titres"));
    let long = "é".repeat(200);
    for nom in ["", "a", "Ω", long.as_str()] {
        assert_eq!(Cle::nommee(nom), Cle::de(nom), "{nom:?}");
        assert_eq!(jeu_abi::IdEffet::nomme(nom), jeu_abi::IdEffet::de(nom));
    }
    for n in [
        0u64,
        1,
        127,
        128,
        300,
        16_383,
        16_384,
        u64::from(u32::MAX),
        u64::MAX,
    ] {
        assert_eq!(Cle::numero(n), Cle::de(&n), "{n}");
    }
}
