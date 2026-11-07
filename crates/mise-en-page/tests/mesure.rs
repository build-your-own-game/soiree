//! La mesure des textes (PHASE-3, § 2.9) : la police du commun et la validation des
//! polices (P22), la coupe, la taille qui rétrécit pour tenir, les débordements.

use jeu_abi::{Alignement, Cle, Coupe, Fixe, Lignes, Oui, Rect, Rgba, StyleTexte, Taille};
use mise_en_page::{
    Agencement, Debord, FAMILLE_DU_COMMUN, INTERLIGNE, MiseEnPage, OCTETS_DU_COMMUN, Polices,
    RefusPolice, Tient, valider_police,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};
use unicode_segmentation::UnicodeSegmentation;

/// Une boîte de `l` × `h` unités.
fn boite(l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::ZERO,
        y: Fixe::ZERO,
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

fn style(taille: Taille, lignes: Lignes, coupe: Coupe, alignement: Alignement) -> StyleTexte {
    StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille,
        couleur: Rgba::BLANC,
        alignement,
        lignes,
        coupe,
    }
}

fn fixe(n: i32) -> Taille {
    Taille::Fixe(Fixe::unites(n))
}

fn mise_en_page() -> MiseEnPage {
    MiseEnPage::new(&Polices::du_commun())
}

/// Les lignes du texte agencé.
fn lignes(a: &Agencement) -> Vec<&str> {
    if a.texte.is_empty() {
        Vec::new()
    } else {
        a.texte.split('\n').collect()
    }
}

/// Un en-tête WOFF2 de 48 octets, sans table : de quoi reconnaître la signature.
fn woff2() -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"wOF2");
    o.extend_from_slice(&[0, 1, 0, 0]);
    o.extend_from_slice(&48_u32.to_be_bytes());
    o.extend_from_slice(&[0; 36]);
    o
}

#[test]
fn la_police_du_commun_se_lit() {
    assert_eq!(valider_police(OCTETS_DU_COMMUN), Ok(()));
    // La copie de Liberation Sans 2.1.5 que Tier list a vérifiée (son BESOINS-COMMUN,
    // § 17) : 410 712 octets, et sa table `name` la dit sous SIL OFL 1.1, version 2.1.5.
    assert_eq!(OCTETS_DU_COMMUN.len(), 410_712);
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_be_bytes).collect() };
    for attendu in ["SIL Open Font License, Version 1.1", "Version 2.1.5"] {
        let o = utf16(attendu);
        assert!(
            OCTETS_DU_COMMUN.windows(o.len()).any(|w| w == o.as_slice()),
            "la table name ne dit pas « {attendu} »"
        );
    }
    // Le sha256 que `polices.rs` cite : les octets embarqués sont ceux de la copie
    // vérifiée, et la licence à côté d'eux est la sienne.
    let sha256 = |o: &[u8]| -> String {
        use std::fmt::Write as _;
        Sha256::digest(o).iter().fold(String::new(), |mut h, b| {
            let _: Result<(), _> = write!(h, "{b:02x}");
            h
        })
    };
    assert_eq!(
        sha256(OCTETS_DU_COMMUN),
        "76d04c18ea243f426b7de1f3ad208e927008f961dc5945e5aad352d0dfde8ee8"
    );
    assert_eq!(
        sha256(include_bytes!("../polices/OFL-Liberation.txt")),
        "93fed46019c38bbe566b479d22148e2e8a1e85ada614accb0211c37b2c61c19b"
    );
    let p = Polices::du_commun();
    assert_eq!(p.cles().collect::<Vec<_>>(), vec![Cle::POLICE_DU_COMMUN]);
    assert_eq!(p.famille(Cle::POLICE_DU_COMMUN), Some(FAMILLE_DU_COMMUN));
}

#[test]
fn seules_les_polices_sfnt_sont_admises_p22() {
    assert_eq!(valider_police(&woff2()), Err(RefusPolice::Woff2));
    let mut woff = woff2();
    woff[..4].copy_from_slice(b"wOFF");
    assert_eq!(valider_police(&woff), Err(RefusPolice::Woff));
    let mut ttc = woff2();
    ttc[..4].copy_from_slice(b"ttcf");
    assert_eq!(valider_police(&ttc), Err(RefusPolice::Collection));
    assert_eq!(valider_police(&[]), Err(RefusPolice::Vide));
    assert_eq!(
        valider_police(b"PK"),
        Err(RefusPolice::PasSfnt {
            debut: b"PK".to_vec()
        })
    );
    assert!(matches!(
        valider_police(b"\x89PNG\r\n\x1a\n"),
        Err(RefusPolice::PasSfnt { .. })
    ));
    // Une signature sfnt, et rien derrière.
    assert_eq!(
        valider_police(&[0, 1, 0, 0, 0, 0, 0, 0]),
        Err(RefusPolice::Illisible)
    );
    // Le message dit quoi faire.
    assert!(RefusPolice::Woff2.to_string().contains("TTF ou OTF"));
    let mut p = Polices::du_commun();
    assert_eq!(
        p.ajouter(Cle::nommee("woff2"), woff2()),
        Err(RefusPolice::Woff2)
    );
    assert_eq!(p.cles().count(), 1);
}

#[test]
fn deux_polices_homonymes_aux_octets_differents_sont_refusees() {
    let mut p = Polices::du_commun();
    // La même police sous une autre clé : les mêmes octets, rien d'ambigu.
    assert_eq!(
        p.ajouter(Cle::nommee("copie"), OCTETS_DU_COMMUN.to_vec()),
        Ok(())
    );
    // Un octet changé loin des en-têtes : la même famille, d'autres octets.
    let mut autre = OCTETS_DU_COMMUN.to_vec();
    let i = autre.len() / 2;
    autre[i] ^= 0xff;
    assert_eq!(
        p.ajouter(Cle::nommee("autre"), autre),
        Err(RefusPolice::Homonyme {
            autre: Cle::POLICE_DU_COMMUN,
            famille: String::from(FAMILLE_DU_COMMUN),
        })
    );
}

#[test]
fn le_signe_isole_reste_avec_son_mot_sq46() {
    let mut m = mise_en_page();
    let s = style(fixe(10), Lignes::Libre, Coupe::Mots, Alignement::Debut);
    // « Tu fais quoi ? » fait 59,7 unités à la taille 10 ; « Tu fais quoi » tiendrait
    // dans 55, mais pas le point d'interrogation : il part avec son mot.
    let a = m.agencer("Tu fais quoi ?", boite(55, 100), &s);
    assert_eq!(a.tient, Tient::Oui);
    assert_eq!(lignes(&a), vec!["Tu fais", "quoi\u{202f}?"]);
    // Les quatre signes, et l'espace insécable aussi.
    for (texte, agence) in [
        ("Oui !", "Oui\u{202f}!"),
        ("Note : bien", "Note\u{202f}: bien"),
        ("a ; b", "a\u{202f}; b"),
        ("Quoi\u{a0}?", "Quoi\u{202f}?"),
        ("Quoi?", "Quoi?"),
    ] {
        let a = m.agencer(texte, boite(500, 100), &s);
        assert_eq!(a.texte, agence, "pour « {texte} »");
    }
    // Entre deux graphèmes, le texte reste tel quel : la coupe est celle du texte.
    let g = style(fixe(10), Lignes::Libre, Coupe::Graphemes, Alignement::Debut);
    assert_eq!(m.agencer("Oui !", boite(500, 100), &g).texte, "Oui !");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// SQ-46 : en `Coupe::Mots`, aucune ligne ne commence par un signe isolé, quelle que
    /// soit la largeur de la boîte.
    #[test]
    fn aucune_ligne_ne_commence_par_un_signe_isole_sq46(
        mots in prop::collection::vec("[a-zé]{1,9}( [?!:;])?", 1..12),
        largeur in 8_i32..120,
    ) {
        let texte = mots.join(" ");
        let mut m = mise_en_page();
        let s = style(fixe(10), Lignes::Libre, Coupe::Mots, Alignement::Debut);
        let a = m.agencer(&texte, boite(largeur, 1_000), &s);
        for l in lignes(&a) {
            prop_assert!(
                !l.starts_with(['?', '!', ':', ';']),
                "la ligne « {} » commence par un signe isolé ({:?})", l, a.texte
            );
        }
    }
}

#[test]
fn un_texte_trop_long_retrecit_et_rien_n_est_perdu_tl55() {
    let mut mep = mise_en_page();
    let texte = "Une étiquette beaucoup trop longue pour sa case, qui doit pourtant se lire en entier, sans qu'aucune lettre ne disparaisse";
    let st = style(
        Taille::Budget {
            max: Fixe::unites(20),
            min: Fixe::unites(1),
        },
        Lignes::AuPlus(4),
        Coupe::Mots,
        Alignement::Centre,
    );
    let bt = boite(120, 108);
    let ag = mep.agencer(texte, bt, &st);
    assert_eq!(ag.tient, Tient::Oui);
    assert!(
        ag.taille.brut() < Fixe::unites(20).brut(),
        "la police a rétréci"
    );
    assert!(
        lignes(&ag).len() > 1 && lignes(&ag).len() <= 4,
        "la ligne a grandi"
    );
    // Rien n'est coupé : chaque lettre est là, dans l'ordre.
    let lettres = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    assert_eq!(lettres(&ag.texte), lettres(texte));
    // La plus grande taille qui tient, au 1/256 près.
    let a_la = |brut: i32, mep: &mut MiseEnPage| {
        let mut fixee = st;
        fixee.taille = Taille::Fixe(Fixe::depuis_brut(brut));
        mep.agencer(texte, bt, &fixee).tient
    };
    assert_eq!(a_la(ag.taille.brut(), &mut mep), Tient::Oui);
    assert_ne!(
        a_la(ag.taille.brut().saturating_add(1), &mut mep),
        Tient::Oui
    );
}

#[test]
fn une_ligne_ne_coupe_jamais_un_grapheme_tl55() {
    let mut m = mise_en_page();
    // Des graphèmes de plusieurs points de code : une lettre et son accent combinant, un
    // emoji et sa couleur de peau, une famille liée par des U+200D. Aucun n'a de glyphe
    // dans la police du commun (sauf la lettre) : chacun est fait de plusieurs glyphes
    // absents, que la coupe par glyphe de `cosmic-text` séparerait.
    let texte = "e\u{301}e\u{301}e\u{301}e\u{301}👍🏽👨\u{200d}👩\u{200d}👧中文e\u{301}";
    let bords: Vec<usize> = texte
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([texte.len()])
        .collect();
    for largeur in 1..40 {
        let s = style(fixe(10), Lignes::Libre, Coupe::Graphemes, Alignement::Debut);
        let a = m.agencer(texte, boite(largeur, 1_000), &s);
        assert_eq!(a.texte.replace('\n', ""), texte, "largeur {largeur}");
        let mut i = 0;
        for l in lignes(&a) {
            assert!(
                bords.contains(&i),
                "largeur {largeur} : une ligne commence dans un graphème ({:?})",
                a.texte
            );
            i += l.len();
        }
    }
}

#[test]
fn un_texte_qui_ne_tient_pas_meme_au_min_deborde() {
    let mut m = mise_en_page();
    let s = style(
        Taille::Budget {
            max: Fixe::unites(3),
            min: Fixe::unites(1),
        },
        Lignes::Une,
        Coupe::Graphemes,
        Alignement::Centre,
    );
    let a = m.agencer("WWWWWWWWWWWW", boite(6, 1), &s);
    assert_eq!(a.taille, Fixe::unites(1), "mesuré à son min");
    assert!(
        matches!(a.tient, Tient::Non(Debord::TropLarge { .. })),
        "{:?}",
        a.tient
    );
    // Et à 1/2, il tient, en largeur comme en hauteur.
    let mut s2 = s;
    s2.taille = Taille::Budget {
        max: Fixe::unites(3),
        min: Fixe::depuis_brut(128),
    };
    let a = m.agencer("WWWWWWWWWWWW", boite(6, 1), &s2);
    assert_eq!(a.tient, Tient::Oui);
    assert!(a.taille.brut() >= 128 && a.taille.brut() < 256);
    // Une boîte trop basse : une ligne fait 1,2 fois la taille.
    let a = m.agencer(
        "a",
        boite(10, 1),
        &style(fixe(1), Lignes::Une, Coupe::Mots, Alignement::Debut),
    );
    assert!(
        matches!(a.tient, Tient::Non(Debord::TropHaut { .. })),
        "{:?}",
        a.tient
    );
    assert!((a.interligne - INTERLIGNE).abs() < 1e-6);
}

#[test]
fn les_fins_de_ligne_et_les_lignes_permises() {
    let mut m = mise_en_page();
    let une = style(fixe(1), Lignes::Une, Coupe::Mots, Alignement::Debut);
    let a = m.agencer("a\nb", boite(10, 10), &une);
    assert_eq!(
        a.tient,
        Tient::Non(Debord::TropDeLignes {
            lignes: 2,
            permises: 1
        })
    );
    let libre = style(fixe(1), Lignes::Libre, Coupe::Mots, Alignement::Debut);
    for texte in ["a\r\nb", "a\rb", "a\u{2028}b", "a\u{2029}b", "a\u{85}b"] {
        let a = m.agencer(texte, boite(10, 10), &libre);
        assert_eq!(lignes(&a), vec!["a", "b"], "pour {texte:?}");
        assert_eq!(a.lignes.len(), 2);
    }
    let deux = style(fixe(1), Lignes::AuPlus(2), Coupe::Mots, Alignement::Debut);
    assert!(matches!(
        m.agencer("un deux trois quatre", boite(3, 10), &deux).tient,
        Tient::Non(Debord::TropDeLignes { permises: 2, .. })
    ));
    // Les espaces qui finissent une ligne ne comptent pas, et sont retirées.
    let a = m.agencer("abc   ", boite(100, 10), &une);
    let b = m.agencer("abc", boite(100, 10), &une);
    assert_eq!(a.texte, "abc");
    assert_eq!(a.lignes[0].largeur, b.lignes[0].largeur);
    // Un texte vide, ou d'espaces : aucune ligne, et il tient, même dans une boîte nulle.
    for vide in ["", "   ", "\n"] {
        let a = m.agencer(vide, boite(0, 0), &libre);
        assert_eq!(a.tient, Tient::Oui, "pour {vide:?}");
        assert_eq!(a.lignes.len(), 0);
    }
}

#[test]
fn un_style_qui_ne_se_mesure_pas_le_dit() {
    let mut m = mise_en_page();
    let b = boite(10, 10);
    for taille in [
        Taille::Fixe(Fixe::ZERO),
        Taille::Fixe(Fixe::unites(-1)),
        Taille::Budget {
            max: Fixe::unites(1),
            min: Fixe::unites(2),
        },
        Taille::Budget {
            max: Fixe::unites(1),
            min: Fixe::ZERO,
        },
    ] {
        let a = m.agencer(
            "a",
            b,
            &style(taille, Lignes::Une, Coupe::Mots, Alignement::Debut),
        );
        assert!(
            matches!(a.tient, Tient::Non(Debord::StyleInvalide(_))),
            "{taille:?} : {:?}",
            a.tient
        );
    }
}

#[test]
fn un_glyphe_absent_se_compte_et_aucune_police_du_systeme_ne_sert() {
    let mut m = mise_en_page();
    let s = style(fixe(10), Lignes::Une, Coupe::Mots, Alignement::Debut);
    // La machine a des polices CJK ; la mesure ne les voit pas.
    let a = m.agencer("中文", boite(1_000, 100), &s);
    assert_eq!(a.absents, 2);
    assert!(a.lignes[0].largeur > 0.0, "une boîte a une chasse");
    assert_eq!(m.agencer("abc", boite(1_000, 100), &s).absents, 0);
    // Une police inconnue : la police du commun la remplace, et c'est dit.
    let mut inconnue = s;
    inconnue.police = Cle::nommee("jamais déclarée");
    let a = m.agencer("abc", boite(1_000, 100), &inconnue);
    assert_eq!(a.police_inconnue, Oui::Oui);
    assert!(
        a.lignes[0]
            .glyphes
            .iter()
            .all(|g| g.police == Cle::POLICE_DU_COMMUN)
    );
    assert_eq!(
        m.agencer("abc", boite(1_000, 100), &s).police_inconnue,
        Oui::Non
    );
}

#[test]
fn l_alignement_place_les_lignes_dans_la_boite() {
    let mut m = mise_en_page();
    let mut places = Vec::new();
    for alignement in [Alignement::Debut, Alignement::Centre, Alignement::Fin] {
        let a = m.agencer(
            "abc",
            boite(100, 10),
            &style(fixe(10), Lignes::Une, Coupe::Mots, alignement),
        );
        let l = &a.lignes[0];
        places.push((l.gauche, l.gauche + l.largeur));
        // Le bloc d'une ligne se centre dans la hauteur.
        assert!((l.haut - (10.0 - 12.0) / 2.0).abs() < 1e-4, "{}", l.haut);
    }
    assert!(places[0].0.abs() < 1e-4);
    assert!((places[1].0 + places[1].1 - 100.0).abs() < 1e-3);
    assert!((places[2].1 - 100.0).abs() < 1e-3);
}

/// Un style tiré au hasard.
fn un_style() -> impl Strategy<Value = StyleTexte> {
    (
        1_i32..40,
        0_i32..40,
        prop_oneof![
            Just(Lignes::Une),
            (0_u8..5).prop_map(Lignes::AuPlus),
            Just(Lignes::Libre)
        ],
        prop_oneof![Just(Coupe::Mots), Just(Coupe::Graphemes)],
        prop_oneof![
            Just(Alignement::Debut),
            Just(Alignement::Centre),
            Just(Alignement::Fin)
        ],
        any::<bool>(),
    )
        .prop_map(|(min, plus, lignes, coupe, alignement, budget)| {
            let min = Fixe::depuis_brut(min.saturating_mul(32));
            let taille = if budget {
                Taille::Budget {
                    max: Fixe::depuis_brut(min.brut().saturating_add(plus.saturating_mul(64))),
                    min,
                }
            } else {
                Taille::Fixe(min)
            };
            style(taille, lignes, coupe, alignement)
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// PUN-53 : le cadre est la boîte du texte. Un texte qui tient a chaque glyphe et
    /// chaque ligne dans sa boîte ; un texte qui ne tient pas le dit, avec la vraie
    /// raison. Et l'agencement ne dépend que du texte, de la boîte et du style : une
    /// mise en page neuve, sans rien de gardé, rend le même.
    #[test]
    fn l_agencement_tient_dans_sa_boite_ou_le_dit_pun53(
        texte in "([a-zA-ZéÉçœ' ,.?!:;0-9]|W|中|👍🏽|e\u{301}|\n){0,60}",
        l in 0_i32..60,
        h in 0_i32..20,
        s in un_style(),
    ) {
        let mut m = mise_en_page();
        let b = boite(l, h);
        let a = m.agencer(&texte, b, &s);
        prop_assert_eq!(&a, &mise_en_page().agencer(&texte, b, &s));
        prop_assert_eq!(&a, &m.agencer(&texte, b, &s));
        let (lf, hf) = (f32::from(u16::try_from(l).unwrap()), f32::from(u16::try_from(h).unwrap()));
        let eps = 1e-3_f32;
        let permises = match s.lignes {
            Lignes::Une => 1,
            Lignes::AuPlus(n) => usize::from(n),
            Lignes::Libre => usize::MAX,
        };
        let plus_large = a.lignes.iter().map(|x| x.largeur).fold(0.0_f32, |x, y| if y > x { y } else { x });
        let hauteur = f32::from(u16::try_from(a.lignes.len()).unwrap()) * a.interligne;
        match &a.tient {
            Tient::Oui => {
                prop_assert!(a.lignes.len() <= permises);
                for ligne in &a.lignes {
                    prop_assert!(ligne.haut >= -eps && ligne.haut + a.interligne <= hf + eps);
                    for g in &ligne.glyphes {
                        prop_assert!(g.x >= -eps && g.x + g.chasse <= lf + eps, "{:?} hors de {}", g, lf);
                    }
                }
            }
            Tient::Non(Debord::TropDeLignes { lignes, permises: p }) => {
                prop_assert!(usize::try_from(*lignes).unwrap() > permises.min(usize::try_from(*p).unwrap_or(usize::MAX)));
            }
            Tient::Non(Debord::TropLarge { largeur, .. }) => {
                prop_assert!(*largeur > lf);
                prop_assert!((plus_large - largeur).abs() < eps);
            }
            Tient::Non(Debord::TropHaut { hauteur: dite, .. }) => {
                prop_assert!(*dite > hf);
                prop_assert!((hauteur - dite).abs() < eps);
            }
            Tient::Non(Debord::StyleInvalide(_)) => prop_assert!(false, "le style tiré est valide"),
        }
        // Une taille qui rétrécit ne s'arrête jamais au-dessus de ce qui tient, ni
        // ailleurs qu'entre ses bornes.
        if let Taille::Budget { max, min } = s.taille {
            prop_assert!(a.taille.brut() >= min.brut() && a.taille.brut() <= max.brut());
            if a.tient != Tient::Oui {
                prop_assert_eq!(a.taille, min);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// La télé dessine exactement l'agencement mesuré (PHASE-3, § 2.9, lot 9) : le tampon
    /// qu'elle donne à `glyphon` porte, glyphe par glyphe et au bit près, les lignes de
    /// l'agencement, quel que soit le texte, la boîte ou le style.
    #[test]
    fn le_tampon_du_dessin_porte_l_agencement(
        texte in "([a-zA-ZéÉçœ' ,.?!:;0-9]|W|中|👍🏽|e\u{301}|\n){0,60}",
        l in 0_i32..60,
        h in 0_i32..20,
        s in un_style(),
    ) {
        let mut m = mise_en_page();
        let b = boite(l, h);
        let a = m.agencer(&texte, b, &s);
        let tampon = m.tampon(&a, b, &s);
        prop_assert_eq!(m.verifier_tampon(&a, &tampon), Ok(()));
    }
}

#[test]
fn un_tampon_d_un_autre_texte_est_refuse() {
    let mut m = mise_en_page();
    let s = style(fixe(10), Lignes::Libre, Coupe::Mots, Alignement::Centre);
    let a = m.agencer("Bonjour à tous", boite(200, 40), &s);
    let b = m.agencer("Bonjour à tout", boite(200, 40), &s);
    let tampon = m.tampon(&b, boite(200, 40), &s);
    assert!(m.verifier_tampon(&a, &tampon).is_err());
    // Une autre boîte déplace les glyphes centrés : l'écart se voit aussi.
    let tampon = m.tampon(&a, boite(201, 40), &s);
    assert!(m.verifier_tampon(&a, &tampon).is_err());
}

#[test]
fn les_octets_canoniques_distinguent_au_bit_pres() {
    let mut mep = mise_en_page();
    let st = style(fixe(10), Lignes::Libre, Coupe::Mots, Alignement::Debut);
    let a = mep.agencer("Égalité : 12 points", boite(200, 40), &st);
    assert_eq!(
        a.octets_canoniques(),
        mise_en_page()
            .agencer("Égalité : 12 points", boite(200, 40), &st)
            .octets_canoniques()
    );
    let mut b = a.clone();
    let glyphe = &mut b.lignes[0].glyphes[3];
    glyphe.x = f32::from_bits(glyphe.x.to_bits() ^ 1);
    assert_ne!(a.octets_canoniques(), b.octets_canoniques());
    let mut c = a.clone();
    c.absents += 1;
    assert_ne!(a.octets_canoniques(), c.octets_canoniques());
}

#[test]
fn le_nom_d_un_bouton_tient_dans_sa_boite() {
    use mise_en_page::bouton::{boite_du_nom, style_du_nom};
    let b = Rect {
        x: Fixe::unites(10),
        y: Fixe::unites(20),
        l: Fixe::unites(200),
        h: Fixe::unites(50),
    };
    let n = boite_du_nom(b);
    assert_eq!(
        (n.x, n.y, n.l, n.h),
        (
            Fixe::unites(20),
            Fixe::unites(30),
            Fixe::unites(180),
            Fixe::unites(30)
        )
    );
    let s = style_du_nom(b);
    let a = mise_en_page().agencer("Valider", n, &s);
    assert_eq!(a.tient, Tient::Oui);
    // Une ligne pleine : la taille la plus grande, 5/6 de la boîte du nom (à 1/256
    // près : 25 × 1,2 n'est pas exactement 30 en flottant).
    assert!(
        a.taille.brut() >= Fixe::unites(25).brut() - 1 && a.taille <= Fixe::unites(25),
        "{:?}",
        a.taille
    );
    // Une boîte nulle ne panique pas.
    let vide = Rect {
        x: Fixe::ZERO,
        y: Fixe::ZERO,
        l: Fixe::ZERO,
        h: Fixe::ZERO,
    };
    let _: Agencement = mise_en_page().agencer("x", boite_du_nom(vide), &style_du_nom(vide));
}
