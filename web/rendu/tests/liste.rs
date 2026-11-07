//! La scène mise à plat, sans GPU : le cadre, les couches, `pixel_entier`, l'opacité qui
//! se multiplie, ce qui ne se dessine pas.

pub mod commun;

use commun::{forme, noeud, rect, scene};
use jeu_abi::{Cle, Fixe, FormeEcran, Oui, Rgba, SorteNoeud};
use rendu::{Affine, Cadre, Contexte, Sorte, mettre_a_plat};

#[test]
fn le_cadre_d_une_scene_et_d_une_page() {
    let zone = FormeEcran::Scene {
        largeur: 1600,
        hauteur: 900,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Non,
    };
    let a = Cadre::new(1280, 720, zone).transformation();
    assert_eq!(
        a,
        Affine {
            echelle: 0.72,
            x: 64.0,
            y: 36.0
        }
    );
    let entiere = FormeEcran::Scene {
        largeur: 640,
        hauteur: 360,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Oui,
    };
    // 90 % de 1920 × 1080 : 2,7 fois, ramené à 2 ; sous 1, l'échelle reste libre.
    assert_eq!(
        Cadre::new(1920, 1080, entiere).transformation().echelle,
        2.0
    );
    assert_eq!(Cadre::new(320, 180, entiere).transformation().echelle, 0.45);
    let mut page = Cadre::new(1280, 720, FormeEcran::Page { largeur_max: 1120 });
    page.echelle_page = 2.0;
    page.defilement = 10.0;
    // 640 unités tiennent : tout le contenu fait 640 unités, à 0.
    assert_eq!(
        page.transformation(),
        Affine {
            echelle: 2.0,
            x: 0.0,
            y: -20.0
        }
    );
}

#[test]
fn pixel_entier_pose_l_origine_sur_un_pixel_sq52() {
    let mut n = noeud(
        1,
        None,
        SorteNoeud::Groupe {
            decalage: (Fixe::depuis_brut(10 * 256 + 77), Fixe::ZERO),
        },
    );
    n.pixel_entier = Oui::Oui;
    let s = scene(
        Rgba::NOIR,
        vec![
            n,
            noeud(
                2,
                Some(0),
                SorteNoeud::Image {
                    image: Cle::du_fil(1),
                    r: rect(0, 0, 3, 3),
                    ech: jeu_abi::Echantillonnage::PlusProche,
                },
            ),
        ],
    );
    let l = mettre_a_plat(
        &s,
        &Cadre::new(100, 100, forme(100, 100)),
        &Contexte::au_repos(),
    );
    assert_eq!(l.elements[0].boite, [10.0, 0.0, 13.0, 3.0]);
}

#[test]
fn l_opacite_se_multiplie_et_les_composants_ne_se_dessinent_pas() {
    let mut g = noeud(
        1,
        None,
        SorteNoeud::Groupe {
            decalage: (Fixe::ZERO, Fixe::ZERO),
        },
    );
    g.opacite = 128;
    let mut r = noeud(
        2,
        Some(0),
        SorteNoeud::Rect {
            r: rect(0, 0, 10, 10),
            couleur: Rgba(255, 255, 255, 255),
        },
    );
    r.opacite = 128;
    let s = scene(
        Rgba::NOIR,
        vec![
            g,
            r,
            noeud(
                3,
                None,
                SorteNoeud::Surface {
                    r: rect(0, 0, 10, 10),
                },
            ),
            noeud(
                4,
                None,
                SorteNoeud::Avertissements {
                    boite: rect(0, 0, 10, 10),
                },
            ),
        ],
    );
    let l = mettre_a_plat(
        &s,
        &Cadre::new(10, 10, forme(10, 10)),
        &Contexte::au_repos(),
    );
    assert_eq!(l.elements.len(), 1);
    let Sorte::Quad { couleur, .. } = &l.elements[0].sorte else {
        panic!()
    };
    let attendu = 128.0 / 255.0 * 128.0 / 255.0;
    assert!((couleur[3] - attendu).abs() < 1e-6);
}

#[test]
fn un_quad_monte_au_dessus_d_un_texte_qu_il_recouvre() {
    let style = jeu_abi::StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: jeu_abi::Taille::Fixe(Fixe::unites(10)),
        couleur: Rgba::BLANC,
        alignement: jeu_abi::Alignement::Debut,
        lignes: jeu_abi::Lignes::Une,
        coupe: jeu_abi::Coupe::Mots,
    };
    let s = scene(
        Rgba::NOIR,
        vec![
            noeud(
                1,
                None,
                SorteNoeud::Rect {
                    r: rect(0, 0, 50, 50),
                    couleur: Rgba::BLANC,
                },
            ),
            noeud(
                2,
                None,
                SorteNoeud::Texte {
                    texte: String::from("a"),
                    boite: rect(0, 0, 50, 20),
                    style,
                },
            ),
            noeud(
                3,
                None,
                SorteNoeud::Rect {
                    r: rect(60, 0, 10, 10),
                    couleur: Rgba::BLANC,
                },
            ),
            noeud(
                4,
                None,
                SorteNoeud::Rect {
                    r: rect(10, 10, 10, 10),
                    couleur: Rgba::BLANC,
                },
            ),
            noeud(
                5,
                None,
                SorteNoeud::Texte {
                    texte: String::from("b"),
                    boite: rect(10, 10, 5, 5),
                    style,
                },
            ),
        ],
    );
    let l = mettre_a_plat(
        &s,
        &Cadre::new(100, 100, forme(100, 100)),
        &Contexte::au_repos(),
    );
    let couches: Vec<u32> = l.elements.iter().map(|e| e.couche).collect();
    // Le rectangle qui ne recouvre aucun texte reste en bas ; celui qui recouvre « a »
    // monte, et « b », par-dessus, le suit.
    assert_eq!(couches, [0, 0, 0, 1, 1]);
    assert_eq!(l.couches, 2);
}

/// TL-47 (lot 11) : pendant un glisser, l'objet tenu reste à sa place à 20 %, une copie à
/// 80 % suit le pointeur, par-dessus tout, et la marque de la place visée se dessine ;
/// sans glisser, rien de cela.
#[test]
fn un_glisser_dessine_l_objet_attenue_sa_copie_et_la_marque_tl47() {
    let blanc = Rgba(255, 255, 255, 255);
    let s = scene(
        Rgba::NOIR,
        vec![
            noeud(
                1,
                None,
                SorteNoeud::Rect {
                    r: rect(0, 0, 10, 10),
                    couleur: blanc,
                },
            ),
            noeud(
                2,
                None,
                SorteNoeud::Rect {
                    r: rect(20, 0, 10, 10),
                    couleur: blanc,
                },
            ),
        ],
    );
    let cadre = Cadre::new(100, 100, forme(100, 100));
    let repos = mettre_a_plat(&s, &cadre, &Contexte::au_repos());
    assert_eq!(repos.elements.len(), 2);
    let contexte = Contexte {
        glisser: Some(rendu::liste::GlisserVu {
            objet: Cle::du_fil(1),
            decalage: (50.0, 40.0),
            marque: Some([30.0, 0.0, 32.0, 10.0]),
        }),
        ..Contexte::au_repos()
    };
    let l = mettre_a_plat(&s, &cadre, &contexte);
    assert_eq!(l.elements.len(), 4, "{l:?}");
    let alpha = |k: usize| match &l.elements[k].sorte {
        Sorte::Quad { couleur, .. } => couleur[3],
        Sorte::Texte { opacite, .. } => *opacite,
    };
    // L'objet à sa place, atténué ; l'autre, intact.
    assert_eq!(l.elements[0].boite, [0.0, 0.0, 10.0, 10.0]);
    assert!((alpha(0) - rendu::liste::OPACITE_DE_L_OBJET_TENU).abs() < 1e-6);
    assert!((alpha(1) - 1.0).abs() < 1e-6);
    // La marque, puis la copie, à la fin de l'ordre du peintre.
    assert_eq!(l.elements[2].boite, [30.0, 0.0, 32.0, 10.0]);
    assert_eq!(l.elements[3].boite, [50.0, 40.0, 60.0, 50.0]);
    assert!((alpha(3) - rendu::liste::OPACITE_DE_LA_COPIE).abs() < 1e-6);
    assert!(l.elements[3].couche >= l.elements[1].couche);
}
