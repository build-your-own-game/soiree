//! Le rendu aux pixels près, en natif sur Metal (PHASE-3, lot 9) : la scène dessinée dans
//! la cible et relue pixel par pixel, comparée à ce que la scène demande.
//!
//! Les scènes sont choisies pour tomber sur des pixels entiers (une échelle de 2) : chaque
//! pixel de la cible a une seule valeur juste, exacte pour un rectangle opaque, à 2 sur
//! 255 près par canal (la tolérance choisie du lot 9) pour un mélange.

#![expect(
    clippy::many_single_char_names,
    clippy::arithmetic_side_effects,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

pub mod commun;

use commun::{dessiner, forme, noeud, proches, rect, relire, rendu, scene};
use jeu_abi::{
    Alignement, Animation, Cle, Coupe, Courbe, Echantillonnage, Fixe, FormeEcran, Instant, Lignes,
    Oui, Pas, Rgba, SorteNoeud, StyleTexte, Taille, Visuel,
};
use mise_en_page::bouton;
use rendu::{Cadre, Contexte, Fautif, ImageRgba, mettre_a_plat};

const FOND: Rgba = Rgba(10, 20, 30, 255);
const ROUGE: Rgba = Rgba(220, 30, 40, 255);
const VERT: Rgba = Rgba(20, 200, 60, 255);
const BLEU: Rgba = Rgba(30, 60, 230, 255);

/// L'image attendue : le fond, puis chaque rectangle (en pixels, bords entiers), opaque
/// ou mélangé par « source plus destination fois (1 − alpha) ».
fn attendue(l: u32, h: u32, fond: Rgba, rects: &[([u32; 4], Rgba, u8)]) -> Vec<[u8; 4]> {
    let mut px = vec![[fond.0, fond.1, fond.2, fond.3]; (l * h) as usize];
    for (r, c, opacite) in rects {
        let a = f32::from(c.3) / 255.0 * f32::from(*opacite) / 255.0;
        for y in r[1]..r[3] {
            for x in r[0]..r[2] {
                let p = &mut px[(y * l + x) as usize];
                let m = |s: u8, d: u8| (f32::from(s) * a + f32::from(d) * (1.0 - a)).round() as u8;
                *p = [
                    m(c.0, p[0]),
                    m(c.1, p[1]),
                    m(c.2, p[2]),
                    (255.0 * a + f32::from(p[3]) * (1.0 - a)).round() as u8,
                ];
            }
        }
    }
    px
}

/// Compare chaque pixel ; rend le premier écart.
fn comparer(image: &ImageRgba, attendue: &[[u8; 4]], tolerance: u8) -> Result<(), String> {
    for y in 0..image.hauteur {
        for x in 0..image.largeur {
            let lu = image.pixel(x, y).unwrap();
            let voulu = attendue[(y * image.largeur + x) as usize];
            if !proches(lu, voulu, tolerance) {
                return Err(format!("pixel ({x}, {y}) : lu {lu:?}, attendu {voulu:?}"));
            }
        }
    }
    Ok(())
}

/// Une scène de rectangles et de groupes, sur un repère de 160 × 90 dessiné à 320 × 180.
fn scene_de_rectangles() -> jeu_abi::SceneFil {
    let mut transparent = noeud(
        5,
        Some(1),
        SorteNoeud::Rect {
            r: rect(20, 0, 30, 10),
            couleur: BLEU,
        },
    );
    transparent.opacite = 128;
    scene(
        FOND,
        vec![
            noeud(
                1,
                None,
                SorteNoeud::Rect {
                    r: rect(10, 10, 40, 20),
                    couleur: ROUGE,
                },
            ),
            noeud(
                2,
                None,
                SorteNoeud::Groupe {
                    decalage: (Fixe::unites(50), Fixe::unites(30)),
                },
            ),
            noeud(
                3,
                Some(1),
                SorteNoeud::Rect {
                    r: rect(0, 0, 30, 30),
                    couleur: VERT,
                },
            ),
            noeud(
                4,
                Some(1),
                SorteNoeud::Rect {
                    r: rect(-45, -25, 10, 10),
                    couleur: BLEU,
                },
            ),
            transparent,
        ],
    )
}

#[test]
fn les_rectangles_et_les_groupes_tombent_au_pixel_pres() {
    let mut r = rendu();
    let cadre = Cadre::new(320, 180, forme(160, 90));
    // Sans le dernier nœud, à moitié opaque : tout est exact.
    let mut s = scene_de_rectangles();
    let _: Option<jeu_abi::NoeudFil> = s.noeuds.pop();
    let image = dessiner(&mut r, &s, &cadre, &Contexte::au_repos());
    assert_eq!((image.largeur, image.hauteur), (320, 180));
    let opaques = [
        ([20, 20, 100, 60], ROUGE, 255),
        ([100, 60, 160, 120], VERT, 255),
        ([10, 10, 30, 30], BLEU, 255),
    ];
    comparer(&image, &attendue(320, 180, FOND, &opaques), 0).unwrap();
}

#[test]
fn l_opacite_se_melange_a_deux_pres() {
    let mut r = rendu();
    let cadre = Cadre::new(320, 180, forme(160, 90));
    let image = dessiner(
        &mut r,
        &scene_de_rectangles(),
        &cadre,
        &Contexte::au_repos(),
    );
    // Le cinquième nœud, dans le groupe (50, 30) : (70, 30) à (100, 40) unités, à moitié
    // opaque, par-dessus le fond et le vert.
    let tous = [
        ([20, 20, 100, 60], ROUGE, 255),
        ([100, 60, 160, 120], VERT, 255),
        ([10, 10, 30, 30], BLEU, 255),
        ([140, 60, 200, 80], BLEU, 128),
    ];
    comparer(&image, &attendue(320, 180, FOND, &tous), 2).unwrap();
}

#[test]
fn la_zone_sure_et_l_echelle_entiere_placent_la_scene() {
    let mut r = rendu();
    let s = scene(
        FOND,
        vec![noeud(
            1,
            None,
            SorteNoeud::Rect {
                r: rect(0, 0, 100, 50),
                couleur: ROUGE,
            },
        )],
    );
    // 90 % de 300 × 200 : 270 × 180 ; 2,7 fois, ramené à 2 ; centré : (50, 50).
    let entiere = FormeEcran::Scene {
        largeur: 100,
        hauteur: 50,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Oui,
    };
    let image = dessiner(
        &mut r,
        &s,
        &Cadre::new(300, 200, entiere),
        &Contexte::au_repos(),
    );
    comparer(
        &image,
        &attendue(300, 200, FOND, &[([50, 50, 250, 150], ROUGE, 255)]),
        0,
    )
    .unwrap();
    // Sans échelle entière : 2,7 fois, (15, 33) à (285, 168).
    let libre = FormeEcran::Scene {
        largeur: 100,
        hauteur: 50,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Non,
    };
    let image = dessiner(
        &mut r,
        &s,
        &Cadre::new(300, 200, libre),
        &Contexte::au_repos(),
    );
    comparer(
        &image,
        &attendue(300, 200, FOND, &[([15, 33, 285, 168], ROUGE, 255)]),
        0,
    )
    .unwrap();
}

#[test]
fn une_page_se_centre_et_defile() {
    let mut r = rendu();
    let s = scene(
        FOND,
        vec![noeud(
            1,
            None,
            SorteNoeud::Rect {
                r: rect(0, 0, 100, 300),
                couleur: VERT,
            },
        )],
    );
    let mut cadre = Cadre::new(300, 100, FormeEcran::Page { largeur_max: 200 });
    cadre.defilement = 250.0;
    let image = dessiner(&mut r, &s, &cadre, &Contexte::au_repos());
    // Contenu de 200 unités centré dans 300 : à 50 ; 250 unités défilées : la fin du
    // rectangle, de 0 à 50.
    comparer(
        &image,
        &attendue(300, 100, FOND, &[([50, 0, 150, 50], VERT, 255)]),
        0,
    )
    .unwrap();
}

#[test]
fn une_image_se_dessine_au_plus_proche() {
    let mut r = rendu();
    let cle = Cle::du_fil(0xabc);
    r.poser_image(
        cle,
        ImageRgba {
            largeur: 2,
            hauteur: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        },
    );
    let s = scene(
        FOND,
        vec![
            noeud(
                1,
                None,
                SorteNoeud::Image {
                    image: cle,
                    r: rect(10, 10, 20, 20),
                    ech: Echantillonnage::PlusProche,
                },
            ),
            noeud(
                2,
                None,
                SorteNoeud::Image {
                    image: Cle::du_fil(0xdef),
                    r: rect(40, 10, 10, 10),
                    ech: Echantillonnage::Lisse,
                },
            ),
        ],
    );
    let cadre = Cadre::new(160, 90, forme(80, 45));
    let d = r.dessiner(&s, &cadre, &Contexte::au_repos()).unwrap();
    assert_eq!(d.images_absentes, 1);
    let image = relire(&r);
    let quarts = [
        ([20, 20, 40, 40], Rgba(255, 0, 0, 255), 255),
        ([40, 20, 60, 40], Rgba(0, 255, 0, 255), 255),
        ([20, 40, 40, 60], Rgba(0, 0, 255, 255), 255),
        ([40, 40, 60, 60], Rgba(255, 255, 255, 255), 255),
        ([80, 20, 100, 40], rendu::IMAGE_ABSENTE, 255),
    ];
    comparer(&image, &attendue(160, 90, FOND, &quarts), 0).unwrap();
}

fn style(taille: i32) -> StyleTexte {
    StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: Taille::Fixe(Fixe::unites(taille)),
        couleur: Rgba::BLANC,
        alignement: Alignement::Centre,
        lignes: Lignes::Libre,
        coupe: Coupe::Mots,
    }
}

#[test]
fn un_texte_se_dessine_dans_sa_boite_et_porte_son_agencement() {
    let mut r = rendu();
    let boite = rect(20, 10, 120, 40);
    let s = scene(
        FOND,
        vec![noeud(
            1,
            None,
            SorteNoeud::Texte {
                texte: String::from("Bonjour à tous, 12 points !"),
                boite,
                style: style(12),
            },
        )],
    );
    let cadre = Cadre::new(320, 180, forme(160, 90));
    let d = r.dessiner(&s, &cadre, &Contexte::au_repos()).unwrap();
    assert_eq!((d.textes, d.glyphes_absents), (1, 0));
    let image = relire(&r);
    // L'encre est dans la boîte (en pixels : 40, 20 à 280, 100), et rien n'en sort.
    let mut encre = 0;
    for y in 0..180 {
        for x in 0..320 {
            let p = image.pixel(x, y).unwrap();
            let dedans = (40..280).contains(&x) && (20..100).contains(&y);
            if p != [FOND.0, FOND.1, FOND.2, FOND.3] {
                assert!(dedans, "encre hors de la boîte en ({x}, {y}) : {p:?}");
                encre += 1;
            }
        }
    }
    assert!(encre > 200, "trop peu d'encre : {encre} pixels");
    // Le tampon que glyphon a dessiné porte l'agencement, glyphe par glyphe.
    let liste = mettre_a_plat(&s, &cadre, &Contexte::au_repos());
    let textes = r.textes(&liste);
    assert_eq!(textes.len(), 1);
    assert_eq!(
        r.mise_en_page()
            .verifier_tampon(&textes[0].agencement, &textes[0].tampon),
        Ok(())
    );
}

#[test]
fn un_glyphe_absent_se_dessine_comme_une_boite_et_se_compte() {
    let mut r = rendu();
    let s = scene(
        FOND,
        vec![noeud(
            1,
            None,
            SorteNoeud::Texte {
                texte: String::from("中"),
                boite: rect(0, 0, 40, 40),
                style: style(30),
            },
        )],
    );
    let cadre = Cadre::new(80, 80, forme(40, 40));
    let d = r.dessiner(&s, &cadre, &Contexte::au_repos()).unwrap();
    assert_eq!(d.glyphes_absents, 1);
    let image = relire(&r);
    let encre = (0..80 * 80)
        .filter(|k| image.pixel(k % 80, k / 80).unwrap() != [FOND.0, FOND.1, FOND.2, FOND.3])
        .count();
    assert!(
        encre > 20,
        "la boîte du glyphe absent n'est pas dessinée ({encre} pixels)"
    );
}

#[test]
fn un_bouton_a_le_fond_et_le_nom_du_commun() {
    let mut r = rendu();
    let boite = rect(10, 10, 100, 30);
    let s = scene(
        FOND,
        vec![noeud(
            1,
            None,
            SorteNoeud::Bouton {
                boite,
                nom: String::from("Valider"),
                action: vec![1],
            },
        )],
    );
    let cadre = Cadre::new(240, 100, forme(120, 50));
    let d = r.dessiner(&s, &cadre, &Contexte::au_repos()).unwrap();
    assert_eq!(d.textes, 1);
    let image = relire(&r);
    let f = bouton::FOND_DU_BOUTON;
    // Un coin du bouton, hors de la boîte du nom (une marge d'un cinquième de 30).
    assert_eq!(image.pixel(22, 22).unwrap(), [f.0, f.1, f.2, f.3]);
    // Le nom est blanc : un pixel au moins, au milieu, l'est presque.
    let blanc = (60..180)
        .flat_map(|x| (30..70).map(move |y| (x, y)))
        .any(|(x, y)| proches(image.pixel(x, y).unwrap(), [255, 255, 255, 255], 8));
    assert!(blanc, "le nom du bouton n'est pas dessiné");
}

#[test]
fn au_repos_une_animation_est_a_son_arrivee_sauf_le_rebond() {
    let mut r = rendu();
    let anime = |courbe| {
        let mut n = noeud(
            1,
            None,
            SorteNoeud::Rect {
                r: rect(10, 10, 10, 10),
                couleur: ROUGE,
            },
        );
        n.anime = Some((
            Instant::de(Pas::ZERO),
            Animation {
                duree: jeu_abi::Duree::pas(30),
                de: Visuel {
                    decalage: (Fixe::ZERO, Fixe::ZERO),
                    echelle_pour_mille: 1000,
                    opacite: 255,
                },
                a: Visuel {
                    decalage: (Fixe::unites(20), Fixe::ZERO),
                    echelle_pour_mille: 1000,
                    opacite: 255,
                },
                courbe,
            },
        ));
        scene(FOND, vec![n])
    };
    let cadre = Cadre::new(100, 60, forme(50, 30));
    let image = dessiner(
        &mut r,
        &anime(Courbe::Sortie),
        &cadre,
        &Contexte::au_repos(),
    );
    comparer(
        &image,
        &attendue(100, 60, FOND, &[([60, 20, 80, 40], ROUGE, 255)]),
        0,
    )
    .unwrap();
    let image = dessiner(
        &mut r,
        &anime(Courbe::Rebond),
        &cadre,
        &Contexte::au_repos(),
    );
    comparer(
        &image,
        &attendue(100, 60, FOND, &[([20, 20, 40, 40], ROUGE, 255)]),
        0,
    )
    .unwrap();
    // À mi-chemin, le rebond est au plus haut : à l'arrivée.
    let mi = Contexte {
        maintenant: Some(Instant::de(Pas::du_fil(15))),
        ..Contexte::au_repos()
    };
    let image = dessiner(&mut r, &anime(Courbe::Rebond), &cadre, &mi);
    comparer(
        &image,
        &attendue(100, 60, FOND, &[([60, 20, 80, 40], ROUGE, 255)]),
        0,
    )
    .unwrap();
}

/// Le contrôle négatif du lot : l'option de test qui oublie un nœud, et le pixel relu
/// à sa place n'est plus le sien.
#[test]
fn l_oubli_d_un_noeud_se_voit_aux_pixels() {
    let mut r = rendu();
    let cadre = Cadre::new(320, 180, forme(160, 90));
    let oublier = |n: u32| Contexte {
        fautif: Some(Fautif::OublierLeNoeud(n)),
        ..Contexte::au_repos()
    };
    let image = dessiner(&mut r, &scene_de_rectangles(), &cadre, &oublier(4));
    let opaques = [
        ([20, 20, 100, 60], ROUGE, 255),
        ([100, 60, 160, 120], VERT, 255),
        ([10, 10, 30, 30], BLEU, 255),
    ];
    // Le dernier rectangle (à moitié opaque) manque : l'écart se voit, à 2 près.
    let transparent = ([140, 60, 200, 80], BLEU, 128);
    let tous = [opaques.as_slice(), &[transparent]].concat();
    assert!(comparer(&image, &attendue(320, 180, FOND, &tous), 2).is_err());
    comparer(&image, &attendue(320, 180, FOND, &opaques), 0).unwrap();
    // Le vert oublié (nœud 2, enfant du groupe) manque seul : le groupe transmet encore
    // son décalage au bleu transparent, posé à sa place.
    let image = dessiner(&mut r, &scene_de_rectangles(), &cadre, &oublier(2));
    let sans_le_vert = [
        ([20, 20, 100, 60], ROUGE, 255),
        ([10, 10, 30, 30], BLEU, 255),
        transparent,
    ];
    comparer(&image, &attendue(320, 180, FOND, &sans_le_vert), 2).unwrap();
}

/// GPU perdu (CONTRAT, « Le GPU ») : l'appareil détruit, le rendu refait sur un autre
/// redessine la même image.
#[test]
fn apres_la_perte_du_gpu_l_image_revient() {
    let mut r = rendu();
    let cle = Cle::du_fil(7);
    r.poser_image(
        cle,
        ImageRgba {
            largeur: 1,
            hauteur: 1,
            rgba: vec![1, 2, 3, 255],
        },
    );
    let mut s = scene_de_rectangles();
    s.noeuds.push(noeud(
        9,
        None,
        SorteNoeud::Image {
            image: cle,
            r: rect(120, 60, 20, 20),
            ech: Echantillonnage::Lisse,
        },
    ));
    s.noeuds.push(noeud(
        10,
        None,
        SorteNoeud::Texte {
            texte: String::from("Encore là"),
            boite: rect(0, 60, 100, 30),
            style: style(10),
        },
    ));
    let cadre = Cadre::new(320, 180, forme(160, 90));
    let avant = dessiner(&mut r, &s, &cadre, &Contexte::au_repos());
    r.appareil().destroy();
    let (a, f) = commun::appareil();
    r.recreer(a, f);
    let apres = dessiner(&mut r, &s, &cadre, &Contexte::au_repos());
    assert_eq!(avant, apres);
    assert_eq!(apres.pixel(250, 130), Some([1, 2, 3, 255]));
}

#[test]
fn un_compte_a_rebours_se_forme_depuis_le_pas() {
    use jeu_abi::CompteAReboursFil;
    use mise_en_page::compte::texte as texte_du_compte;
    let court = CompteAReboursFil::Court {
        echeance: Pas::du_fil(600),
    };
    assert_eq!(texte_du_compte(&court, Pas::du_fil(0)), "10");
    assert_eq!(texte_du_compte(&court, Pas::du_fil(599)), "1");
    assert_eq!(texte_du_compte(&court, Pas::du_fil(700)), "0");
    let fige = CompteAReboursFil::Fige {
        restant: jeu_abi::Duree::s(75),
    };
    assert_eq!(texte_du_compte(&fige, Pas::ZERO), "1:15");
}
