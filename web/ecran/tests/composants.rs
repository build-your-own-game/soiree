//! Les composants de la télé, sans navigateur (lot 11) : le glisser (`deplacables`), les
//! champs (`saisie`), ce que vise un clic ou une touche (`cibles`). Chaque test cite la
//! leçon qui l'a fait naître.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y ont
//! droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles d'écriture »).

#![cfg(test)]

use ecran::cibles::{clic, modale, raccourci};
use ecran::deplacables::{Glisser, Plateau, Zone, defilement, place_visee};
use ecran::saisie::{Champ, couper};
use jeu_abi::{
    Cle, CodeTouche, ComposantVuFil, Deplace, Duree, Fixe, FormeEcran, Mods, NoeudFil, Oui,
    RaccourciFil, Rect, Rgba, SceneFil, SorteNoeud, Unite, ZoneVu,
};
use rendu::{Cadre, Contexte, boites};

fn cle(n: u64) -> Cle {
    Cle::du_fil(n)
}

fn r(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

/// Trois objets de 80 × 80 dans une zone, sur une ligne, puis une seconde zone vide.
fn plateau() -> Plateau {
    let o = |n: u64, x: f32| (cle(n), [x, 0.0, x + 80.0, 80.0]);
    Plateau {
        cle: cle(99),
        zones: vec![
            Zone {
                cle: cle(1),
                boite: [0.0, 0.0, 400.0, 80.0],
                objets: vec![o(10, 0.0), o(11, 80.0), o(12, 160.0)],
            },
            Zone {
                cle: cle(2),
                boite: [0.0, 100.0, 400.0, 180.0],
                objets: Vec::new(),
            },
        ],
        poubelle: Some((cle(3), [0.0, 300.0, 300.0, 360.0])),
        seuil: 4.0,
        bord: 70.0,
        vitesse_max: 1320.0,
        plafond_s: 0.05,
    }
}

/// TL-39 : la place visée se compte dans l'ordre de l'état, sans l'objet tenu ; le
/// premier objet dont le bord haut est sous le pointeur, ou qui est à sa hauteur et dont
/// le milieu est à sa droite.
#[test]
fn la_place_visee_se_compte_sans_l_objet_tenu_tl39() {
    let p = plateau();
    let objets = &p.zones[0].objets;
    // À gauche du milieu du premier objet : avant lui.
    assert_eq!(place_visee(objets, cle(12), (10.0, 40.0)), 0);
    // À droite du milieu du premier, à gauche de celui du second : entre eux.
    assert_eq!(place_visee(objets, cle(12), (50.0, 40.0)), 1);
    // Sans l'objet tenu : le second objet tenu, au-dessus du troisième, vise 1.
    assert_eq!(place_visee(objets, cle(11), (170.0, 40.0)), 1);
    // Au-dessus de tous : avant le premier.
    assert_eq!(place_visee(objets, cle(12), (300.0, -5.0)), 0);
    // Après tous, à leur hauteur : la fin, comptée sans l'objet tenu.
    assert_eq!(place_visee(objets, cle(10), (390.0, 40.0)), 2);
    assert_eq!(place_visee(objets, cle(99), (390.0, 40.0)), 3);
}

/// TL-47 : le glisser ne devient actif qu'au-delà du seuil, en distance euclidienne ; en
/// dessous, relâcher ne fait rien, et rien n'est dit au jeu.
#[test]
fn sous_le_seuil_relacher_ne_fait_rien_tl47() {
    let p = plateau();
    let mut g = Glisser::appui(&p, (40.0, 40.0), 1).expect("un objet sous le pointeur");
    assert_eq!(g.objet, cle(10));
    // 2,8 pixels de chaque côté : 3,96 pixels, sous le seuil de 4.
    assert_eq!(g.deplacer(&p, (42.8, 42.8)), None);
    assert!(!g.actif);
    assert_eq!(g.relacher(&p), None);
    assert_eq!(g.annuler(), None);
    assert!(
        Glisser::appui(&p, (390.0, 40.0), 1).is_none(),
        "hors d'un objet"
    );
}

/// TL-36, TL-39, TL-43 : un glisser actif dit au jeu la place visée quand elle change, et
/// une seule fois ; hors de toute zone, la dernière place valide reste ; le relâché dépose
/// là, dans les termes de l'état.
#[test]
fn le_glisser_dit_la_place_visee_et_garde_la_derniere_valide_tl43() {
    let p = plateau();
    let mut g = Glisser::appui(&p, (40.0, 40.0), 7).unwrap();
    // Au début, la place valide est celle de l'objet : (zone 1, 0).
    assert_eq!(
        g.deplacer(&p, (45.0, 40.0)),
        Some(Deplace::Survol {
            objet: cle(10),
            zone: cle(1),
            indice: 0
        })
    );
    assert!(g.actif);
    // Même place : rien de plus.
    assert_eq!(g.deplacer(&p, (46.0, 40.0)), None);
    // Dans la seconde zone, vide : sa fin, 0.
    assert_eq!(
        g.deplacer(&p, (200.0, 140.0)),
        Some(Deplace::Survol {
            objet: cle(10),
            zone: cle(2),
            indice: 0
        })
    );
    // Hors de toute zone : la dernière place valide reste, rien n'est redit.
    assert_eq!(g.deplacer(&p, (600.0, 600.0)), None);
    assert_eq!(
        g.relacher(&p),
        Some(Deplace::Depose {
            objet: cle(10),
            zone: cle(2),
            indice: 0
        })
    );
    let vu = g.vu(&p).expect("un glisser actif se dessine");
    assert_eq!(vu.objet, cle(10));
    // La copie suit le pointeur, au même point saisi (40, 40 depuis le coin).
    assert_eq!(vu.decalage, (600.0 - 40.0, 600.0 - 40.0));
    // La marque est au début de la seconde zone, vide.
    let m = vu.marque.unwrap();
    assert!(m[1] >= 100.0 && m[3] <= 180.0, "{m:?}");
}

/// Le relâché au-dessus de la poubelle la vise ; Échap ou `pointercancel` annulent un
/// glisser actif (CAHIER de Tier list, 4.7 ; TL-44).
#[test]
fn la_poubelle_et_l_annulation() {
    let p = plateau();
    let mut g = Glisser::appui(&p, (100.0, 40.0), 1).unwrap();
    let _: Option<Deplace> = g.deplacer(&p, (100.0, 330.0));
    assert!(g.sur_la_poubelle);
    assert_eq!(
        g.relacher(&p),
        Some(Deplace::Depose {
            objet: cle(11),
            zone: cle(3),
            indice: 0
        })
    );
    assert!(g.vu(&p).unwrap().marque.is_none());
    assert_eq!(g.annuler(), Some(Deplace::Annule { objet: cle(11) }));
}

/// TL-40, TL-41, TL-42 : la page défile près du bord, à une vitesse proportionnelle à la
/// proximité, jusqu'à la plus grande au bord même ; la distance dépend du temps écoulé,
/// pas du nombre d'images ; un réveil compte au plus le plafond.
#[test]
fn le_defilement_depend_du_temps_et_du_plafond_tl41_tl42() {
    // Au milieu de la fenêtre : rien.
    assert_eq!(defilement(400.0, 800.0, 70.0, 1320.0, 0.016, 0.05), 0.0);
    // Au bord même du bas : la vitesse la plus grande, 1320 px/s, sur 16 ms.
    let bas = defilement(800.0, 800.0, 70.0, 1320.0, 0.016, 0.05);
    assert!((bas - 1320.0 * 0.016).abs() < 1e-3, "{bas}");
    // À mi-bande en haut : la moitié, vers le haut.
    let haut = defilement(35.0, 800.0, 70.0, 1320.0, 0.016, 0.05);
    assert!((haut + 660.0 * 0.016).abs() < 1e-3, "{haut}");
    // TL-41 : deux réveils de 8 ms font autant qu'un de 16 ms.
    let deux = 2.0 * defilement(800.0, 800.0, 70.0, 1320.0, 0.008, 0.05);
    assert!((deux - bas).abs() < 1e-3);
    // TL-42 : un réveil après une suspension de 2 s ne compte que 50 ms.
    let reveil = defilement(800.0, 800.0, 70.0, 1320.0, 2.0, 0.05);
    assert!((reveil - 1320.0 * 0.05).abs() < 1e-3, "{reveil}");
}

/// Le composant se lit dans la scène : ses zones et ses objets aux boîtes des nœuds qui
/// portent leurs clés, groupes compris (`rendu::boites`).
#[test]
fn le_plateau_se_lit_dans_la_scene_aux_boites_dessinees() {
    let mut s = SceneFil::vide();
    let rect = |c: u64, parent: Option<u32>, rr: Rect| {
        NoeudFil::simple(
            cle(c),
            parent,
            SorteNoeud::Rect {
                r: rr,
                couleur: Rgba::BLANC,
            },
        )
    };
    s.noeuds.push(rect(1, None, r(0, 0, 400, 80)));
    s.noeuds.push(NoeudFil::simple(
        cle(50),
        None,
        SorteNoeud::Groupe {
            decalage: (Fixe::unites(10), Fixe::unites(200)),
        },
    ));
    s.noeuds.push(rect(10, Some(1), r(0, 0, 80, 80)));
    s.noeuds.push(NoeudFil::simple(
        cle(99),
        None,
        SorteNoeud::Composant {
            boite: None,
            composant: ComposantVuFil::Deplacables {
                zones: vec![ZoneVu {
                    cle: cle(1),
                    objets: vec![cle(10), cle(77)],
                }],
                poubelle: None,
                seuil: Fixe::unites(4),
                bord: Fixe::unites(70),
                vitesse_max: Fixe::unites(1320),
                plafond_reveil: Duree::ms(50),
            },
        },
    ));
    let cadre = Cadre::new(1120, 800, FormeEcran::Page { largeur_max: 1120 });
    let b = boites(&s, &cadre, &Contexte::au_repos());
    let p = Plateau::de_la_scene(&s, &b, 1.0).unwrap();
    assert_eq!(p.cle, cle(99));
    assert_eq!(p.zones.len(), 1);
    // L'objet 77, qu'aucun nœud ne montre, ne se prend pas ; l'objet 10 est décalé par
    // son groupe.
    assert_eq!(
        p.zones[0].objets,
        vec![(cle(10), [10.0, 200.0, 90.0, 280.0])]
    );
    assert_eq!(p.seuil, 4.0);
    assert!((p.plafond_s - 0.05).abs() < 1e-6);
}

/// TL-27, TL-28 : la saisie envoyée tient dans la limite comptée dans l'unité du champ,
/// coupée entre deux graphèmes : une famille emoji reste entière ou part entière.
#[test]
fn une_saisie_se_coupe_entre_deux_graphemes_tl27_tl28() {
    let famille = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    assert_eq!(famille.encode_utf16().count(), 8);
    let t = format!("ab{famille}cd");
    // 10 unités UTF-16 : la famille (8) ne tient pas après « ab » à 9.
    assert_eq!(couper(&t, Unite::Utf16, 9), "ab");
    assert_eq!(couper(&t, Unite::Utf16, 10), format!("ab{famille}"));
    assert_eq!(couper(&t, Unite::Graphemes, 3), format!("ab{famille}"));
    assert_eq!(couper(&t, Unite::Utf16, 100), t);
    // « é » décomposé : une lettre et son accent restent ensemble.
    assert_eq!(
        couper("e\u{301}e\u{301}", Unite::PointsDeCode, 3),
        "e\u{301}"
    );
    assert_eq!(couper("ß", Unite::Capitales, 1), "");
}

/// TL-30, TL-32, TL-33 : un champ qui a le focus n'est pas réécrit par l'écho de ce qu'il
/// a envoyé, ni pendant une composition ; il l'est quand le jeu change la valeur lui-même
/// (le champ de nom vidé après « Add Text ») ; sans focus, il prend la valeur de la scène
/// (le texte en trop disparaît).
#[test]
fn le_texte_sous_le_curseur_n_est_jamais_reecrit_tl33() {
    let mut c = Champ::default();
    assert!(c.reecrire("Tier List", Oui::Non, Oui::Non, ""));
    // L'utilisateur tape « Tier List 2 » ; le jeu renvoie ce qu'il a reçu.
    let envoye = c.envoyer("Tier List 2", Unite::Utf16, 500).to_owned();
    assert!(!c.reecrire(&envoye, Oui::Oui, Oui::Non, "Tier List 2"));
    // Le jeu nettoie l'espace finale de « Tier List 2 » : ce n'est pas une valeur
    // nouvelle pour le jeu (la scène précédente avait déjà « Tier List 2 »).
    let _: &str = c.envoyer("Tier List 2 ", Unite::Utf16, 500);
    assert!(!c.reecrire("Tier List 2", Oui::Oui, Oui::Non, "Tier List 2 "));
    // Le jeu vide le champ : il est réécrit, même avec le focus.
    assert!(c.reecrire("", Oui::Oui, Oui::Non, "Tier List 2 "));
    // Pendant une composition, jamais.
    let mut d = Champ::default();
    let _: bool = d.reecrire("a", Oui::Non, Oui::Non, "");
    assert!(!d.reecrire("b", Oui::Oui, Oui::Oui, "a"));
    // TL-30 : sans focus, le texte en trop disparaît.
    let mut e = Champ::default();
    let coupe = e.envoyer("abcdef", Unite::Utf16, 3).to_owned();
    assert_eq!(coupe, "abc");
    assert!(e.reecrire("abc", Oui::Non, Oui::Non, "abcdef"));
}

/// TL-33 (lot 11) : les échos reviennent en retard sur une frappe lettre à lettre ; un
/// écho ancien (« B » quand « Ban » est déjà tapé) n'est pas un changement du jeu, et ne
/// réécrit pas le champ (constaté dans Firefox : le curseur revenait au début) ; le jeu
/// qui change la valeur lui-même la réécrit toujours.
#[test]
fn un_echo_en_retard_ne_reecrit_pas_le_champ_tl33() {
    let mut c = Champ::default();
    assert!(c.reecrire("Tier List", Oui::Non, Oui::Non, ""));
    for t in ["B", "Ba", "Ban"] {
        let _: &str = c.envoyer(t, Unite::Utf16, 500);
    }
    assert!(!c.reecrire("B", Oui::Oui, Oui::Non, "Ban"));
    assert!(!c.reecrire("Ba", Oui::Oui, Oui::Non, "Ban"));
    assert!(!c.reecrire("Ban", Oui::Oui, Oui::Non, "Ban"));
    // Tous les échos revenus : une valeur nouvelle vient du jeu.
    assert!(c.reecrire("", Oui::Oui, Oui::Non, "Ban"));
    assert!(c.envoyes.is_empty());
}

/// Une scène avec deux boutons qui se recouvrent, puis une modale et un bouton dans son
/// panneau.
fn scene_a_boutons(avec_modale: Oui) -> SceneFil {
    let mut s = SceneFil::vide();
    let bouton = |c: u64, rr: Rect, a: u8| {
        NoeudFil::simple(
            cle(c),
            None,
            SorteNoeud::Bouton {
                boite: rr,
                nom: String::from("b"),
                action: vec![a],
            },
        )
    };
    s.noeuds.push(bouton(1, r(0, 0, 100, 100), 1));
    s.noeuds.push(bouton(2, r(50, 50, 100, 100), 2));
    if avec_modale == Oui::Oui {
        s.noeuds.push(NoeudFil::simple(
            cle(3),
            None,
            SorteNoeud::Composant {
                boite: Some(r(0, 0, 400, 400)),
                composant: ComposantVuFil::Modale {
                    fermer: vec![9],
                    panneau: r(200, 200, 100, 100),
                },
            },
        ));
        s.noeuds.push(bouton(4, r(210, 210, 20, 20), 4));
    }
    s
}

/// SAL-152 : un clic vise le dernier bouton qui contient le point ; une modale ne laisse
/// viser que ce qui vient après elle, et un clic hors de son panneau la ferme (TL-25 : le
/// clic sur le voile).
#[test]
fn un_clic_vise_le_bouton_du_dessus_et_la_modale_ferme_au_dehors_sal152() {
    let cadre = Cadre::new(1120, 800, FormeEcran::Page { largeur_max: 1120 });
    let c = Contexte::au_repos();
    let s = scene_a_boutons(Oui::Non);
    let b = boites(&s, &cadre, &c);
    assert_eq!(clic(&s, &b, None, (60.0, 60.0)), Some(&[2u8][..]));
    assert_eq!(clic(&s, &b, None, (10.0, 10.0)), Some(&[1u8][..]));
    assert_eq!(clic(&s, &b, None, (300.0, 10.0)), None);
    let s = scene_a_boutons(Oui::Oui);
    let b = boites(&s, &cadre, &c);
    let m = modale(&s, &cadre, &c).unwrap();
    assert_eq!(m.indice, 2);
    assert_eq!(m.panneau, [200.0, 200.0, 300.0, 300.0]);
    // Le bouton 2, sous la modale, ne se vise plus : le clic, hors du panneau, la ferme.
    assert_eq!(clic(&s, &b, Some(&m), (60.0, 60.0)), Some(&[9u8][..]));
    // Dans le panneau, sur le bouton de la modale.
    assert_eq!(clic(&s, &b, Some(&m), (215.0, 215.0)), Some(&[4u8][..]));
    // Dans le panneau, hors de tout bouton : rien.
    assert_eq!(clic(&s, &b, Some(&m), (280.0, 280.0)), None);
}

/// Les raccourcis : `CMD_OU_CTRL` veut dire Cmd sur Mac et Ctrl ailleurs ; Maj compte ;
/// une répétition automatique seulement si le raccourci la veut (CAHIER de Tier list, § 7).
#[test]
fn un_raccourci_suit_cmd_sur_mac_et_ctrl_ailleurs() {
    let rac = |touche: &'static str, mods: Mods, repetition: Oui, a: u8| RaccourciFil {
        touche: CodeTouche::new(touche),
        mods,
        repetition,
        action: vec![a],
    };
    let maj = Mods {
        maj: Oui::Oui,
        ..Mods::CMD_OU_CTRL
    };
    let liste = [
        rac("KeyZ", Mods::CMD_OU_CTRL, Oui::Oui, 1),
        rac("KeyZ", maj, Oui::Oui, 2),
        rac("Escape", Mods::AUCUN, Oui::Non, 3),
    ];
    let cmd = Mods {
        meta: Oui::Oui,
        ..Mods::AUCUN
    };
    let ctrl = Mods {
        ctrl: Oui::Oui,
        ..Mods::AUCUN
    };
    assert_eq!(
        raccourci(&liste, "KeyZ", cmd, Oui::Non, Oui::Oui),
        Some(&[1u8][..])
    );
    assert_eq!(raccourci(&liste, "KeyZ", ctrl, Oui::Non, Oui::Oui), None);
    assert_eq!(
        raccourci(&liste, "KeyZ", ctrl, Oui::Non, Oui::Non),
        Some(&[1u8][..])
    );
    let cmd_maj = Mods {
        maj: Oui::Oui,
        ..cmd
    };
    assert_eq!(
        raccourci(&liste, "KeyZ", cmd_maj, Oui::Non, Oui::Oui),
        Some(&[2u8][..])
    );
    assert_eq!(
        raccourci(&liste, "Escape", Mods::AUCUN, Oui::Non, Oui::Oui),
        Some(&[3u8][..])
    );
    assert_eq!(
        raccourci(&liste, "Escape", Mods::AUCUN, Oui::Oui, Oui::Oui),
        None
    );
    assert_eq!(
        raccourci(&liste, "KeyZ", cmd, Oui::Oui, Oui::Oui),
        Some(&[1u8][..])
    );
}
