//! La page du téléphone, sur le fil (PHASE-3, § 2.10 ; ABI 4) : sa structure au
//! décodage, et sa validation comme une entrée venue de l'extérieur (REM-57).

use jeu_abi::{
    AssetFil, Clavier, Cle, Declencheur, DefautPage, Directions, Illisible, Limites, NoeudPage,
    PageFil, RETENU_MAX, Rgba, SorteAsset, SorteNoeudPage, StylePage, TEXTE_PAGE_MAX, TaillePage,
    TenuPage, Unite, decoder, encoder, valider_page,
};
use proptest::prelude::*;

const LIMITES: Limites = Limites {
    geste: 8,
    file: 32,
    vue: 4096,
    effets: 8,
    memoire: 64,
    brouillon: 1 << 20,
};

const IMAGE: Cle = Cle::du_fil(100);
const POLICE: Cle = Cle::du_fil(101);

fn assets() -> Vec<AssetFil> {
    vec![
        AssetFil {
            cle: IMAGE,
            sorte: SorteAsset::Image,
            taille: 10,
        },
        AssetFil {
            cle: POLICE,
            sorte: SorteAsset::Police,
            taille: 10,
        },
    ]
}

fn noeud(cle: u64, parent: Option<u32>, sorte: SorteNoeudPage) -> NoeudPage {
    NoeudPage {
        cle: Cle::du_fil(cle),
        parent,
        sorte,
    }
}

/// Une page qui a de chaque sorte de nœud, valide.
fn complete() -> PageFil {
    PageFil {
        noeuds: vec![
            noeud(1, None, SorteNoeudPage::Colonne),
            noeud(2, Some(0), SorteNoeudPage::Rangee),
            noeud(3, Some(1), SorteNoeudPage::Grille { colonnes: 3 }),
            noeud(4, Some(0), SorteNoeudPage::Pile),
            noeud(
                5,
                Some(3),
                SorteNoeudPage::Image {
                    image: IMAGE,
                    hauteur: TaillePage::Hauteur(40),
                    teinte: Some(Rgba(1, 2, 3, 255)),
                },
            ),
            noeud(
                6,
                Some(0),
                SorteNoeudPage::Texte {
                    texte: String::from("Bonjour"),
                    style: StylePage {
                        police: POLICE,
                        ..StylePage::ORDINAIRE
                    },
                },
            ),
            noeud(
                7,
                Some(0),
                SorteNoeudPage::Bouton {
                    nom: String::from("Vas-y"),
                    geste: vec![1, 2],
                    quand: Declencheur::Contact,
                    style: StylePage::ORDINAIRE,
                },
            ),
            noeud(
                8,
                Some(0),
                SorteNoeudPage::Maintenu {
                    nom: String::from("Saute"),
                    style: StylePage::ORDINAIRE,
                },
            ),
            noeud(
                9,
                Some(0),
                SorteNoeudPage::Croix {
                    directions: Directions::Huit,
                },
            ),
            noeud(
                10,
                Some(0),
                SorteNoeudPage::Champ {
                    limite: 18,
                    unite: Unite::Utf16,
                    exemple: String::from("Ton prénom"),
                    clavier: Clavier::Prenom,
                    entree: Some(vec![3]),
                },
            ),
            noeud(
                11,
                Some(0),
                SorteNoeudPage::Choix {
                    options: vec![
                        (Cle::du_fil(1), String::from("Un")),
                        (Cle::du_fil(2), String::from("Deux")),
                    ],
                    balayable: jeu_abi::Oui::Oui,
                },
            ),
            // Une clé déjà vue, mais sous un autre parent : permis (SAL-165).
            noeud(5, Some(0), SorteNoeudPage::Colonne),
        ],
        tenu: Some(TenuPage {
            octets: vec![1],
            relache: Some(vec![0]),
        }),
        retenir: Some(vec![0; RETENU_MAX]),
    }
}

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    o
}

#[test]
fn une_page_vide_et_une_page_complete_sont_valides() {
    assert_eq!(valider_page(&PageFil::vide(), &[], &LIMITES), Ok(()));
    let p = complete();
    assert_eq!(valider_page(&p, &assets(), &LIMITES), Ok(()));
    // Elle se relit telle quelle.
    assert_eq!(decoder::<PageFil>(&encode(&p)), Ok(p));
}

/// La liste à plat ne se décode que si chaque parent précède son enfant et le contient :
/// décoder une page ne peut pas faire déborder la pile.
#[test]
fn le_decodage_refuse_un_parent_qui_ne_contient_rien_ou_vient_apres() {
    let mut p = complete();
    // Le texte (indice 5) pris pour parent.
    p.noeuds.push(noeud(99, Some(5), SorteNoeudPage::Colonne));
    assert_eq!(
        decoder::<PageFil>(&encode(&p)).map_err(|e| e.quoi),
        Err(Illisible::Valeur)
    );
    let mut p = complete();
    p.noeuds.push(noeud(99, Some(400), SorteNoeudPage::Colonne));
    assert_eq!(
        decoder::<PageFil>(&encode(&p)).map_err(|e| e.quoi),
        Err(Illisible::Valeur)
    );
}

fn juger(p: &PageFil) -> Result<(), DefautPage> {
    valider_page(p, &assets(), &LIMITES)
}

fn changer(i: usize, s: SorteNoeudPage) -> PageFil {
    let mut p = complete();
    p.noeuds[i].sorte = s;
    p
}

/// REM-57 : ce que décrit le jeu est borné avant d'atteindre le moteur d'affichage.
/// Les bornes de la page entière, une à une, sur une page autrement valide.
#[test]
fn les_bornes_de_la_page_se_nomment_rem57() {
    // Trop de nœuds.
    let mut p = PageFil::vide();
    p.noeuds.push(noeud(0, None, SorteNoeudPage::Colonne));
    for k in 1..=jeu_abi::NOEUDS_PAGE_MAX as u64 {
        p.noeuds.push(noeud(k, Some(0), SorteNoeudPage::Colonne));
    }
    assert!(matches!(juger(&p), Err(DefautPage::TropDeNoeuds { .. })));
    // Trop profonde.
    let mut p = PageFil::vide();
    for k in 0..=u32::try_from(jeu_abi::PROFONDEUR_PAGE_MAX).unwrap() {
        p.noeuds.push(noeud(
            u64::from(k),
            k.checked_sub(1),
            SorteNoeudPage::Colonne,
        ));
    }
    assert!(matches!(juger(&p), Err(DefautPage::TropProfonde { .. })));
    // Deux frères de même clé (SAL-165).
    let mut p = complete();
    p.noeuds.push(noeud(6, Some(0), SorteNoeudPage::Rangee));
    assert_eq!(
        juger(&p),
        Err(DefautPage::CleEnDouble {
            cle: Cle::du_fil(6)
        })
    );
    // Un tenu trop gros, ou sa valeur relâchée.
    let mut p = complete();
    p.tenu = Some(TenuPage {
        octets: vec![0; 9],
        relache: None,
    });
    assert_eq!(juger(&p), Err(DefautPage::TenuHorsBornes));
    let mut p = complete();
    p.tenu = Some(TenuPage {
        octets: vec![0],
        relache: Some(vec![0; 9]),
    });
    assert_eq!(juger(&p), Err(DefautPage::TenuHorsBornes));
    // Ce que la page retient, au-delà d'un Kio.
    let mut p = complete();
    p.retenir = Some(vec![0; RETENU_MAX + 1]);
    assert_eq!(
        juger(&p),
        Err(DefautPage::RetenuTropGros {
            octets: RETENU_MAX + 1
        })
    );
    // Chaque défaut se dit.
    assert_ne!(DefautPage::TenuHorsBornes.to_string(), "");
}

/// REM-57 : ce que porte chaque nœud est borné ; chaque défaut, un à un.
#[test]
fn chaque_noeud_porte_ses_bornes_rem57() {
    // Un texte trop long.
    let long = "a".repeat(TEXTE_PAGE_MAX + 1);
    let p = changer(
        5,
        SorteNoeudPage::Texte {
            texte: long.clone(),
            style: StylePage::ORDINAIRE,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::TexteTropLong { .. })));
    // Une grille sans colonne, une image ou un texte hors de leurs tailles.
    let p = changer(2, SorteNoeudPage::Grille { colonnes: 0 });
    assert!(matches!(juger(&p), Err(DefautPage::HorsBornes { .. })));
    let p = changer(
        4,
        SorteNoeudPage::Image {
            image: IMAGE,
            hauteur: TaillePage::Hauteur(101),
            teinte: None,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::HorsBornes { .. })));
    let p = changer(
        5,
        SorteNoeudPage::Texte {
            texte: String::new(),
            style: StylePage {
                taille: TaillePage::Px(4),
                ..StylePage::ORDINAIRE
            },
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::HorsBornes { .. })));
    // Une image non déclarée, ou déclarée comme police ; une police inconnue.
    let p = changer(
        4,
        SorteNoeudPage::Image {
            image: POLICE,
            hauteur: TaillePage::Px(64),
            teinte: None,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::AssetInconnu { .. })));
    let p = changer(
        5,
        SorteNoeudPage::Texte {
            texte: String::new(),
            style: StylePage {
                police: IMAGE,
                ..StylePage::ORDINAIRE
            },
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::AssetInconnu { .. })));
    // Un geste au-delà de `limites.geste`.
    let p = changer(
        6,
        SorteNoeudPage::Bouton {
            nom: String::new(),
            geste: vec![0; 9],
            quand: Declencheur::Relache,
            style: StylePage::ORDINAIRE,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::GesteHorsBornes { .. })));
    // Un champ sans limite, un choix sans option ou à deux options de même clé.
    let p = changer(
        9,
        SorteNoeudPage::Champ {
            limite: 0,
            unite: Unite::Graphemes,
            exemple: String::new(),
            clavier: Clavier::Texte,
            entree: None,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::HorsBornes { .. })));
    let p = changer(
        10,
        SorteNoeudPage::Choix {
            options: vec![],
            balayable: jeu_abi::Oui::Non,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::HorsBornes { .. })));
    let p = changer(
        10,
        SorteNoeudPage::Choix {
            options: vec![
                (Cle::du_fil(1), String::new()),
                (Cle::du_fil(1), String::new()),
            ],
            balayable: jeu_abi::Oui::Non,
        },
    );
    assert!(matches!(juger(&p), Err(DefautPage::CleEnDouble { .. })));
}

proptest! {
    /// REM-64 : des octets au hasard ne font jamais paniquer le décodage d'une page, ni
    /// sa validation quand ils se décodent.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_la_page_rem64(
        octets in proptest::collection::vec(any::<u8>(), 0..256)
    ) {
        if let Ok(p) = decoder::<PageFil>(&octets) {
            let _: Result<(), DefautPage> = valider_page(&p, &assets(), &LIMITES);
        }
    }

    /// Une page valide le reste si l'on retire une feuille (la dernière).
    #[test]
    fn retirer_la_derniere_feuille_garde_la_page_valide(n in 1usize..12) {
        let mut p = complete();
        p.noeuds.truncate(n);
        prop_assert_eq!(valider_page(&p, &assets(), &LIMITES), Ok(()));
    }
}
